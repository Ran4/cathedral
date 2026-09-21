//! Startup staging has no access to the live App. Only a fully constructed
//! shared owner can be installed; a failure drops all staged owners.
use super::*;
use bevy::prelude::*;
use cathedral_backends::{
    BackendRuntime, BackendsConfig, BackendsHandle, BackendsOptions,
    prompt_log::{ArchiveWriter, LocalTime, PromptLog},
};
use serde::{
    Deserialize,
    de::{self, SeqAccess, Visitor},
};

const NAV_JSON: &str = include_str!("../../assets/world/navigation.json");
const NAV_BIN: &[u8] = include_bytes!("../../assets/world/navigation.bin");
/// Construction scratch for these immutable embedded artifacts only. The
/// current artifact is measured in the owner test; the complete dynamic cache
/// has a separate exact structural maximum reserved before parsing.
const NAV_PARSE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupRefusal {
    Admission,
    Navigation,
    Services,
    AlreadyInstalled,
    UnprovedWholeAppAccounting,
}

/// One immutable committed recipe identity, retained by every actual consumer.
/// Backend source/config and non-navigation asset allocation are not yet a
/// complete-world proof; no checkpoint adoption permission is minted here.
#[derive(Resource, Clone)]
pub(crate) struct CommittedStartup(Arc<Committed>);
struct Committed {
    nav: Arc<NavData>,
    backend_config: Arc<BackendsConfig>,
    runtime: Arc<BackendRuntime>,
    prompt_log: std::sync::Mutex<PromptLog>,
    nav_lease: Reservation,
    // Its allowance includes the bounded startup setting copies above, so
    // release it only after shared services and their settings are disposed.
    config: InstalledConfig,
    installed: InstalledRecipe,
}
/// The preparation worker is owned separately from the recipe. A retired
/// payload may retain the recipe on that worker; it must never thereby own a
/// join guard for its own disposal thread.
pub(crate) struct StagedStartup {
    recipe: CommittedStartup,
    #[cfg(target_os = "linux")]
    preparation: cathedral_backends::checkpoint_preparation::CheckpointPreparation,
}
#[cfg(target_os = "linux")]
#[derive(Resource)]
struct InstalledPreparation {
    _worker: cathedral_backends::checkpoint_preparation::CheckpointPreparation,
}

