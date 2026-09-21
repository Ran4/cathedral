//! Admission for installed startup inputs. This is not a reservation for a
//! mutable World, renderer, or a second 512 MiB authority root.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "Source retention and Nav inventory are the installed factory seam; only startup control, config and diagnostics are integrated in this cut."
    )
)]
use cathedral_sim::{
    NavData,
    checkpoint::{self, CheckpointBudget, Cohort, Reservation},
};
use std::{
    mem::size_of,
    ops::{Deref, DerefMut},
    path::Path,
    sync::Arc,
};

/// The budget's fixed table, this owner and Arc controls. This bootstrap root
/// deliberately cannot satisfy complete checkpoint's 512 MiB Running gate.
const CONTROL_BYTES: usize = 4096;
/// 64 KiB input+sentinel, parser/error scratch, the parsed configuration and
/// sixteen simultaneously retained startup copies of its string leaves. The
/// concrete AppConfig has eight String leaves and no arbitrary collection.
/// This covers the main/plugin/resource startup copies, not backend env/keys,
/// mutable World state or renderer resources.
const CONFIG_BYTES: usize = 4 * 1024 * 1024;
fn error(reason: &'static str) -> checkpoint::CheckpointError {
    checkpoint::CheckpointError {
        owner: "installed",
        reason: reason.into(),
    }
}

/// A closed inventory: cache rows are counted at their maximum supported
/// warmth, independently from graph allocations and from recipe source bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct InstalledCost {
    pub recipe_bytes: usize,
    pub navigation_graph_bytes: usize,
    pub navigation_cache_bytes: usize,
}
impl InstalledCost {
    pub fn total(self) -> checkpoint::Result<usize> {
        self.recipe_bytes
            .checked_add(self.navigation_graph_bytes)
            .and_then(|n| n.checked_add(self.navigation_cache_bytes))
            .ok_or_else(|| error("inventory overflow"))
    }

    /// Borrowing-only inventory for the two actual navigation roles. A deep
    /// NavData clone has a separate graph but shares its distance cache; two
    /// independently parsed graphs share neither. No cache is warmed here.
    pub fn navigation(roles: [Option<&Arc<NavData>>; 2]) -> Self {
        let mut out = Self::default();
        for (index, nav) in roles.into_iter().enumerate() {
            let Some(nav) = nav else { continue };
            let inventory = nav.checkpoint_storage_inventory();
            let earlier = (index == 1).then_some(roles[0]).flatten();
            if !earlier.is_some_and(|first| Arc::ptr_eq(first, nav)) {
                out.navigation_graph_bytes = out
                    .navigation_graph_bytes
                    .saturating_add(inventory.graph_and_indexes_bytes.saturating_add(64));
            }
            if !earlier.is_some_and(|first| first.checkpoint_shares_distance_cache(nav)) {
                out.navigation_cache_bytes = out
                    .navigation_cache_bytes
                    .saturating_add(inventory.cache_maximum_bytes);
            }
        }
        out
    }
}

/// One shared startup budget. Its root covers only fixed startup control;
/// admitted services and retained recipe inputs own disjoint subordinate
/// charges. No caller-supplied whole-world estimate is hidden in this type.
pub(crate) struct InstalledRecipe {
    budget: Arc<CheckpointBudget>,
    _control: Reservation,
}
impl InstalledRecipe {
    pub fn new() -> checkpoint::Result<Self> {
        let budget = Arc::new(CheckpointBudget::default());
        let control = budget.reserve(Cohort::Running, CONTROL_BYTES)?;
        Ok(Self {
            budget,
            _control: control,
        })
    }
    pub fn budget(&self) -> &Arc<CheckpointBudget> {
        &self.budget
    }

    pub fn load_config(&self) -> checkpoint::Result<InstalledConfig> {
        self.load_config_from_paths(
            crate::config::CONFIG_PATH,
            crate::config::DEFAULT_CONFIG_PATH,
        )
    }

    pub fn load_config_from_paths(
        &self,
        override_path: impl AsRef<Path>,
        default_path: impl AsRef<Path>,
    ) -> checkpoint::Result<InstalledConfig> {
        let lease = self.budget.reserve_running_overhead(CONFIG_BYTES)?;
        let value = crate::config::load_config_from_paths(override_path, default_path);
        Ok(InstalledConfig {
            value,
            _lease: lease,
        })
    }

    /// Copy only admitted bounded source inputs. All capacities, record
    /// headers, Arc controls and source payloads are charged BEFORE any clone.
    /// Sources are a recipe, not parsed HydrationAssets; their later parsing
    /// and mutable authority still require their own admission.
    pub fn retain_sources(&self, sources: &[&[u8]]) -> checkpoint::Result<RecipeSources> {
        let bytes = source_cost(sources)?;
        let lease = self.budget.reserve_running_overhead(bytes)?;
        let sources = sources
            .iter()
            .map(|v| v.to_vec().into_boxed_slice())
            .collect();
        Ok(RecipeSources(Arc::new(SourceOwner {
            sources,
            _lease: lease,
        })))
    }
}

/// Kept in main until App::run has returned and disposed its startup copies.
/// Changes there are scalar environment overrides; this is not a general
/// admitted API for retaining arbitrarily edited settings in future worlds.
pub(crate) struct InstalledConfig {
    value: crate::config::AppConfig,
    _lease: Reservation,
}
impl Deref for InstalledConfig {
    type Target = crate::config::AppConfig;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl DerefMut for InstalledConfig {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

pub(crate) const MAX_RECIPE_SOURCES: usize = 4096;
pub(crate) const MAX_RECIPE_SOURCE_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_RECIPE_BYTES: usize = 32 * 1024 * 1024;
fn source_cost(sources: &[&[u8]]) -> checkpoint::Result<usize> {
    if sources.len() > MAX_RECIPE_SOURCES {
        return Err(error("too many recipe sources"));
    }
    let mut bytes = 64 + size_of::<SourceOwner>() + sources.len() * (size_of::<Box<[u8]>>() + 32);
    let mut payload = 0usize;
    for source in sources {
        if source.len() > MAX_RECIPE_SOURCE_BYTES {
            return Err(error("recipe source too large"));
        }
        payload = payload
            .checked_add(source.len())
            .ok_or_else(|| error("recipe source size overflow"))?;
    }
    if payload > MAX_RECIPE_BYTES {
        return Err(error("recipe payload too large"));
    }
    bytes += payload;
    Ok(bytes)
}
struct SourceOwner {
    sources: Box<[Box<[u8]>]>,
    // Data and the entire index die before capacity can be reused.
    _lease: Reservation,
}
#[derive(Clone)]
pub(crate) struct RecipeSources(Arc<SourceOwner>);
impl RecipeSources {
    pub fn source(&self, index: usize) -> Option<&[u8]> {
        self.0.sources.get(index).map(AsRef::as_ref)
    }
    pub fn charged_bytes(&self) -> usize {
        self.0._lease.bytes()
    }
}

#[cfg(test)]
mod tests;

mod startup;
pub(crate) use startup::{CommittedStartup, StagedStartup, StartupRefusal};
