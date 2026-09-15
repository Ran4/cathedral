//! Opaque host archive admission travels with the actual immutable exchange.
//! No archive IO or execution identity is part of saved simulation authority.
use crate::ActorId;
use std::{any::Any, fmt, sync::Arc};

/// Host-owned allocation retention. Drop must only release admission, never IO.
pub trait PromptArchiveRetention: Any + fmt::Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;
}

#[derive(Default)]
pub struct PromptArchivePermit(Option<Arc<dyn PromptArchiveRetention>>);
impl PromptArchivePermit {
    pub fn new(owner: Arc<dyn PromptArchiveRetention>) -> Self {
        Self(Some(owner))
    }
    pub fn owner<T: Any>(&self) -> Option<&T> {
        self.0.as_ref()?.as_any().downcast_ref()
    }
    pub fn enabled(&self) -> bool {
        self.0.is_some()
    }
}
impl fmt::Debug for PromptArchivePermit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PromptArchivePermit")
            .field(&self.enabled())
            .finish()
    }
}

/// Caller-owned input. It becomes opaque when paired with its unique permit.
#[derive(Debug, PartialEq)]
pub struct PromptExchangeData {
    pub actor_id: ActorId,
    pub actor_name: String,
    pub prompt: String,
    pub answer: Option<String>,
    pub duration_seconds: f64,
    pub error: Option<String>,
}
/// Shared event clones retain the same strings. No mutable dereference or
/// extraction exists, even when Arc::get_mut finds a unique wrapper.
#[derive(Debug)]
pub struct PromptExchange {
    data: PromptExchangeData,
    admission: PromptArchivePermit,
}
impl PromptExchange {
    pub fn new(data: PromptExchangeData, admission: PromptArchivePermit) -> Arc<Self> {
        Arc::new(Self { data, admission })
    }
    pub fn admission(&self) -> &PromptArchivePermit {
        &self.admission
    }
}
impl std::ops::Deref for PromptExchange {
    type Target = PromptExchangeData;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}
impl PartialEq for PromptExchange {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::{Cognition, CognitionBusy, RequestId};
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Debug)]
    pub struct Token(pub Arc<AtomicUsize>);
    impl PromptArchiveRetention for Token {
        fn as_any(&self) -> &dyn Any {
            self
        }
    }
    impl Drop for Token {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }
    #[derive(Default)]
    pub struct Calls {
        pub requests: Vec<(bool, String)>,
        pub attempts: usize,
        pub refused: bool,
    }
    pub struct Probe {
        pub limit: usize,
        pub retained: Arc<AtomicUsize>,
        pub calls: Arc<Mutex<Calls>>,
    }
    impl Probe {
        pub fn new(limit: usize) -> Self {
            Self {
                limit,
                retained: Default::default(),
                calls: Default::default(),
            }
        }
        pub fn retained(&self) -> usize {
            self.retained.load(Ordering::SeqCst)
        }
    }
    impl Cognition for Probe {
        fn reserve_prompt_archive(
            &mut self,
            _: usize,
            _: usize,
        ) -> Result<PromptArchivePermit, CognitionBusy> {
            if self.retained() >= self.limit {
                return Err(CognitionBusy);
            }
            self.retained.fetch_add(1, Ordering::SeqCst);
            Ok(PromptArchivePermit::new(Arc::new(Token(Arc::clone(
                &self.retained,
            )))))
        }
        fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
            let mut calls = self.calls.lock().unwrap();
            calls.attempts += 1;
            if calls.refused {
                return Err(CognitionBusy);
            }
            calls.requests.push((false, prompt));
            Ok(RequestId(calls.requests.len() as u64))
        }
        fn request_night(
            &mut self,
            prompt: String,
            _: Option<u32>,
        ) -> Result<RequestId, CognitionBusy> {
            let mut calls = self.calls.lock().unwrap();
            calls.attempts += 1;
            if calls.refused {
                return Err(CognitionBusy);
            }
            calls.requests.push((true, prompt));
            Ok(RequestId(calls.requests.len() as u64))
        }
    }
}
