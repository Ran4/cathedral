use super::*;
use cathedral_sim::checkpoint::{
    Admitted, CheckpointBudget, Cohort,
    complete::{CompleteCheckpointCandidate, CompleteCheckpointInput},
};
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use std::{
    ffi::CString,
    fs::File,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    path::Path,
};

const RECORD_LIMIT: usize = 16 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    version: u16,
    slot: SlotId,
    candidate: OperationId,
    previous: Option<SlotReference>,
    older: Option<SlotReference>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record<T> {
    body: T,
    sha256: [u8; 32],
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Edge {
    Before,
    After,
}
#[cfg(test)]
pub(super) type Hook = std::sync::Arc<dyn Fn(Phase, Edge) -> std::io::Result<()> + Send + Sync>;

pub(super) struct Store {
    directory: File,
    // One interrupted publication blocks new publications globally until its
    // explicit recovery; never grow a map of failed in-memory generations.
    uncertain: Option<Journal>,
    #[cfg(test)]
    pub(super) hook: Option<Hook>,
    #[cfg(test)]
    pub(super) partial_payload_bytes: Option<usize>,
}
impl Store {
    pub(super) fn open(path: &Path) -> Result<Self, StorageError> {
        use std::os::unix::fs::OpenOptionsExt;
        let directory = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(|e| StorageError::io(Phase::Open, e))?;
        // Initial support is deliberately narrow. The ext filesystem family
        // uses this magic (validated on ext4); tmpfs and remote/unknown filesystems
        // must not silently inherit its durability claim.
        let mut filesystem = std::mem::MaybeUninit::<libc::statfs>::uninit();
        // SAFETY: a live directory FD and writable statfs output storage.
        if unsafe { libc::fstatfs(directory.as_raw_fd(), filesystem.as_mut_ptr()) } != 0 {
            return Err(StorageError::io(
                Phase::Open,
                std::io::Error::last_os_error(),
            ));
        }
        // SAFETY: successful fstatfs initialized the output structure.
        if unsafe { filesystem.assume_init() }.f_type != 0xef53 {
            return Err(StorageError {
                phase: Phase::Open,
                kind: std::io::ErrorKind::Unsupported,
                message: "durable checkpoint storage currently requires a Linux ext-family filesystem",
            });
        }
        // SAFETY: a live owned directory descriptor; no borrowed pointer.
        if unsafe { libc::flock(directory.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(StorageError::io(
                Phase::Open,
                std::io::Error::last_os_error(),
            ));
        }
        directory
            .sync_all()
            .map_err(|e| StorageError::io(Phase::Open, e))?;
        Ok(Self {
            directory,
            uncertain: None,
            #[cfg(test)]
            hook: None,
            #[cfg(test)]
            partial_payload_bytes: None,
        })
    }
    fn at<T>(
        &self,
        phase: Phase,
        f: impl FnOnce() -> std::io::Result<T>,
    ) -> Result<T, StorageError> {
        #[cfg(test)]
        if let Some(hook) = &self.hook {
            hook(phase, Edge::Before).map_err(|e| StorageError::io(phase, e))?;
        }
        let value = f().map_err(|e| StorageError::io(phase, e))?;
        #[cfg(test)]
        if let Some(hook) = &self.hook {
            hook(phase, Edge::After).map_err(|e| StorageError::io(phase, e))?;
        }
        Ok(value)
    }
    fn open_file(&self, name: &str, create: bool) -> std::io::Result<File> {
        let name = CString::new(name).expect("internal filename contains no NUL");
        let flags = if create {
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL
        } else {
            libc::O_RDONLY
        };
        // SAFETY: live FD and terminated internal filename; returned FD is owned.
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                0o600,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: successful openat supplied this unique owned descriptor.
        let file = unsafe { File::from_raw_fd(fd) };
        if !file.metadata()?.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "not a regular file",
            ));
        }
        Ok(file)
    }
    fn unlink(&self, name: &str) -> std::io::Result<()> {
        let name = CString::new(name).expect("internal filename");
        // SAFETY: pinned FD and internal terminated name; never removes a directory.
        if unsafe { libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0) } == 0 {
            return Ok(());
        }
        let e = std::io::Error::last_os_error();
        if e.kind() == std::io::ErrorKind::NotFound {
            Ok(())
        } else {
            Err(e)
        }
    }
    fn rename(&self, from: &str, to: &str, immutable: bool) -> std::io::Result<()> {
        let from = CString::new(from).expect("internal filename");
        let to = CString::new(to).expect("internal filename");
        let fd = self.directory.as_raw_fd();
        // SAFETY: both names are internal, terminated and relative to the same
        // pinned directory. linkat refuses replacement of immutable payloads.
        let result = unsafe {
            if immutable {
                libc::linkat(fd, from.as_ptr(), fd, to.as_ptr(), 0)
            } else {
                libc::renameat(fd, from.as_ptr(), fd, to.as_ptr())
            }
        };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    fn read_record<T: DeserializeOwned + Serialize>(
        &self,
        name: &str,
    ) -> Result<Option<T>, StorageError> {
        let mut file = match self.open_file(name, false) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(StorageError::io(Phase::Inspect, e)),
        };
        let mut bytes = [0u8; RECORD_LIMIT + 1];
        let mut len = 0;
        while len < bytes.len() {
            let n = file
                .read(&mut bytes[len..])
                .map_err(|e| StorageError::io(Phase::Inspect, e))?;
            if n == 0 {
                break;
            }
            len += n;
        }
        if len > RECORD_LIMIT {
            return Err(StorageError::invalid("reference byte limit exceeded"));
        }
        let record: Record<T> = serde_json::from_slice(&bytes[..len])
            .map_err(|_| StorageError::invalid("invalid closed reference record"))?;
        let encoded = serde_json::to_vec(&record.body)
            .map_err(|_| StorageError::invalid("invalid reference body"))?;
        if hash(&encoded) != record.sha256 {
            return Err(StorageError::invalid("reference checksum mismatch"));
        }
        Ok(Some(record.body))
    }
    fn write_record<T: Serialize>(
        &self,
        slot: &SlotId,
        suffix: &str,
        value: &T,
        phases: [Phase; 4],
    ) -> Result<(), StorageError> {
        let body = serde_json::to_vec(value)
            .map_err(|_| StorageError::invalid("reference encoding failed"))?;
        let encoded = serde_json::to_vec(&Record {
            body: value,
            sha256: hash(&body),
        })
        .map_err(|_| StorageError::invalid("reference encoding failed"))?;
        if encoded.len() > RECORD_LIMIT {
            return Err(StorageError::invalid("reference byte limit exceeded"));
        }
        let name = slot_file(slot, suffix);
        let temp = format!("{name}.tmp");
        let file = self.at(phases[0], || {
            self.unlink(&temp)?;
            let mut f = self.open_file(&temp, true)?;
            f.write_all(&encoded)?;
            Ok(f)
        })?;
        self.at(phases[1], || file.sync_all())?;
        self.at(phases[2], || self.rename(&temp, &name, false))?;
        self.at(phases[3], || self.directory.sync_all())
    }
    fn reference(
        &self,
        slot: &SlotId,
        suffix: &str,
    ) -> Result<Option<SlotReference>, StorageError> {
        let reference = self.read_record::<SlotReference>(&slot_file(slot, suffix))?;
        if let Some(r) = &reference {
            validate_reference(r, slot)?;
        }
        Ok(reference)
    }
    fn journal(&self, slot: &SlotId) -> Result<Option<Journal>, StorageError> {
        if let Some(j) = &self.uncertain {
            if &j.slot == slot {
                return Ok(Some(j.clone()));
            }
        }
        let journal = self.read_record::<Journal>(&slot_file(slot, "pending"))?;
        if let Some(j) = &journal {
            if j.version != 1
                || &j.slot != slot
                || j.candidate.service == [0; 16]
                || j.candidate.sequence == 0
            {
                return Err(StorageError::invalid("invalid publication journal"));
            }
            for r in [&j.previous, &j.older].into_iter().flatten() {
                validate_reference(r, slot)?;
            }
        }
        Ok(journal)
    }
    fn verify_payload(&self, reference: &SlotReference) -> Result<File, StorageError> {
        let mut file = self
            .open_file(&payload_file(&reference.slot, reference.generation), false)
            .map_err(|e| StorageError::io(Phase::Read, e))?;
        verify_file(&mut file, reference.payload_bytes, reference.payload_sha256)
            .map_err(|e| StorageError::io(Phase::Read, e))?;
        Ok(file)
    }
    pub(super) fn save(
        &mut self,
        slot: &SlotId,
        id: OperationId,
        metadata: SaveMetadata,
        payload: &Admitted<CompleteCheckpointCandidate>,
    ) -> Result<SlotReference, StorageError> {
        if self.uncertain.is_some() || self.journal(slot)?.is_some() {
            return Err(StorageError::invalid(
                "interrupted publication requires explicit recovery",
            ));
        }
        let previous = self.reference(slot, "active")?;
        let older = self.reference(slot, "previous")?;
        if previous.is_none() && older.is_some() {
            return Err(StorageError::invalid(
                "missing active requires explicit previous recovery",
            ));
        }
        // Never rotate an invalid active reference over a valid recovery copy.
        if let Some(r) = &previous {
            self.verify_payload(r)?;
        }
        let journal = Journal {
            version: 1,
            slot: slot.clone(),
            candidate: id,
            previous,
            older,
        };
        self.uncertain = Some(journal.clone());
        let result = self.publish(&journal, metadata, payload);
        if result.is_ok() {
            self.uncertain = None;
        }
        result
    }
    fn publish(
        &self,
        journal: &Journal,
        metadata: SaveMetadata,
        payload: &Admitted<CompleteCheckpointCandidate>,
    ) -> Result<SlotReference, StorageError> {
        use Phase::*;
        let slot = &journal.slot;
        self.write_record(
            slot,
            "pending",
            journal,
            [JournalWrite, JournalFlush, JournalReplace, JournalSync],
        )?;
        let value = payload.value();
        let reference = SlotReference {
            format_version: 1,
            slot: slot.clone(),
            generation: journal.candidate,
            payload_bytes: value.bytes().len() as u64,
            payload_sha256: hash(value.bytes()),
            world_identity: value.world_identity().bytes(),
            boundary: value.boundary().seconds(),
            metadata,
        };
        validate_reference(&reference, slot)?;
        let target = payload_file(slot, reference.generation);
        let temp = format!("{target}.tmp");
        let mut file = self.at(PayloadWrite, || {
            let mut f = self.open_file(&temp, true)?;
            #[cfg(test)]
            if let Some(limit) = self.partial_payload_bytes {
                f.write_all(&value.bytes()[..limit.min(value.bytes().len())])?;
                return Err(std::io::Error::from_raw_os_error(libc::ENOSPC));
            }
            f.write_all(value.bytes())?;
            Ok(f)
        })?;
        self.at(PayloadValidate, || {
            verify_file(&mut file, reference.payload_bytes, reference.payload_sha256)
        })?;
        self.at(PayloadFlush, || file.sync_all())?;
        self.at(PayloadPublish, || self.rename(&temp, &target, true))?;
        self.at(PayloadSync, || self.directory.sync_all())?;
        if let Some(previous) = &journal.previous {
            self.write_record(
                slot,
                "previous",
                previous,
                [RecoveryWrite, RecoveryFlush, RecoveryReplace, RecoverySync],
            )?;
        }
        self.write_record(
            slot,
            "active",
            &reference,
            [ActiveWrite, ActiveFlush, ActiveReplace, ActiveSync],
        )?;
        // New active and its prior acknowledged reference are now durable.
        // Only their known older ancestor and this candidate's temp may go.
        self.cleanup(
            journal,
            Some(reference.generation),
            journal.previous.as_ref().map(|r| r.generation),
        )?;
        self.finish(slot)?;
        Ok(reference)
    }
    fn cleanup(
        &self,
        journal: &Journal,
        active: Option<OperationId>,
        previous: Option<OperationId>,
    ) -> Result<(), StorageError> {
        self.at(Phase::Cleanup, || {
            let ids = [
                Some(journal.candidate),
                journal.older.as_ref().map(|r| r.generation),
            ];
            for id in ids.into_iter().flatten() {
                let name = payload_file(&journal.slot, id);
                self.unlink(&format!("{name}.tmp"))?;
                if Some(id) != active && Some(id) != previous {
                    self.unlink(&name)?;
                }
            }
            for suffix in ["active.tmp", "previous.tmp", "pending.tmp"] {
                self.unlink(&slot_file(&journal.slot, suffix))?;
            }
            Ok(())
        })?;
        self.at(Phase::CleanupSync, || self.directory.sync_all())
    }
    fn finish(&self, slot: &SlotId) -> Result<(), StorageError> {
        self.at(Phase::PendingRemove, || {
            self.unlink(&slot_file(slot, "pending"))
        })?;
        self.at(Phase::PendingSync, || self.directory.sync_all())
    }
    pub(super) fn recover(
        &mut self,
        slot: &SlotId,
        id: OperationId,
    ) -> Result<Option<SlotReference>, StorageError> {
        use Phase::*;
        if self.uncertain.as_ref().is_some_and(|j| &j.slot != slot) {
            return Err(StorageError::invalid("another slot requires recovery"));
        }
        let journal = match self.journal(slot).ok().flatten() {
            Some(journal) => journal,
            None => {
                // Explicit recovery can select a valid previous reference even
                // when the active file or journal is damaged. Unknown artifacts
                // are quarantined: only a valid active reference supplies a
                // candidate identity eligible for deletion.
                let previous = self.reference(slot, "previous")?.ok_or_else(|| {
                    StorageError::invalid("no acknowledged predecessor to recover")
                })?;
                let candidate = self
                    .reference(slot, "active")
                    .ok()
                    .flatten()
                    .map_or(id, |r| r.generation);
                Journal {
                    version: 1,
                    slot: slot.clone(),
                    candidate,
                    previous: Some(previous),
                    older: None,
                }
            }
        };
        if let Some(previous) = &journal.previous {
            self.verify_payload(previous)?;
        }
        // Keep the original journal through EVERY recovery attempt. In
        // particular never snapshot the newly visible unacknowledged active.
        self.uncertain = Some(journal.clone());
        self.write_record(
            slot,
            "pending",
            &journal,
            [JournalWrite, JournalFlush, JournalReplace, JournalSync],
        )?;
        if let Some(previous) = &journal.previous {
            self.write_record(
                slot,
                "previous",
                previous,
                [RecoveryWrite, RecoveryFlush, RecoveryReplace, RecoverySync],
            )?;
            self.write_record(
                slot,
                "active",
                previous,
                [ActiveWrite, ActiveFlush, ActiveReplace, ActiveSync],
            )?;
        } else {
            self.at(ActiveReplace, || {
                self.unlink(&slot_file(slot, "active"))?;
                self.unlink(&slot_file(slot, "previous"))
            })?;
            self.at(ActiveSync, || self.directory.sync_all())?;
        }
        let generation = journal.previous.as_ref().map(|r| r.generation);
        self.cleanup(&journal, generation, generation)?;
        self.finish(slot)?;
        self.uncertain = None;
        Ok(journal.previous)
    }
    pub(super) fn load(
        &self,
        slot: &SlotId,
        selection: LoadSelection,
        budget: &CheckpointBudget,
    ) -> Result<LoadedCheckpoint, StorageError> {
        let journal = self.journal(slot)?;
        let reference = match selection {
            LoadSelection::Active => {
                if journal.is_some() {
                    return Err(StorageError::invalid(
                        "active publication is interrupted; select previous",
                    ));
                }
                self.reference(slot, "active")?
            }
            LoadSelection::Previous => match journal {
                Some(j) => j.previous,
                None => self.reference(slot, "previous")?,
            },
        }
        .ok_or_else(|| StorageError {
            phase: Phase::Read,
            kind: std::io::ErrorKind::NotFound,
            message: "selected slot generation does not exist",
        })?;
        let mut file = self.at(Phase::Read, || {
            self.open_file(&payload_file(slot, reference.generation), false)
        })?;
        let len = usize::try_from(reference.payload_bytes)
            .map_err(|_| StorageError::invalid("payload length overflow"))?;
        // This reservation precedes any payload allocation. The original Vec
        // moves into CompleteCheckpointInput; no uncharged extracted clone.
        let mut reservation = budget
            .reserve(Cohort::LoadCandidate, len + 4096)
            .map_err(|_| StorageError::admission("load candidate admission refused"))?;
        let mut bytes = Vec::with_capacity(len);
        if bytes.capacity() > len {
            reservation
                .resize(bytes.capacity() + 4096)
                .map_err(|_| StorageError::admission("load capacity admission refused"))?;
        }
        self.at(Phase::Read, || {
            if file.metadata()?.len() != reference.payload_bytes {
                return Err(integrity());
            }
            bytes.resize(len, 0);
            file.read_exact(&mut bytes)?;
            let mut extra = [0u8];
            if file.read(&mut extra)? != 0 || hash(&bytes) != reference.payload_sha256 {
                return Err(integrity());
            }
            Ok(())
        })?;
        let mut input =
            CompleteCheckpointInput::from_owned(bytes, reservation).map_err(|error| {
                if error.owner == "admission" {
                    StorageError::admission("load owner admission refused")
                } else {
                    StorageError::invalid("invalid bounded checkpoint input").at(Phase::Envelope)
                }
            })?;
        let summary = input.inspect_envelope().map_err(|error| {
            if error.owner == "admission" {
                StorageError::admission("checkpoint envelope inspection scratch is busy")
            } else {
                StorageError::invalid("incompatible or malformed checkpoint envelope")
                    .at(Phase::Envelope)
            }
        })?;
        if summary.version != reference.format_version
            || summary.world_identity.bytes() != reference.world_identity
            || summary.boundary.seconds().to_bits() != reference.boundary.to_bits()
        {
            return Err(
                StorageError::invalid("payload and reference metadata disagree")
                    .at(Phase::Envelope),
            );
        }
        Ok(LoadedCheckpoint { reference, input })
    }
}
fn validate_reference(reference: &SlotReference, slot: &SlotId) -> Result<(), StorageError> {
    if &reference.slot != slot
        || reference.format_version != 1
        || reference.world_identity == [0; 16]
        || reference.generation.service == [0; 16]
        || reference.generation.sequence == 0
        || reference.payload_bytes == 0
        || reference.payload_bytes > cathedral_sim::checkpoint::POPULATED_PAYLOAD_BYTES as u64
        || !reference.boundary.is_finite()
        || reference.boundary < 0.0
    {
        return Err(StorageError::invalid("invalid slot reference fields"));
    }
    reference.metadata.validate()
}
fn verify_file(file: &mut File, len: u64, expected: [u8; 32]) -> std::io::Result<()> {
    use std::io::{Seek, SeekFrom};
    if file.metadata()?.len() != len {
        return Err(integrity());
    }
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut count = 0u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > len {
            return Err(integrity());
        }
        hash.update(&buffer[..n]);
    }
    if count != len || <[u8; 32]>::from(hash.finalize()) != expected {
        return Err(integrity());
    }
    Ok(())
}
fn integrity() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "payload length/checksum mismatch",
    )
}
pub(super) fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
pub(super) fn slot_file(slot: &SlotId, suffix: &str) -> String {
    format!("slot-{}.{}", slot.as_str(), suffix)
}
pub(super) fn payload_file(slot: &SlotId, id: OperationId) -> String {
    let service: String = id.service.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "slot-{}.gen-{service}-{:016x}.json",
        slot.as_str(),
        id.sequence
    )
}
