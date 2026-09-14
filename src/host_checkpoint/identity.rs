//! Host-only initial identity. No simulation IO and no deployment-path reread.
use cathedral_sim::checkpoint::complete::WorldIdentity;
use std::{io::Read, sync::OnceLock};
#[derive(Clone, Copy, serde::Serialize)]
pub(crate) struct HostImageIdentity {
    pub digest: [u8; 32],
    pub bytes: u64,
    pub initial_hash_microseconds: u128,
}
pub(crate) fn image_identity() -> Option<HostImageIdentity> {
    static IMAGE: OnceLock<Option<HostImageIdentity>> = OnceLock::new();
    *IMAGE.get_or_init(|| {
        use sha2::{Digest, Sha256};
        let start = std::time::Instant::now();
        // Linux exposes the currently executing inode, including after atomic
        // deployment replacement. Read failure disables only complete capture.
        let mut file = std::fs::File::open("/proc/self/exe").ok()?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        let mut bytes = 0u64;
        loop {
            let n = file.read(&mut buffer).ok()?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
            bytes = bytes.checked_add(n as u64)?;
        }
        Some(HostImageIdentity {
            digest: hash.finalize().into(),
            bytes,
            initial_hash_microseconds: start.elapsed().as_micros(),
        })
    })
}
pub(crate) fn lineage_identity() -> Option<WorldIdentity> {
    #[cfg(test)]
    if let Ok(hex) = std::env::var("ALIBI_COMPLETE_WORLD_ID") {
        // Explicit fixture input before Engine construction, never a rewrite of
        // captured authority. Ordinary tests/startup still receive OS entropy.
        if hex.len() != 32 || !hex.is_ascii() {
            return None;
        }
        let mut bytes = [0u8; 16];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).ok()?;
        }
        return WorldIdentity::from_bytes(bytes).ok();
    }
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .ok()?;
    WorldIdentity::from_bytes(bytes).ok()
}
