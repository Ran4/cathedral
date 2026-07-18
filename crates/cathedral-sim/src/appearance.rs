//! The structured body-appearance seam (`features/npc_bodies.md` §2).
//!
//! [`AppearanceSnapshot`] is composed **once**, at character creation, from
//! sheet facts — never per frame and never host-side — and then crosses the
//! boundary verbatim on [`crate::ActorSnapshot`]. The host maps it to
//! materials/meshes; the sim never reads it back, and nothing about it ever
//! enters a prompt.
//!
//! Determinism: everything here is a pure function of the character's authored
//! facts and id (FNV-1a, not `DefaultHasher`, so the same cast dresses the same
//! across runs *and* toolchains). Same world seed → same appearances.

use serde::{Deserialize, Serialize};

use crate::ids::ActorId;

/// Body silhouette class (shoulder/hip/height scaling host-side).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Build {
    Female,
    Male,
}

/// The seven readable-at-30-m dress classes. The host maps each to a band of
/// outfit materials; `palette_seed` tints within the band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutfitClass {
    Cleric,
    Merchant,
    Craftsman,
    Laborer,
    Watch,
    Notable,
    Poor,
}

/// Occupation-readable headgear, one optional mesh part on the puppet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Headgear {
    None,
    Hood,
    Coif,
    Brim,
    KettleHelm,
}

/// Everything the renderer needs to dress a body, composed once in the sim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppearanceSnapshot {
    pub build: Build,
    pub outfit: OutfitClass,
    pub headgear: Headgear,
    /// Deterministic per-id tint seed (FNV-1a of the actor id). The host picks
    /// a tint within the outfit's palette band (and, later, a face texture)
    /// from it.
    pub palette_seed: u32,
    /// The bespoke-look override for the named majors (`"sven"`, `"conny"`,
    /// `"ilse"`), carried explicitly so their established colors survive the
    /// seam. Deliberately *not* derived from `name_for_player`, which is
    /// subject to the unknown-people naming rule — an unrecognized Ilse must
    /// still look like Ilse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bespoke: Option<String>,
}

impl Default for AppearanceSnapshot {
    /// The pre-seam fallback look: the host maps this to the lilac placeholder.
    /// Used by the player record and by compact test fixtures, which carry no
    /// lore facts to compose from.
    fn default() -> Self {
        Self {
            build: Build::Male,
            outfit: OutfitClass::Laborer,
            headgear: Headgear::None,
            palette_seed: 0,
            bespoke: None,
        }
    }
}

impl AppearanceSnapshot {
    /// Compose a body from the facts a lore sheet actually carries.
    ///
    /// What is used and what is deliberately not:
    /// - `gender` → [`Build`] (the authored values are `"f"`/`"m"`).
    /// - `occupation_id` → [`OutfitClass`] via [`outfit_class_of`]; no
    ///   occupation (the `no_fixed_trade` folder) reads as [`OutfitClass::Poor`].
    /// - `rank == "warden"` promotes a trade to [`OutfitClass::Notable`] —
    ///   guild officers dress above their bench; masters keep their trade so a
    ///   master smith still reads as a smith.
    /// - poverty circumstances (`pauper`, `begs_regularly`, `alms_dependent`,
    ///   `unhoused`) pull a trade down to [`OutfitClass::Poor`]; uniforms,
    ///   vows and office (Watch/Cleric/Notable) trump rags.
    /// - `district`/ward is **not** used: geography does not change dress
    ///   class, and the per-id `palette_seed` already varies the street.
    /// - `faction_role` is **not** used: every authored faction role is a
    ///   secret-society role (the Custody's moths, the Unwalled), and a
    ///   faction outfit would leak a secret at thirty metres.
    pub fn compose(
        id: &ActorId,
        gender: &str,
        occupation_id: Option<&str>,
        rank: Option<&str>,
        circumstances: &[String],
    ) -> Self {
        let build = if gender.eq_ignore_ascii_case("f") {
            Build::Female
        } else {
            Build::Male
        };
        let mut outfit = match occupation_id {
            Some(occupation) => outfit_class_of(occupation),
            None => OutfitClass::Poor,
        };
        if rank == Some("warden")
            && matches!(
                outfit,
                OutfitClass::Craftsman | OutfitClass::Merchant | OutfitClass::Laborer
            )
        {
            outfit = OutfitClass::Notable;
        }
        let destitute = circumstances.iter().any(|circumstance| {
            matches!(
                circumstance.as_str(),
                "pauper" | "begs_regularly" | "alms_dependent" | "unhoused"
            )
        });
        if destitute
            && matches!(
                outfit,
                OutfitClass::Craftsman | OutfitClass::Merchant | OutfitClass::Laborer
            )
        {
            outfit = OutfitClass::Poor;
        }
        let palette_seed = palette_seed_of(id);
        Self {
            build,
            outfit,
            headgear: headgear_of(outfit, palette_seed),
            palette_seed,
            bespoke: None,
        }
    }

