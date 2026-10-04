//! Save slots (D06): one file per slot, `<dir>/<slot>.save`.
//!
//! The first line is a small JSON [`SlotHeader`] — version, label, wall-clock
//! save time — so a slot list never has to decode a city; the rest of the file
//! is the gzip'd JSON of a [`SaveGame`].
//!
//! Writes run on one worker thread ([`SaveWriter`]), never in the engine pump
//! (D05): encode, decode the bytes back as a check, write `<slot>.tmp` and
//! fsync it, keep the previous save as `<slot>.bak`, then rename the new file
//! into place.

use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use cathedral_sim::{CHECKPOINT_VERSION, Checkpoint, CheckpointLabel};
use crossbeam_channel::{Receiver, Sender, unbounded};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use serde::{Deserialize, Serialize};

/// The player state only the host has. The sim knows where the player stands
/// and which way they face; where they look and whether they fly are the
/// controller's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct HostSave {
    /// Radians, positive up.
    pub pitch: f32,
    pub flying: bool,
}

/// Everything one slot holds.
#[derive(Clone, Serialize, Deserialize)]
pub struct SaveGame {
    pub checkpoint: Checkpoint,
    pub host: HostSave,
}

/// A slot's first line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotHeader {
    pub version: u32,
    pub label: CheckpointLabel,
    /// Wall-clock seconds since the Unix epoch.
    pub saved_at_unix: u64,
}

/// What one write cost, for the log line.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotWrite {
    pub path: PathBuf,
    pub json_bytes: usize,
    pub file_bytes: usize,
    pub encode_ms: f64,
    pub write_ms: f64,
}

/// A slot name is a file stem: 1–64 of `[A-Za-z0-9_-]`.
pub fn slot_path(dir: &Path, slot: &str) -> Result<PathBuf, String> {
    let valid = (1..=64).contains(&slot.len())
        && slot
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
    if !valid {
        return Err(format!(
            "slot name {slot:?} must be 1-64 letters, digits, `_` or `-`"
        ));
    }
    Ok(dir.join(format!("{slot}.save")))
}

/// Encode, check and atomically write one slot.
pub fn write_slot(dir: &Path, slot: &str, game: &SaveGame) -> Result<SlotWrite, String> {
    let path = slot_path(dir, slot)?;
    let started = Instant::now();
    let json = serde_json::to_vec(game).map_err(|error| format!("could not encode: {error}"))?;
    serde_json::from_slice::<SaveGame>(&json)
        .map_err(|error| format!("the encoded save does not decode: {error}"))?;
    let header = SlotHeader {
        version: game.checkpoint.version,
        label: game.checkpoint.label.clone(),
        saved_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs()),
    };
    let mut bytes = serde_json::to_vec(&header).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    let mut gzip = GzEncoder::new(bytes, Compression::default());
    gzip.write_all(&json).map_err(|error| error.to_string())?;
    let bytes = gzip.finish().map_err(|error| error.to_string())?;
    let encode_ms = started.elapsed().as_secs_f64() * 1e3;

    let started = Instant::now();
    let io = |error: std::io::Error| format!("could not write {}: {error}", path.display());
    fs::create_dir_all(dir).map_err(io)?;
    let tmp = path.with_extension("tmp");
    let mut file = File::create(&tmp).map_err(io)?;
    file.write_all(&bytes).map_err(io)?;
    file.sync_all().map_err(io)?;
    drop(file);
    if path.exists() {
        fs::rename(&path, path.with_extension("bak")).map_err(io)?;
    }
    fs::rename(&tmp, &path).map_err(io)?;
    Ok(SlotWrite {
        json_bytes: json.len(),
        file_bytes: bytes.len(),
        encode_ms,
        write_ms: started.elapsed().as_secs_f64() * 1e3,
        path,
    })
}

/// Read and decode one slot. A save of another format version is refused on
/// its header, before the city is decoded.
pub fn read_slot(dir: &Path, slot: &str) -> Result<SaveGame, String> {
    let mut path = slot_path(dir, slot)?;
    // A crash between `write_slot`'s two renames leaves only the previous save.
    if !path.exists() && path.with_extension("bak").exists() {
        path = path.with_extension("bak");
    }
    let file = File::open(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut reader = BufReader::new(file);
    let header = read_header(&mut reader)?;
    if header.version != CHECKPOINT_VERSION {
        return Err(format!(
            "slot {slot} is save format {}; this build reads {CHECKPOINT_VERSION}",
            header.version
        ));
    }
    let mut json = Vec::new();
    GzDecoder::new(reader)
        .read_to_end(&mut json)
        .map_err(|error| format!("slot {slot} is damaged: {error}"))?;
    serde_json::from_slice(&json).map_err(|error| format!("slot {slot} does not decode: {error}"))
}

/// Every slot in `dir` with a readable header, newest first. Headers only.
pub fn list_slots(dir: &Path) -> Vec<(String, SlotHeader)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut slots: Vec<(String, SlotHeader)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()? != "save" {
                return None;
            }
            let slot = path.file_stem()?.to_str()?.to_string();
            let header = read_header(&mut BufReader::new(File::open(&path).ok()?)).ok()?;
            Some((slot, header))
        })
        .collect();
    slots.sort_by(|a, b| {
        b.1.saved_at_unix
            .cmp(&a.1.saved_at_unix)
            .then_with(|| a.0.cmp(&b.0))
    });
    slots
}

fn read_header(reader: &mut impl BufRead) -> Result<SlotHeader, String> {
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|error| format!("unreadable save header: {error}"))?;
    serde_json::from_str(&line).map_err(|error| format!("not a save file: {error}"))
}

/// One finished write.
pub struct SaveOutcome {
    pub slot: String,
    pub result: Result<SlotWrite, String>,
}

/// The one thread every save is written on. Dropping it waits for the writes
/// already handed to it, so a save requested just before quitting lands.
pub struct SaveWriter {
    jobs: Option<Sender<(String, SaveGame)>>,
    done: Receiver<SaveOutcome>,
    thread: Option<JoinHandle<()>>,
}

impl SaveWriter {
    pub fn spawn(dir: PathBuf) -> Result<Self, String> {
        let (jobs, inbox) = unbounded::<(String, SaveGame)>();
        let (report, done) = unbounded();
        let thread = thread::Builder::new()
            .name("save-writer".into())
            .spawn(move || {
                for (slot, game) in inbox {
                    let result = write_slot(&dir, &slot, &game);
                    if report.send(SaveOutcome { slot, result }).is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| format!("could not start the save writer: {error}"))?;
        Ok(Self {
            jobs: Some(jobs),
            done,
            thread: Some(thread),
        })
    }

    pub fn save(&self, slot: String, game: SaveGame) -> Result<(), String> {
        self.jobs
            .as_ref()
            .and_then(|jobs| jobs.send((slot, game)).ok())
            .ok_or_else(|| "the save writer has stopped".to_string())
    }

    /// The writes finished since the last call.
    pub fn finished(&self) -> Vec<SaveOutcome> {
        self.done.try_iter().collect()
    }
}

impl Drop for SaveWriter {
    fn drop(&mut self) {
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
