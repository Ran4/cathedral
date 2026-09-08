//! Exact immutable navigation binding and bounded borrowed actor references.
use super::*;
use crate::world::checkpoint::{BackboneCandidate, BackboneRefs};
#[derive(Clone, Copy)]
pub struct AnimalsCheckpointContext<'a> {
    pub(crate) now: LogicalTime,
    pub(crate) backbone: BackboneRefs<'a>,
    nav: Option<&'a NavData>,
    pub(crate) engine_nav: Option<&'a NavData>,
}
impl<'a> AnimalsCheckpointContext<'a> {
    pub fn from_world(w: &'a World, now: LogicalTime) -> Self {
        Self {
            now,
            backbone: BackboneRefs::from_world(w),
            nav: w.nav.as_deref(),
            engine_nav: w.nav.as_deref(),
        }
    }
    pub fn from_backbone(
        candidate: &'a BackboneCandidate,
        now: LogicalTime,
        nav: Option<&'a NavData>,
    ) -> Self {
        Self {
            now,
            backbone: candidate.references(),
            nav,
            engine_nav: nav,
        }
    }
    /// The Engine's movement consumer retains its own immutable nav copy. After
    /// a public World.nav edit this binding can differ from World's binding.
    pub fn with_engine_nav(mut self, nav: Option<&'a NavData>) -> Self {
        self.engine_nav = nav;
        self
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BindingV1 {
    #[serde(deserialize_with = "crate::checkpoint::serde_support::required_option")]
    nav: Option<[u8; 32]>,
}
impl BindingV1 {
    pub(crate) fn new(c: AnimalsCheckpointContext<'_>) -> Result<Self> {
        checkpoint::logical(OWNER, c.now.seconds())?;
        check(
            c.backbone.characters.len() <= 25_000,
            "animals actor context count",
        )?;
        // NavData's fields are immutable and its only constructor validates the
        // grid/graph. No parser, cache warming, index or copied geometry here.
        Ok(Self {
            nav: c.nav.map(NavData::checkpoint_fingerprint),
        })
    }
}