    /// Attach (or clear) the bespoke-look override of a named major.
    pub fn with_bespoke(mut self, bespoke: Option<String>) -> Self {
        self.bespoke = bespoke;
        self
    }
}

/// FNV-1a over the actor id — the deterministic per-id tint seed. Not
/// [`std::hash::DefaultHasher`] (its output may change between Rust releases):
/// a screenshot taken next year should show the same crowd in the same coats.
pub fn palette_seed_of(id: &ActorId) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in id.as_str().bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

/// The occupation-id → dress-class table, over the ids in
/// `lore/core_lore/occupations.json`. An id this table has never heard of
/// dresses as a laborer — the neutral read for "someone who works".
fn outfit_class_of(occupation_id: &str) -> OutfitClass {
    match occupation_id {
        // Robes, coifs and clerkly dress: the church, its pen-holders and the
        // pilgrims walking to it.
        "anchoress" | "bell_ringer" | "candor_cleric" | "church_attendant" | "custody_clerk"
        | "pilgrim" | "scholar" | "scribe_and_clerk" => OutfitClass::Cleric,
        // The armed and the warranted.
        "bailiff_and_gaoler" | "executioner" | "militia_and_soldier" | "watchman_and_keeper" => {
            OutfitClass::Watch
        }
        // Civic office — visibly *somebody* rather than a trade.
        "civic_officer" | "court_officer" | "revenue_worker" => OutfitClass::Notable,
        // People whose work is selling.
        "draper" | "fish_trader" | "food_provisioner" | "freight_broker" | "grocer_and_spicer"
        | "market_seller" | "merchant" | "money_dealer" | "salt_trader" => OutfitClass::Merchant,
        // People whose work is making.
        "baker" | "bellfounder" | "brewer" | "butcher" | "carpenter_and_builder"
        | "cartwright_and_wheelwright" | "chandler" | "cloth_worker" | "cook" | "cooper"
        | "fine_metalworker" | "garment_worker" | "glazier" | "healer" | "instrument_maker"
        | "leather_worker" | "mason" | "miller" | "painter" | "potter" | "roper" | "shoemaker"
        | "smith" => OutfitClass::Craftsman,
        // Everyone else who carries, hauls, serves, guides or scrubs — and the
        // fallback for an occupation coined after this table.
        _ => OutfitClass::Laborer,
    }
}

