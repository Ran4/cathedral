//! Finite native name resolution shared by one backend runtime.
//!
//! Reqwest's default resolver and Tokio's string ToSocketAddrs spawn blocking
//! work that survives future cancellation. Every production HTTP/websocket
//! path instead uses this two-slot resolver. A slot includes queued/running
//! calls and retained answers, not merely the asynchronous waiter.

use crate::BackendSender;
use cathedral_sim::checkpoint::Reservation;
use std::{
    io,
    net::{SocketAddr, ToSocketAddrs},
    sync::{Arc, OnceLock},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub const DNS_SLOTS: usize = 2;
pub const MAX_DNS_HOST_BYTES: usize = 253;
pub const MAX_DNS_ADDRESSES: usize = 32;

#[derive(Debug)]
pub(crate) struct DnsPool {
    slots: Arc<Semaphore>,
    // Shared runtime/native scope; actual generation endpoints are separate.
    _lease: Option<Reservation>,
}
impl DnsPool {
    pub(crate) fn new(lease: Option<Reservation>) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(DNS_SLOTS)),
            _lease: lease,
        }
    }
}

#[derive(Clone, Debug)]
pub struct NativeResolver {
    pool: Arc<DnsPool>,
    events: Option<BackendSender>,
}
struct DnsPermit {
    _events: Option<BackendSender>,
    _slot: OwnedSemaphorePermit,
    _pool: Arc<DnsPool>,
}
/// Queued cancellation drops the actual host/operation before its admission.
/// Drop prevents a native closure from capturing these fields independently.
struct DnsWork {
    host: String,
    resolve: Option<Box<dyn FnOnce(String) -> io::Result<Vec<SocketAddr>> + Send>>,
    permit: Option<DnsPermit>,
}
impl Drop for DnsWork {
    fn drop(&mut self) {}
}
impl DnsWork {
    fn run(mut self) -> io::Result<Answer> {
        let addresses = self.resolve.take().unwrap()(std::mem::take(&mut self.host))?;
        if addresses.capacity() > MAX_DNS_ADDRESSES {
            return Err(io::Error::other("DNS answer exceeds limit"));
        }
        Ok(Answer {
            addresses: addresses.into_iter(),
            _permit: self.permit.take().unwrap(),
        })
    }
}

struct Answer {
    addresses: std::vec::IntoIter<SocketAddr>,
    _permit: DnsPermit,
}
impl Iterator for Answer {
    type Item = SocketAddr;
    fn next(&mut self) -> Option<SocketAddr> {
        self.addresses.next()
    }
}
impl NativeResolver {
    pub(crate) fn new(pool: Arc<DnsPool>, events: Option<BackendSender>) -> Self {
        Self { pool, events }
    }
    /// Standalone clients have no world owner; their common fallback pool is
    /// still finite. Production generation clients use BackendRuntime::resolver.
    pub(crate) fn standalone() -> Self {
        static POOL: OnceLock<Arc<DnsPool>> = OnceLock::new();
        Self::new(
            Arc::clone(POOL.get_or_init(|| Arc::new(DnsPool::new(None)))),
            None,
        )
    }
    pub async fn lookup(&self, host: &str, port: u16) -> io::Result<reqwest::dns::Addrs> {
        if host.is_empty() || host.len() > MAX_DNS_HOST_BYTES {
            return Err(io::Error::other("DNS host exceeds limit"));
        }
        self.run(host.to_owned(), move |host| {
            // libc resolver allocations during this call are a trusted native
            // allowance. Only this bounded address list enters game ownership.
            Ok((host.as_str(), port)
                .to_socket_addrs()?
                .take(MAX_DNS_ADDRESSES)
                .collect())
        })
        .await
    }
    pub(crate) async fn run<F>(&self, host: String, resolve: F) -> io::Result<reqwest::dns::Addrs>
    where
        F: FnOnce(String) -> io::Result<Vec<SocketAddr>> + Send + 'static,
    {
        if host.capacity() > MAX_DNS_HOST_BYTES
            || self.events.as_ref().is_some_and(|e| !e.is_active())
        {
            return Err(io::Error::other("DNS host rejected"));
        }
        let slot = Arc::clone(&self.pool.slots)
            .try_acquire_owned()
            .map_err(|_| {
                io::Error::new(io::ErrorKind::WouldBlock, "DNS work capacity exhausted")
            })?;
        let permit = DnsPermit {
            _slot: slot,
            _events: self.events.clone(),
            _pool: Arc::clone(&self.pool),
        };
        let work = DnsWork {
            host,
            resolve: Some(Box::new(resolve)),
            permit: Some(permit),
        };
        let answer = tokio::task::spawn_blocking(move || work.run())
            .await
            .map_err(io::Error::other)??;
        Ok(Box::new(answer))
    }
}
impl reqwest::dns::Resolve for NativeResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let resolver = self.clone();
        Box::pin(async move { resolver.lookup(name.as_str(), 0).await.map_err(Into::into) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BackendRuntime, backend_channel};
    fn address() -> SocketAddr {
        "127.0.0.1:0".parse().unwrap()
    }

    #[test]
    fn dns_answer_retains_capacity_until_actual_iterator_disposal() {
        let runtime = BackendRuntime::new().unwrap();
        let (events, _) = backend_channel();
        let resolver = runtime.resolver(events);
        runtime.block_on(async {
            let first = resolver
                .run("first".into(), |_| Ok(vec![address()]))
                .await
                .unwrap();
            let second = resolver
                .run("second".into(), |_| Ok(vec![address()]))
                .await
                .unwrap();
            let refused = resolver
                .run("refused".into(), |_| panic!("no resolver call on refusal"))
                .await;
            assert!(matches!(refused, Err(ref e) if e.kind() == io::ErrorKind::WouldBlock));
            drop(first);
            let third = resolver
                .run("third".into(), |_| Ok(vec![address()]))
                .await
                .unwrap();
            assert_eq!(third.count(), 1);
            drop(second);
        });
    }

    #[test]
    fn dns_rejects_retained_host_and_answer_capacity_before_keeping_it() {
        let runtime = BackendRuntime::new().unwrap();
        let (events, _) = backend_channel();
        let resolver = runtime.resolver(events);
        runtime.block_on(async {
            let mut host = String::with_capacity(MAX_DNS_HOST_BYTES + 1);
            host.push_str("tiny");
            assert!(
                resolver
                    .run(host, |_| panic!("oversized retained host"))
                    .await
                    .is_err()
            );
            assert!(
                resolver
                    .lookup(&"a".repeat(MAX_DNS_HOST_BYTES + 1), 0)
                    .await
                    .is_err()
            );
            assert!(
                resolver
                    .run("answer".into(), |_| Ok(Vec::with_capacity(
                        MAX_DNS_ADDRESSES + 1
                    )))
                    .await
                    .is_err()
            );
            let valid = resolver
                .run("valid".into(), |_| Ok(vec![address()]))
                .await
                .unwrap();
            assert_eq!(valid.count(), 1);
        });
    }
}