impl StagedStartup {
    pub fn prepare(
        installed: InstalledRecipe,
        config: InstalledConfig,
    ) -> Result<Self, StartupRefusal> {
        Self::prepare_with(installed, config, |config| {
            BackendsConfig::load(&BackendsOptions {
                uv_binary: config.smart_actors.uv_binary.clone(),
                fake_mode: config.smart_actors.fake_backend,
                ..BackendsOptions::default()
            })
        })
    }
    pub(crate) fn prepare_with(
        installed: InstalledRecipe,
        config: InstalledConfig,
        backend: impl FnOnce(&crate::config::AppConfig) -> BackendsConfig,
    ) -> Result<Self, StartupRefusal> {
        Self::prepare_with_directory(
            installed,
            config,
            backend,
            crate::session_log::paths().map(|session| session.root.join("prompts")),
        )
    }
    fn prepare_with_directory(
        installed: InstalledRecipe,
        config: InstalledConfig,
        backend: impl FnOnce(&crate::config::AppConfig) -> BackendsConfig,
        directory: Option<std::path::PathBuf>,
    ) -> Result<Self, StartupRefusal> {
        if !installed
            .budget()
            .owns_reservation(&config._lease, Cohort::Running)
        {
            return Err(StartupRefusal::Admission);
        }
        let nodes: NavNodes =
            serde_json::from_str(NAV_JSON).map_err(|_| StartupRefusal::Navigation)?;
        let count = nodes.nodes.0;
        let cache = count
            .checked_mul(size_of::<std::sync::OnceLock<Box<[f32]>>>())
            .and_then(|n| n.checked_add(64))
            .and_then(|n| {
                count
                    .checked_mul(count.checked_mul(4)?.checked_add(32)?)
                    .and_then(|rows| n.checked_add(rows))
            })
            .ok_or(StartupRefusal::Admission)?;
        let mut nav_lease = installed
            .budget()
            .reserve_running_overhead(
                NAV_PARSE_BYTES
                    .checked_add(cache)
                    .ok_or(StartupRefusal::Admission)?,
            )
            .map_err(|_| StartupRefusal::Admission)?;
        let nav = Arc::new(
            NavData::from_parts(NAV_JSON, NAV_BIN).map_err(|_| StartupRefusal::Navigation)?,
        );
        let cost = InstalledCost::navigation([Some(&nav), None])
            .total()
            .map_err(|_| StartupRefusal::Admission)?;
        if cost > nav_lease.bytes() {
            return Err(StartupRefusal::Admission);
        }
        nav_lease
            .resize(cost)
            .map_err(|_| StartupRefusal::Admission)?;
        let backend_config = Arc::new(backend(&config));
        let runtime = BackendRuntime::start_admitted(installed.budget())
            .map_err(|_| StartupRefusal::Services)?;
        let archive = ArchiveWriter::start_admitted(installed.budget())
            .map_err(|_| StartupRefusal::Services)?;
        let model = if backend_config.fake_mode {
            Some("fake".to_owned())
        } else {
            backend_config.model_name()
        };
        let prompt_log = std::sync::Mutex::new(PromptLog::with_writer(
            directory,
            model,
            Box::new(LocalTime::now),
            archive,
        ));
        #[cfg(target_os = "linux")]
        let preparation = cathedral_backends::checkpoint_preparation::CheckpointPreparation::start(
            installed.budget().clone(),
        )
        .map_err(|_| StartupRefusal::Services)?;
        Ok(Self {
            recipe: CommittedStartup(Arc::new(Committed {
                config,
                nav,
                backend_config,
                runtime,
                prompt_log,
                nav_lease,
                installed,
            })),
            #[cfg(target_os = "linux")]
            preparation,
        })
    }
    pub fn recipe(&self) -> &CommittedStartup {
        &self.recipe
    }
    /// Startup-only publication. Duplicate installation refuses before any
    /// resource is changed. This does not replace a running world.
    pub fn install(self, app: &mut App) -> Result<(), (StartupRefusal, Self)> {
        if app.world().contains_resource::<CommittedStartup>()
            || app.is_plugin_added::<crate::smart_actors::SmartActorsPlugin>()
            || app.is_plugin_added::<crate::nav_overlay::NavDebugPlugin>()
        {
            return Err((StartupRefusal::AlreadyInstalled, self));
        }
        app.insert_resource(self.recipe);
        #[cfg(target_os = "linux")]
        app.insert_resource(InstalledPreparation {
            _worker: self.preparation,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests;
impl CommittedStartup {
    /// No caller-supplied upper bound can authorize whole-App staging. The
    /// installed non-nav assets, backend transport/config copies and mutable
    /// live/candidate/retired authority have no accepted complete profile yet.
    pub fn require_complete_admission(&self) -> Result<(), StartupRefusal> {
        Err(StartupRefusal::UnprovedWholeAppAccounting)
    }
    pub fn config(&self) -> &crate::config::AppConfig {
        &self.0.config
    }
    pub fn nav(&self) -> &Arc<NavData> {
        &self.0.nav
    }
    pub fn budget(&self) -> &Arc<CheckpointBudget> {
        self.0.installed.budget()
    }
    pub fn navigation_bytes(&self) -> usize {
        self.0.nav_lease.bytes()
    }
    pub fn backends(
        &self,
        session: Option<cathedral_backends::SessionDir>,
        generation: cathedral_sim::RuntimeGeneration,
    ) -> BackendsHandle {
        BackendsHandle::with_shared_runtime(
            self.0.runtime.clone(),
            self.0.backend_config.clone(),
            session,
            generation,
        )
    }
    pub fn prompt_log(&self) -> PromptLog {
        self.0
            .prompt_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .fork()
    }
}

// Count the node array without retaining decoded rows or an arbitrary JSON
// Value. The embedded document is immutable across this preparation.
#[derive(Deserialize)]
struct NavNodes {
    nodes: Count,
}
struct Count(usize);
impl<'de> Deserialize<'de> for Count {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Counter;
        impl<'de> Visitor<'de> for Counter {
            type Value = Count;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Count, A::Error> {
                let mut n = 0usize;
                while a.next_element::<de::IgnoredAny>()?.is_some() {
                    n = n
                        .checked_add(1)
                        .ok_or_else(|| de::Error::custom("node count overflow"))?;
                }
                Ok(Count(n))
            }
        }
        d.deserialize_seq(Counter)
    }
}
