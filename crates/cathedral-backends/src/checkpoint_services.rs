//! Main-thread forwarding adapters with separately owned Send service storage.
//! Taking the storage makes every remaining adapter inert before its Drop.
use cathedral_sim::checkpoint::complete::ContinuationServices;
use cathedral_sim::*;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};

pub struct SendContinuationServices {
    pub generation: RuntimeGeneration,
    pub cognition: Box<dyn Cognition + Send>,
    pub transcription: Box<dyn Transcription + Send>,
    pub tts: Box<dyn Tts + Send>,
    pub sight: Box<dyn Sight + Send>,
    pub capabilities: Capabilities,
    pub runtime_dir: PathBuf,
}
struct Shared<T: ?Sized>(Rc<RefCell<Option<Box<T>>>>);
impl<T: ?Sized> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}
impl<T: ?Sized> Shared<T> {
    fn new(value: Box<T>) -> Self {
        Self(Rc::new(RefCell::new(Some(value))))
    }
    fn take(&self) -> Box<T> {
        self.0
            .borrow_mut()
            .take()
            .expect("service storage taken once")
    }
}
impl Cognition for Shared<dyn Cognition + Send> {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        self.0
            .borrow_mut()
            .as_mut()
            .ok_or(CognitionBusy)?
            .request(prompt)
    }
    fn request_with_budget(
        &mut self,
        prompt: String,
        budget: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        self.0
            .borrow_mut()
            .as_mut()
            .ok_or(CognitionBusy)?
            .request_with_budget(prompt, budget)
    }
    fn request_night(
        &mut self,
        prompt: String,
        budget: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        self.0
            .borrow_mut()
            .as_mut()
            .ok_or(CognitionBusy)?
            .request_night(prompt, budget)
    }
}
impl Transcription for Shared<dyn Transcription + Send> {
    fn available(&self, kind: SttBackendKind) -> bool {
        self.0.borrow().as_ref().is_some_and(|v| v.available(kind))
    }
    fn submit_batch(
        &mut self,
        job: TranscriptionJobId,
        path: PathBuf,
        kind: SttBackendKind,
    ) -> Result<(), SttSubmitError> {
        self.0
            .borrow_mut()
            .as_mut()
            .ok_or(SttSubmitError::Unavailable)?
            .submit_batch(job, path, kind)
    }
    fn realtime_begin(&mut self, key: &str) -> bool {
        self.0
            .borrow_mut()
            .as_mut()
            .is_some_and(|v| v.realtime_begin(key))
    }
    fn realtime_append(&mut self, key: &str, samples: &[i16]) -> bool {
        self.0
            .borrow_mut()
            .as_mut()
            .is_some_and(|v| v.realtime_append(key, samples))
    }
    fn realtime_commit(&mut self, key: &str) -> bool {
        self.0
            .borrow_mut()
            .as_mut()
            .is_some_and(|v| v.realtime_commit(key))
    }
    fn realtime_clear(&mut self, key: &str) {
        if let Some(v) = self.0.borrow_mut().as_mut() {
            v.realtime_clear(key);
        }
    }
    fn recording_seconds(&self, path: &Path) -> Option<f64> {
        self.0
            .borrow()
            .as_ref()
            .and_then(|v| v.recording_seconds(path))
    }
    fn discard_recording(&mut self, path: &Path) {
        if let Some(v) = self.0.borrow_mut().as_mut() {
            v.discard_recording(path);
        }
    }
}
impl Tts for Shared<dyn Tts + Send> {
    fn available(&self, kind: TtsBackendKind) -> bool {
        self.0.borrow().as_ref().is_some_and(|v| v.available(kind))
    }
    fn submit(&mut self, request: TtsRequest) -> Result<(), TtsSubmitError> {
        self.0
            .borrow_mut()
            .as_mut()
            .ok_or(TtsSubmitError::Unavailable)?
            .submit(request)
    }
    fn warm(&mut self, kind: TtsBackendKind) {
        if let Some(v) = self.0.borrow_mut().as_mut() {
            v.warm(kind);
        }
    }
}
impl Sight for Shared<dyn Sight + Send> {
    fn line_of_sight(&self, from: Vec3, to: Vec3) -> bool {
        self.0
            .borrow()
            .as_ref()
            .is_some_and(|v| v.line_of_sight(from, to))
    }
    fn npc_pov_frame(&mut self, actor: &ActorId) -> Option<PovFrame> {
        self.0
            .borrow_mut()
            .as_mut()
            .and_then(|v| v.npc_pov_frame(actor))
    }
}
/// All allocation/captured service inputs must be admitted before construction.
/// This is not a backend startup/probe API. Existing services are moved once.
pub struct ForwardingServices {
    cognition: Shared<dyn Cognition + Send>,
    transcription: Shared<dyn Transcription + Send>,
    tts: Shared<dyn Tts + Send>,
    sight: Shared<dyn Sight + Send>,
    generation: RuntimeGeneration,
    capabilities: Capabilities,
    runtime_dir: PathBuf,
}
impl ForwardingServices {
    pub fn new(s: SendContinuationServices) -> Self {
        Self {
            cognition: Shared::new(s.cognition),
            transcription: Shared::new(s.transcription),
            tts: Shared::new(s.tts),
            sight: Shared::new(s.sight),
            generation: s.generation,
            capabilities: s.capabilities,
            runtime_dir: s.runtime_dir,
        }
    }
    pub fn adapters(&self) -> ContinuationServices {
        ContinuationServices {
            generation: self.generation,
            cognition: Box::new(self.cognition.clone()),
            transcription: Box::new(self.transcription.clone()),
            tts: Box::new(self.tts.clone()),
            sight: Box::new(self.sight.clone()),
            capabilities: self.capabilities,
            runtime_dir: self.runtime_dir.clone(),
        }
    }
    /// Infallible ownership move after preflight, without destroying services.
    pub fn into_send(self) -> SendContinuationServices {
        SendContinuationServices {
            generation: self.generation,
            cognition: self.cognition.take(),
            transcription: self.transcription.take(),
            tts: self.tts.take(),
            sight: self.sight.take(),
            capabilities: self.capabilities,
            runtime_dir: self.runtime_dir,
        }
    }
}
