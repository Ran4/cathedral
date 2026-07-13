//! The private runtime directory for a session's audio (D28).
//!
//! Recorded utterances and synthesized speech are files on disk with a
//! player's voice in them, so the directory is created 0700 and removed when
//! the handle drops — including on a panic. cathedral-backends owns it because
//! it must exist *before* the game's microphone worker starts writing into it;
//! `BridgeHandle::runtime_dir()` keeps exposing the path (the mic worker's own
//! path confinement is unchanged).

use std::{
    fs,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// `<temp>/cathedral-smart-actors-<session id>`, removed on drop.
#[derive(Debug)]
pub struct SessionDir {
    path: PathBuf,
}

impl SessionDir {
    /// Create the directory under the OS temp dir.
    pub fn create(session_id: &str) -> std::io::Result<Self> {
        Self::create_in(&std::env::temp_dir(), session_id)
    }

    /// Create it under `parent` — the same layout, for tests.
    pub fn create_in(parent: &Path, session_id: &str) -> std::io::Result<Self> {
        let path = parent.join(format!("cathedral-smart-actors-{session_id}"));
        create_private_dir(&path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Unique per process and per call (`bridge.rs:1289-1296`).
    pub fn new_session_id() -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let counter = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{:x}-{timestamp:x}-{counter:x}", process::id())
    }
}

impl Drop for SessionDir {
    fn drop(&mut self) {
        // Best effort: a leftover temp directory is a nuisance, a panic here
        // during unwinding would be worse.
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// 0700 on unix: nobody else on the machine reads the player's voice.
fn create_private_dir(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(path)?;
    }
    #[cfg(not(unix))]
    fs::create_dir(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parent(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "cathedral-session-dir-{tag}-{}",
            SessionDir::new_session_id()
        ));
        fs::create_dir_all(&path).expect("parent directory");
        path
    }

    #[test]
    fn the_directory_is_private_and_disappears_with_its_handle() {
        let parent = parent("private");
        let path;
        {
            let session = SessionDir::create_in(&parent, "abc123").expect("created");
            path = session.path().to_path_buf();
            assert_eq!(
                path.file_name().expect("name"),
                "cathedral-smart-actors-abc123"
            );
            assert!(path.is_dir());

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = fs::metadata(&path).expect("metadata").permissions().mode();
                assert_eq!(mode & 0o777, 0o700, "{mode:o}");
            }

            fs::write(path.join("speech-1.wav"), b"RIFF").expect("a session file");
        }
        assert!(
            !path.exists(),
            "the directory and its contents go with the handle"
        );
        fs::remove_dir_all(&parent).ok();
    }

    #[test]
    fn creating_the_same_directory_twice_is_an_error() {
        let parent = parent("collision");
        let _first = SessionDir::create_in(&parent, "same").expect("created");
        assert!(
            SessionDir::create_in(&parent, "same").is_err(),
            "create, not create_all: an existing directory is not ours to take over"
        );
        fs::remove_dir_all(&parent).ok();
    }

    #[test]
    fn session_ids_are_unique() {
        let first = SessionDir::new_session_id();
        let second = SessionDir::new_session_id();
        assert_ne!(first, second);
    }
}
