//! Entity ids, as they key the world maps and arrive in untrusted JSON args.
//!
//! Validity (one rule for the whole workspace): non-empty, at most
//! [`MAX_ID_CHARS`] Unicode scalar values, no control characters.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::MAX_ID_CHARS;

/// An id that is empty, too long, or carries control characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidId;

impl fmt::Display for InvalidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "an id must be 1..={MAX_ID_CHARS} characters and free of control characters"
        )
    }
}

impl std::error::Error for InvalidId {}

/// Whether `value` is a well-formed entity id.
pub fn is_valid_id(value: &str) -> bool {
    let mut count = 0usize;
    for character in value.chars() {
        if character.is_control() {
            return false;
        }
        count += 1;
        if count > MAX_ID_CHARS {
            return false;
        }
    }
    count > 0
}

macro_rules! id_newtype {
    ($name:ident, $what:literal) => {
        #[doc = concat!("Stable, opaque identity of ", $what, ".")]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Validating constructor — the only way untrusted input becomes an id.
            pub fn new(value: impl Into<String>) -> Result<Self, InvalidId> {
                let value = value.into();
                if is_valid_id(&value) {
                    Ok(Self(value))
                } else {
                    Err(InvalidId)
                }
            }

            /// Build an id without validating. For seed data and tests only.
            pub fn from_raw(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_string(self) -> String {
                self.0
            }

            /// Whether this id (which may have come from serde, which is
            /// transparent and does not validate) is well-formed.
            pub fn is_valid(&self) -> bool {
                is_valid_id(&self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }
    };
}

id_newtype!(ActorId, "an actor");
id_newtype!(ItemId, "an item");

/// Correlates a cognition submission with its completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(pub u64);

/// A speech event's id (`speech-{sequence}`), typed to stop string-parsing drift.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpeechEventId(pub String);

impl fmt::Display for SpeechEventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_reject_empty_control_chars_and_overlong() {
        assert!(ActorId::new("k0fb1").is_ok());
        assert_eq!(ActorId::new(""), Err(InvalidId));
        assert_eq!(ActorId::new("a\u{0}b"), Err(InvalidId));
        assert_eq!(ActorId::new("a\nb"), Err(InvalidId));
        assert!(ItemId::new("a".repeat(MAX_ID_CHARS)).is_ok());
        assert_eq!(ItemId::new("a".repeat(MAX_ID_CHARS + 1)), Err(InvalidId));
        // Length counts Unicode scalar values, not bytes (D11).
        assert!(ItemId::new("é".repeat(MAX_ID_CHARS)).is_ok());
    }
}