/// Headgear per class. Where the class is not uniform the choice rides a high
/// bit of the palette seed — away from the low bits a tint ramp would read —
/// so it stays deterministic per id without correlating with the coat color.
fn headgear_of(outfit: OutfitClass, palette_seed: u32) -> Headgear {
    let variant = (palette_seed >> 7) & 1 == 0;
    match outfit {
        OutfitClass::Cleric => {
            if variant {
                Headgear::Coif
            } else {
                Headgear::Hood
            }
        }
        OutfitClass::Watch => Headgear::KettleHelm,
        OutfitClass::Notable => Headgear::Brim,
        OutfitClass::Poor => Headgear::Hood,
        OutfitClass::Craftsman => {
            if variant {
                Headgear::Brim
            } else {
                Headgear::None
            }
        }
        OutfitClass::Merchant => {
            if variant {
                Headgear::Brim
            } else {
                Headgear::Coif
            }
        }
        OutfitClass::Laborer => {
            if variant {
                Headgear::Coif
            } else {
                Headgear::None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: &str) -> ActorId {
        ActorId::from_raw(raw)
    }

    /// The determinism contract: identical facts compose identical bodies,
    /// and the palette seed is a pure function of the id.
    #[test]
    fn composition_is_deterministic_per_id_and_facts() {
        let circumstances = vec!["recent_migrant".to_string()];
        let first =
            AppearanceSnapshot::compose(&id("sv3n1"), "m", Some("smith"), None, &circumstances);
        let second =
            AppearanceSnapshot::compose(&id("sv3n1"), "m", Some("smith"), None, &circumstances);
        assert_eq!(first, second);
        assert_eq!(first.palette_seed, palette_seed_of(&id("sv3n1")));
        // Different ids draw different tints (spot-checked, not universal —
        // a 32-bit hash may collide, just not on the shipped trio).
        assert_ne!(
            palette_seed_of(&id("sv3n1")),
            palette_seed_of(&id("cb947"))
        );
    }

    #[test]
    fn sheet_facts_map_to_the_expected_classes() {
        let none: &[String] = &[];
        let compose = |gender: &str, occupation: Option<&str>, rank: Option<&str>| {
            AppearanceSnapshot::compose(&id("aaaaa"), gender, occupation, rank, none)
        };

        let cleric = compose("f", Some("candor_cleric"), None);
        assert_eq!(cleric.build, Build::Female);
        assert_eq!(cleric.outfit, OutfitClass::Cleric);
        assert!(matches!(cleric.headgear, Headgear::Coif | Headgear::Hood));

        let watch = compose("m", Some("watchman_and_keeper"), None);
        assert_eq!(watch.outfit, OutfitClass::Watch);
        assert_eq!(watch.headgear, Headgear::KettleHelm);

        assert_eq!(compose("m", Some("smith"), None).outfit, OutfitClass::Craftsman);
        assert_eq!(
            compose("m", Some("fish_trader"), None).outfit,
            OutfitClass::Merchant
        );
        assert_eq!(
            compose("m", Some("civic_officer"), None).outfit,
            OutfitClass::Notable
        );
        assert_eq!(
            compose("m", Some("general_labourer"), None).outfit,
            OutfitClass::Laborer
        );
        // An occupation the table has never heard of dresses as a laborer.
        assert_eq!(
            compose("m", Some("astronaut"), None).outfit,
            OutfitClass::Laborer
        );

        // A guild warden dresses above the bench; a master keeps the trade.
        assert_eq!(
            compose("m", Some("smith"), Some("warden")).outfit,
            OutfitClass::Notable
        );
        assert_eq!(
            compose("m", Some("smith"), Some("master")).outfit,
            OutfitClass::Craftsman
        );
    }

    #[test]
    fn poverty_pulls_trades_down_but_never_strips_a_uniform() {
        let poor = vec!["pauper".to_string()];
        let beggar =
            AppearanceSnapshot::compose(&id("aaaaa"), "f", Some("entertainer"), None, &poor);
        assert_eq!(beggar.outfit, OutfitClass::Poor);
        assert_eq!(beggar.headgear, Headgear::Hood);

        // No fixed trade reads poor even before circumstances.
        let no_trade = AppearanceSnapshot::compose(&id("aaaaa"), "m", None, None, &[]);
        assert_eq!(no_trade.outfit, OutfitClass::Poor);

        // Uniforms, vows and office trump rags.
        let poor_watchman = AppearanceSnapshot::compose(
            &id("aaaaa"),
            "m",
            Some("watchman_and_keeper"),
            None,
            &poor,
        );
        assert_eq!(poor_watchman.outfit, OutfitClass::Watch);
        assert_eq!(poor_watchman.headgear, Headgear::KettleHelm);
    }

    /// The named majors carry their identity explicitly — never via
    /// `name_for_player`, which the unknown-people rule rewrites.
    #[test]
    fn bespoke_rides_on_top_of_a_composed_body() {
        let sven = AppearanceSnapshot::compose(&id("sv3n1"), "m", Some("smith"), None, &[])
            .with_bespoke(Some("sven".to_string()));
        assert_eq!(sven.bespoke.as_deref(), Some("sven"));
        // The composed facts survive underneath the override.
        assert_eq!(sven.outfit, OutfitClass::Craftsman);

        let ambient = AppearanceSnapshot::compose(&id("aaaaa"), "m", Some("smith"), None, &[]);
        assert_eq!(ambient.bespoke, None);
    }
}
