//! The same LoadCandidate lease survives host construction and cancellation.
//! No retiring slot is required to return an inactive candidate to its worker.
use super::*;
use crate::engine::{
    RetiredEngineState, cognition_inputs_checkpoint::EngineCognitionInputsCandidate,
    speech_checkpoint::EngineSpeechCandidate,
};

/// Opaque Send disposal owner. It cannot be adopted, polled, decoded or borrowed
/// as a World. All semantic/asset/service charges drop after their real owners.
#[allow(dead_code)]
pub struct CandidateDisposal {
    engine: RetiredEngineState,
    speech: Option<EngineSpeechCandidate>,
    host: super::super::host::HostCandidate,
    cognition: Option<EngineCognitionInputsCandidate>,
    asset_reservation: Reservation,
    service_reservation: Option<Reservation>,
}
impl Admitted<HydratedEngine> {
    /// Construction uses inert adapters. Dispose them on this host thread and
    /// move every large domain/asset owner, with its original load lease.
    pub fn into_disposal(self) -> Admitted<CandidateDisposal> {
        self.map(|h| {
            let (engine, adapters) = h.engine.into_retirement();
            adapters.dispose();
            CandidateDisposal {
                engine,
                speech: Some(h.speech),
                host: h.host,
                cognition: Some(h.cognition),
                asset_reservation: h.asset_reservation,
                service_reservation: None,
            }
        })
    }
}
impl Admitted<PreparedContinuation> {
    /// The production host must first detach the actual Send services from its
    /// forwarding owners. Only the now-empty adapters are dropped here. The
    /// existing arbitrary trait-object API has no universal destructor bound.
    pub fn into_disposal(self) -> Admitted<CandidateDisposal> {
        self.map(|p| {
            let (engine, adapters) = p.engine.into_retirement();
            adapters.dispose();
            drop(p.failed_services);
            CandidateDisposal {
                engine,
                speech: None,
                host: p.host,
                cognition: None,
                asset_reservation: p.asset_reservation,
                service_reservation: p.service_reservation,
            }
        })
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn cancelled_candidate_disposal_is_send() {
        fn send<T: Send>() {}
        send::<super::Admitted<super::CandidateDisposal>>();
    }
}
