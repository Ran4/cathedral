//! Exact immutable catalog binding; no embedded fallback on incompatibility.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Write;
impl ItemCatalog {
    pub fn checkpoint_fingerprint(&self) -> [u8; 32] {
        struct Sink(Sha256);
        impl Write for Sink {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.update(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut sink = Sink(Sha256::new());
        sink.0.update(b"cathedral-item-catalog-checkpoint-v1\0");
        // Definitions are immutable, validated at construction, sorted by kind.
        serde_json::to_writer(&mut sink, &(self.schema_version, &self.kinds))
            .expect("catalog serializes to an infallible hash sink");
        sink.0.finalize().into()
    }
}
