//! One diagnostic readback/encode at a time. This does not admit renderer or
//! whole-world checkpoint memory; those acceptance gates remain closed.
use super::*;
use bevy::{ecs::system::SystemParam, render::renderer::RenderDevice, window::PrimaryWindow};
use cathedral_sim::checkpoint::{CheckpointBudget, Reservation};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, Ordering},
};
use std::time::{Duration, Instant};

const REQUEST_BYTES: usize = 4096;
const MAX_DIMENSION: u32 = 4096;
const MAX_PIXELS: usize = 4096 * 2160;
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    Startup,
    Control,
    Renderer,
    Window,
    Extent,
    Busy,
    Path,
    Execution,
    FullWorld,
    RendererBudget,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Outcome {
    Pending,
    Encoding,
    Saved,
    InvalidImage,
    WriteFailed,
    TimedOut,
}

#[derive(Debug)]
struct Completion {
    state: AtomicU8,
    deadline: Instant,
    slot: Arc<Slot>,
}
#[derive(Clone, Debug)]
pub(crate) struct Receipt(Arc<Completion>);
impl Receipt {
    pub(crate) fn outcome(&self) -> Outcome {
        self.at(Instant::now())
    }
    fn at(&self, now: Instant) -> Outcome {
        loop {
            let value = self.0.state.load(Ordering::Acquire);
            if value <= Outcome::Encoding as u8 && now >= self.0.deadline {
                if self
                    .0
                    .state
                    .compare_exchange(
                        value,
                        Outcome::TimedOut as u8,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_err()
                {
                    continue;
                }
                return Outcome::TimedOut;
            }
            return match value {
                0 => Outcome::Pending,
                1 => Outcome::Encoding,
                2 => Outcome::Saved,
                3 => Outcome::InvalidImage,
                4 => Outcome::WriteFailed,
                _ => Outcome::TimedOut,
            };
        }
    }
    fn finish(&self, outcome: Outcome) {
        if self.outcome() == Outcome::Encoding {
            let _ = self.0.state.compare_exchange(
                Outcome::Encoding as u8,
                outcome as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }
}
#[derive(Debug)]
struct Slot {
    busy: AtomicBool,
    _lease: Reservation,
}
struct Flight {
    receipt: Receipt,
}
impl Drop for Flight {
    fn drop(&mut self) {
        self.receipt.0.slot.busy.store(false, Ordering::Release);
    }
}

#[derive(Resource)]
pub(crate) struct Requests {
    slot: Arc<Slot>,
}
impl Requests {
    pub(crate) fn admitted(
        budget: &Arc<CheckpointBudget>,
    ) -> cathedral_sim::checkpoint::Result<Self> {
        let lease = budget.reserve_running_overhead(REQUEST_BYTES)?;
        Ok(Self {
            slot: Arc::new(Slot {
                busy: AtomicBool::new(false),
                _lease: lease,
            }),
        })
    }
    fn begin(&self, now: Instant) -> Result<Arc<Flight>, Refusal> {
        self.slot
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| Refusal::Busy)?;
        Ok(Arc::new(Flight {
            receipt: Receipt(Arc::new(Completion {
                state: AtomicU8::new(Outcome::Pending as u8),
                deadline: now + TIMEOUT,
                slot: self.slot.clone(),
            })),
        }))
    }
}

#[derive(SystemParam)]
pub(crate) struct CaptureContext<'w, 's> {
    requests: Option<Res<'w, Requests>>,
    startup: Option<Res<'w, crate::installed_recipe::CommittedStartup>>,
    controls: Option<Res<'w, crate::checkpoint_controls::CheckpointControls>>,
    bridge: Option<Res<'w, crate::smart_actors::bridge::BridgeHandle>>,
    engine: Option<NonSend<'w, crate::smart_actors::local_engine::LocalEngine>>,
    renderer: Option<Res<'w, RenderDevice>>,
    environment: Option<Res<'w, FrameEnvironment>>,
    windows: Query<'w, 's, (Entity, &'static Window), With<PrimaryWindow>>,
}
#[derive(Resource)]
pub(crate) struct FrameEnvironment {
    pub headless: bool,
    pub audio_disabled: bool,
}
impl CaptureContext<'_, '_> {
    pub(crate) fn window(&self) -> Option<Entity> {
        self.windows.single().ok().map(|w| w.0)
    }
    fn authority(&self) -> Result<(), Refusal> {
        let startup = self.startup.as_deref().ok_or(Refusal::Startup)?;
        if !self
            .controls
            .as_ref()
            .is_some_and(|c| c.belongs_to(startup))
        {
            return Err(Refusal::Control);
        }
        if startup.config().smart_actors.enabled
            && !self
                .engine
                .as_deref()
                .zip(self.bridge.as_deref())
                .is_some_and(|(engine, bridge)| engine.owns_control(bridge, startup))
        {
            return Err(Refusal::Control);
        }
        Ok(())
    }
    /// PNG completion is diagnostic evidence, never a whole-App frame pass.
    #[allow(
        dead_code,
        reason = "closed full-frame acceptance gate; diagnostics do not confer acceptance"
    )]
    pub(crate) fn checkpoint_acceptance(&self) -> Result<(), Refusal> {
        self.authority()?;
        if !self
            .environment
            .as_ref()
            .is_some_and(|mode| mode.headless && mode.audio_disabled)
            || !self
                .windows
                .single()
                .is_ok_and(|(_, window)| !window.visible)
            || !self
                .startup
                .as_ref()
                .unwrap()
                .config()
                .smart_actors
                .fake_backend
            || self
                .startup
                .as_ref()
                .unwrap()
                .config()
                .smart_actors
                .tts_backend
                != "off"
        {
            return Err(Refusal::Execution);
        }
        if self.renderer.is_none() {
            return Err(Refusal::Renderer);
        }
        self.startup
            .as_ref()
            .unwrap()
            .require_complete_admission()
            .map_err(|_| Refusal::FullWorld)?;
        Err(Refusal::RendererBudget)
    }
    pub(crate) fn submit(
        &self,
        commands: &mut Commands,
        path: PathBuf,
    ) -> Result<Receipt, Refusal> {
        self.authority()?;
        if self.renderer.is_none() {
            return Err(Refusal::Renderer);
        }
        let (window, info) = self.windows.single().map_err(|_| Refusal::Window)?;
        check_extent(info.physical_width(), info.physical_height())?;
        if path.capacity() > 2048 {
            return Err(Refusal::Path);
        }
        let requests = self.requests.as_ref().ok_or(Refusal::Startup)?;
        if !self.startup.as_ref().unwrap().budget().owns_reservation(
            &requests.slot._lease,
            cathedral_sim::checkpoint::Cohort::Running,
        ) {
            return Err(Refusal::Startup);
        }
        let flight = requests.begin(Instant::now())?;
        let receipt = flight.receipt.clone();
        commands
            .spawn(Screenshot::window(window))
            .observe(observer(path, flight));
        Ok(receipt)
    }
}

fn check_extent(width: u32, height: u32) -> Result<usize, Refusal> {
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or(Refusal::Extent)?;
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || pixels > MAX_PIXELS
    {
        return Err(Refusal::Extent);
    }
    Ok(pixels)
}
fn valid_image(image: &Image) -> bool {
    use bevy::render::render_resource::{TextureDimension, TextureFormat};
    let d = &image.texture_descriptor;
    let Ok(pixels) = check_extent(d.size.width, d.size.height) else {
        return false;
    };
    d.dimension == TextureDimension::D2
        && d.size.depth_or_array_layers == 1
        && d.mip_level_count == 1
        && d.sample_count == 1
        && matches!(
            d.format,
            TextureFormat::Rgba8UnormSrgb
                | TextureFormat::Bgra8Unorm
                | TextureFormat::Bgra8UnormSrgb
        )
        && image.data.as_ref().is_some_and(|data| {
            data.len() == pixels * 4
                && data.capacity() <= MAX_PIXELS * 4 + 256 * MAX_DIMENSION as usize
        })
}
fn copy_pixels(image: &Image) -> Option<Image> {
    if !valid_image(image) {
        return None;
    }
    // Image::clone also copies an arbitrary owned sampler label. Screenshots
    // only need these validated pixels and their closed format/extent.
    Some(Image::new(
        image.texture_descriptor.size,
        bevy::render::render_resource::TextureDimension::D2,
        image.data.as_ref()?.clone(),
        image.texture_descriptor.format,
        bevy::asset::RenderAssetUsages::MAIN_WORLD,
    ))
}
fn observer(path: PathBuf, flight: Arc<Flight>) -> impl FnMut(On<ScreenshotCaptured>) {
    let mut owned = Some((path, flight));
    move |captured| {
        let Some((path, flight)) = owned.take() else {
            return;
        };
        if flight.receipt.outcome() != Outcome::Pending {
            return;
        }
        if flight
            .receipt
            .0
            .state
            .compare_exchange(
                Outcome::Pending as u8,
                Outcome::Encoding as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return;
        }
        let Some(image) = copy_pixels(&captured.image) else {
            flight.receipt.finish(Outcome::InvalidImage);
            return;
        };
        AsyncComputeTaskPool::get()
            .spawn(async move {
                let result = write_image(image, &path);
                flight.receipt.finish(if result.is_ok() {
                    Outcome::Saved
                } else {
                    Outcome::WriteFailed
                });
                if let Err(error) = result {
                    error!("Screenshot write failed: {error}");
                }
                // Actual image/encoder disposal precedes releasing the busy slot.
                drop(flight);
            })
            .detach();
    }
}
fn write_image(image: Image, path: &std::path::Path) -> Result<(), String> {
    let dynamic = image.try_into_dynamic().map_err(|e| e.to_string())?;
    dynamic.to_rgb8().save(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
