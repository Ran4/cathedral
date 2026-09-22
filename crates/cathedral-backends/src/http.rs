//! Shared provider HTTP retention policy. This bounds idle connection storage,
//! not active transport buffers or complete per-generation allocation.

/// Preserve reqwest 0.12.28's supported redirect behavior explicitly. A request
/// can visit at most eleven destinations (initial plus ten redirects).
const MAX_REDIRECTS: usize = 10;

pub(crate) fn builder(resolver: crate::dns::NativeResolver) -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .dns_resolver(std::sync::Arc::new(resolver))
        // Locked hyper-util 0.1.20 makes Pool::inner None when this is zero:
        // there is no retained idle/connecting/waiter map across destinations.
        // A positive per-host cap would not bound the number of redirect hosts.
        // Each request pays a new connection/TLS handshake; HTTP/2 remains
        // supported, but cross-request connection reuse is deliberately lost.
        .pool_max_idle_per_host(0)
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
}

#[cfg(test)]
mod tests;
