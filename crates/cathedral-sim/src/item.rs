//! Items, item kinds, and the embedded item catalog (`sim.py:83-87`, now with
//! kinds/quantity/metadata — `features/food_and_items/01_items_and_stacks.md`).
//!
//! Items live only in `World.items`; possession is expressed by id in
//! `Character.holds`. An item is a **stack**: a kind, a positive quantity, and a
//! small map of catalog-declared metadata. The display name is *derived* from
//! the embedded catalog (`assets/world/items.json`) so a vendor can never hold a
//! "loaf" that secretly disagrees with its kind.

use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, LazyLock},
};

use serde::{Deserialize, Serialize};

use crate::ids::{ItemId, is_valid_id};

/// The reserved metadata key that fully *replaces* the derived display name.
/// Allowed on any kind (it is never a stack-forking adjective), it is how a
/// one-off prop — the test manifest's anvil and rope — gets a name without
/// exploding the catalog with a kind per prop.
pub const DISPLAY_METADATA_KEY: &str = "display";

/// The embedded catalog, parsed once. `include_str!` like `rounds.json`
/// (`round.rs`), so both hosts and the headless binary get it with no wiring.
static EMBEDDED_CATALOG: LazyLock<Arc<ItemCatalog>> = LazyLock::new(|| {
    Arc::new(
        ItemCatalog::from_json_str(include_str!("../../../assets/world/items.json"))
            .expect("the embedded item catalog must parse and validate"),
    )
});

// --------------------------------------------------------------------- ItemKind

/// A row in the item catalog — a newtype over `String`, id-validated exactly
/// like [`ItemId`]. Serde-transparent so a kind is a bare JSON string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemKind(String);

impl ItemKind {
    /// Validating constructor — the only way untrusted input becomes a kind.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidKind> {
        let value = value.into();
        if is_valid_id(&value) {
            Ok(Self(value))
        } else {
            Err(InvalidKind)
        }
    }

    /// Build a kind without validating. For catalog/seed data and tests.
    pub fn from_raw(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_valid(&self) -> bool {
        is_valid_id(&self.0)
    }
}

impl fmt::Display for ItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ItemKind {
    fn from(value: &str) -> Self {
        Self::from_raw(value)
    }
}

impl From<String> for ItemKind {
    fn from(value: String) -> Self {
        Self::from_raw(value)
    }
}

impl std::borrow::Borrow<str> for ItemKind {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// A kind string that is empty, too long, or carries control characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidKind;

impl fmt::Display for InvalidKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an item kind must be a non-empty id free of control characters")
    }
}

impl std::error::Error for InvalidKind {}

// ------------------------------------------------------------------------- Item

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    /// A row in the embedded item catalog.
    pub kind: ItemKind,
    /// How many. Never 0: a stack at 0 is removed from the world.
    #[serde(default = "one")]
    pub quantity: u32,
    /// Small, catalog-declared descriptors: `{"grade": "kersey"}`. Part of identity
    /// — stacks merge only when metadata is byte-equal.
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl Item {
    /// A single-unit stack of `kind`, no metadata.
    pub fn new(id: ItemId, kind: impl Into<ItemKind>) -> Self {
        Self {
            id,
            kind: kind.into(),
            quantity: 1,
            metadata: BTreeMap::new(),
        }
    }

    /// A stack of `quantity` of `kind`, no metadata.
    pub fn stack(id: ItemId, kind: impl Into<ItemKind>, quantity: u32) -> Self {
        Self {
            id,
            kind: kind.into(),
            quantity,
            metadata: BTreeMap::new(),
        }
    }

    /// Builder: attach one metadata pair (chainable, for tests and seeds).
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Whether two stacks are **the same stuff** — same kind and byte-equal
    /// metadata. The precondition for a merge (which also requires the kind be
    /// stackable, checked against the catalog by the caller).
    pub fn same_stuff_as(&self, other: &Item) -> bool {
        self.kind == other.kind && self.metadata == other.metadata
    }
}

// ---------------------------------------------------------------- the catalog

/// A kind's edibility (`eat` applies `satiety` to the hunger gauge in M2).
/// Drinks are edible kinds whose `thirst` outweighs their `satiety`: `eat`
/// applies both gauges, so a pot of ale quenches more than it feeds
/// (`features/implemented/add_items_described_in_the_lore.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edible {
    pub satiety: u32,
    /// What one unit restores on the thirst gauge. 0 (the default) for dry food.
    #[serde(default)]
    pub thirst: u32,
}

fn default_true() -> bool {
    true
}

/// One catalog row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemKindDef {
    pub kind: ItemKind,
    /// The bare display noun ("loaf", "bowl of stew").
    pub display: String,
    /// The irregular plural, when naive `display + "s"` is wrong ("loaves").
    #[serde(default)]
    pub display_plural: Option<String>,
    /// The renderer's mesh selector; the snapshot carries it so the host never
    /// needs the catalog.
    pub visual_key: String,
    /// Non-stackable kinds (a served bowl of stew) always occupy one stack of
    /// quantity 1; a second bowl is a second id.
    #[serde(default = "default_true")]
    pub stackable: bool,
    /// Present iff the kind can be `eat`en (reserved for M2's hunger gauge).
    #[serde(default)]
    pub edible: Option<Edible>,
    /// The **only** metadata keys this kind may carry, each mapped to its allowed
    /// values (an empty list means "any string"). An item with an undeclared key
    /// or value fails validation — this is what stops content from silently
    /// forking the stack space into unmergeable snowflakes. Adjectives are
    /// prefixed in this map's (sorted) key order.
    #[serde(default)]
    pub metadata: BTreeMap<String, Vec<String>>,
    /// Price by metadata selector (`""` = default, `"grade=broadcloth"` overrides).
    /// Reserved for M1's spark standard and M4's `you_sell` line.
    #[serde(default)]
    pub price_sparks: BTreeMap<String, u32>,
}

/// The catalog file's top-level shape.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemCatalogFile {
    schema_version: u32,
    kinds: Vec<ItemKindDef>,
}

/// A catalog file that does not describe a usable item catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemCatalogError {
    pub message: String,
}

impl ItemCatalogError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ItemCatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ItemCatalogError {}

/// The embedded item catalog: the single source of truth for display names,
/// visuals, stackability, edibility, metadata validity and prices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemCatalog {
    pub schema_version: u32,
    kinds: BTreeMap<ItemKind, ItemKindDef>,
}

impl ItemCatalog {
    /// Parse and validate a catalog file.
    pub fn from_json_str(source: &str) -> Result<Self, ItemCatalogError> {
        let file: ItemCatalogFile = serde_json::from_str(source)
            .map_err(|error| ItemCatalogError::new(format!("invalid item catalog: {error}")))?;
        let mut kinds: BTreeMap<ItemKind, ItemKindDef> = BTreeMap::new();
        for def in file.kinds {
            if !def.kind.is_valid() {
                return Err(ItemCatalogError::new(format!(
                    "invalid item kind '{}'",
                    def.kind
                )));
            }
            if def.display.trim().is_empty() {
                return Err(ItemCatalogError::new(format!(
                    "kind '{}' needs a non-empty display",
                    def.kind
                )));
            }
            if def.visual_key.trim().is_empty() {
                return Err(ItemCatalogError::new(format!(
                    "kind '{}' needs a non-empty visual_key",
                    def.kind
                )));
            }
            if kinds.insert(def.kind.clone(), def.clone()).is_some() {
                return Err(ItemCatalogError::new(format!(
                    "duplicate item kind '{}'",
                    def.kind
                )));
            }
        }
        if !kinds.contains_key(&ItemKind::from_raw("generic")) {
            return Err(ItemCatalogError::new(
                "the catalog must define the 'generic' escape-hatch kind",
            ));
        }
        Ok(Self {
            schema_version: file.schema_version,
            kinds,
        })
    }

    /// The embedded catalog, shared behind an `Arc`. Every `World` gets this by
    /// default, so no host wiring is needed.
    pub fn embedded() -> Arc<ItemCatalog> {
        EMBEDDED_CATALOG.clone()
    }

    pub fn get(&self, kind: &ItemKind) -> Option<&ItemKindDef> {
        self.kinds.get(kind)
    }

    /// Every declared kind, in id order.
    pub fn kinds(&self) -> impl Iterator<Item = &ItemKindDef> {
        self.kinds.values()
    }

    /// The derived, singular display name — metadata `display` override, else
    /// the declared adjectives (in key order) before the kind's noun, else the
    /// bare kind string for an ad-hoc test kind the catalog does not know.
    pub fn display_name(&self, item: &Item) -> String {
        if let Some(display) = item.metadata.get(DISPLAY_METADATA_KEY) {
            return display.clone();
        }
        match self.kinds.get(&item.kind) {
            Some(def) => with_adjectives(def, item, &def.display),
            None => item.kind.as_str().to_string(),
        }
    }

    /// The derived plural noun phrase, for counted percept lines ("3 sparks").
    pub fn display_plural(&self, item: &Item) -> String {
        if let Some(display) = item.metadata.get(DISPLAY_METADATA_KEY) {
            return format!("{display}s");
        }
        match self.kinds.get(&item.kind) {
            Some(def) => {
                let base = def
                    .display_plural
                    .clone()
                    .unwrap_or_else(|| format!("{}s", def.display));
                with_adjectives(def, item, &base)
            }
            None => format!("{}s", item.kind.as_str()),
        }
    }

    /// The renderer mesh selector, `"generic"` for an unknown kind.
    pub fn visual_key(&self, item: &Item) -> String {
        self.kinds
            .get(&item.kind)
            .map(|def| def.visual_key.clone())
            .unwrap_or_else(|| "generic".to_string())
    }

    /// Whether stacks of this kind may merge / carry a quantity above 1.
    /// Unknown kinds are treated as stackable (they are test props).
    pub fn stackable(&self, item: &Item) -> bool {
        self.kinds.get(&item.kind).is_none_or(|def| def.stackable)
    }

    /// Whether `eat` accepts this item: a **known** kind must be marked edible;
    /// an unknown ad-hoc test kind is permitted (legacy "eat allowed anything").
    pub fn is_edible(&self, item: &Item) -> bool {
        match self.kinds.get(&item.kind) {
            Some(def) => def.edible.is_some(),
            None => true,
        }
    }

    /// The `satiety` an `eat` of this item restores (M2), if it is edible.
    pub fn satiety(&self, item: &Item) -> Option<u32> {
        self.kinds
            .get(&item.kind)
            .and_then(|def| def.edible.as_ref())
            .map(|edible| edible.satiety)
    }

    /// What an `eat` of this item restores on the thirst gauge, if it is edible.
    pub fn thirst_quench(&self, item: &Item) -> Option<u32> {
        self.kinds
            .get(&item.kind)
            .and_then(|def| def.edible.as_ref())
            .map(|edible| edible.thirst)
    }

    /// Whether this kind is a drink — edible, quenching more than it feeds —
    /// which only changes the verb the world narrates (`drank`, not `ate`).
    pub fn is_drink(&self, item: &Item) -> bool {
        self.kinds
            .get(&item.kind)
            .and_then(|def| def.edible.as_ref())
            .is_some_and(|edible| edible.thirst > edible.satiety)
    }

    /// The catalog price of this exact stack in sparks (M1/M4). `None` if the
    /// kind is unpriced or unknown.
    pub fn price_sparks(&self, item: &Item) -> Option<u32> {
        let def = self.kinds.get(&item.kind)?;
        // Most specific metadata selector wins; fall back to the `""` default.
        for (key, value) in &item.metadata {
            let selector = format!("{key}={value}");
            if let Some(price) = def.price_sparks.get(&selector) {
                return Some(*price);
            }
        }
        def.price_sparks.get("").copied()
    }

    /// Validate a stack for the invariants: positive quantity, one-stack rule
    /// for non-stackable kinds, and every metadata key/value catalog-declared.
    /// Lenient about an **unknown** kind (a test prop); [`Self::validate_seed_item`]
    /// is the strict gate content must pass.
    pub fn validate_item(&self, item: &Item) -> Result<(), String> {
        if item.quantity == 0 {
            return Err(format!("item '{}' has quantity 0", item.id));
        }
        let Some(def) = self.kinds.get(&item.kind) else {
            return Ok(());
        };
        if !def.stackable && item.quantity != 1 {
            return Err(format!(
                "item '{}' of non-stackable kind '{}' has quantity {}",
                item.id, item.kind, item.quantity
            ));
        }
        for (key, value) in &item.metadata {
            if key == DISPLAY_METADATA_KEY {
                continue;
            }
            let Some(allowed) = def.metadata.get(key) else {
                return Err(format!(
                    "item '{}' carries metadata key '{key}' that kind '{}' does not declare",
                    item.id, item.kind
                ));
            };
            if !allowed.is_empty() && !allowed.iter().any(|candidate| candidate == value) {
                return Err(format!(
                    "item '{}' key '{key}' value '{value}' is not allowed by kind '{}'",
                    item.id, item.kind
                ));
            }
        }
        Ok(())
    }

    /// The strict gate for seed/content items: the kind must be catalog-known,
    /// then every rule of [`Self::validate_item`].
    pub fn validate_seed_item(&self, item: &Item) -> Result<(), String> {
        if !self.kinds.contains_key(&item.kind) {
            return Err(format!(
                "item '{}' has unknown kind '{}'",
                item.id, item.kind
            ));
        }
        self.validate_item(item)
    }
}

/// Prefix a kind's declared metadata values (in key order) before `base`.
fn with_adjectives(def: &ItemKindDef, item: &Item, base: &str) -> String {
    let mut phrase = String::new();
    for key in def.metadata.keys() {
        if let Some(value) = item.metadata.get(key) {
            phrase.push_str(value);
            phrase.push(' ');
        }
    }
    phrase.push_str(base);
    phrase
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> ItemCatalog {
        (*ItemCatalog::embedded()).clone()
    }

    #[test]
    fn the_embedded_catalog_loads_and_has_the_core_kinds() {
        let catalog = catalog();
        for kind in [
            "spark", "loaf", "herring", "smoked_eel", "stew", "generic",
            // A sample of the lore wave (`add_items_described_in_the_lore.md`):
            "ale", "water", "candle", "apple", "badge", "rope", "keepsake", "letter",
        ] {
            assert!(
                catalog.get(&ItemKind::from_raw(kind)).is_some(),
                "missing kind {kind}"
            );
        }
    }

    #[test]
    fn display_names_derive_from_kind_and_metadata() {
        let catalog = catalog();
        let spark = Item::new(ItemId::from_raw("c0prs"), "spark");
        assert_eq!(catalog.display_name(&spark), "spark");
        assert_eq!(catalog.display_plural(&spark), "sparks");

        let plain_loaf = Item::new(ItemId::from_raw("bd7k2"), "loaf");
        assert_eq!(catalog.display_name(&plain_loaf), "loaf");
        assert_eq!(catalog.display_plural(&plain_loaf), "loaves");

        let broadcloth =
            Item::new(ItemId::from_raw("cl001"), "cloth").with_metadata("grade", "broadcloth");
        assert_eq!(catalog.display_name(&broadcloth), "broadcloth bolt of cloth");

        let eel = Item::new(ItemId::from_raw("fz001"), "smoked_eel");
        assert_eq!(catalog.display_name(&eel), "smoked eel");

        // Reserved display override for a prop.
        let anvil = Item::new(ItemId::from_raw("zz001"), "generic")
            .with_metadata(DISPLAY_METADATA_KEY, "anvil");
        assert_eq!(catalog.display_name(&anvil), "anvil");

        // Unknown ad-hoc kind renders as itself.
        let sturgeon = Item::new(ItemId::from_raw("st001"), "sturgeon");
        assert_eq!(catalog.display_name(&sturgeon), "sturgeon");

        // Declared adjectives prefix in key order: the lore wave's keepsake.
        let button =
            Item::new(ItemId::from_raw("kp001"), "keepsake").with_metadata("kind", "wooden button");
        assert_eq!(catalog.display_name(&button), "wooden button keepsake");
    }

    #[test]
    fn drinks_declare_a_thirst_refill() {
        let catalog = catalog();
        let ale = Item::new(ItemId::from_raw("a"), "ale");
        assert!(catalog.is_edible(&ale));
        assert!(catalog.is_drink(&ale));
        assert!(catalog.thirst_quench(&ale).unwrap_or(0) > 0);
        // Dry food quenches nothing and is not a drink.
        let loaf = Item::new(ItemId::from_raw("l"), "loaf");
        assert!(!catalog.is_drink(&loaf));
        assert_eq!(catalog.thirst_quench(&loaf), Some(0));
        // Water is pure drink: no satiety at all.
        let water = Item::new(ItemId::from_raw("w"), "water");
        assert_eq!(catalog.satiety(&water), Some(0));
        assert!(catalog.is_drink(&water));
    }

    #[test]
    fn prices_and_edibility_come_from_the_catalog() {
        let catalog = catalog();
        assert_eq!(
            catalog.price_sparks(&Item::new(ItemId::from_raw("h"), "herring")),
            Some(1)
        );
        let broadcloth =
            Item::new(ItemId::from_raw("w"), "cloth").with_metadata("grade", "broadcloth");
        assert_eq!(catalog.price_sparks(&broadcloth), Some(40));
        assert_eq!(
            catalog.price_sparks(&Item::new(ItemId::from_raw("l"), "loaf")),
            Some(2)
        );
        assert!(catalog.is_edible(&Item::new(ItemId::from_raw("h"), "herring")));
        assert!(!catalog.is_edible(&Item::new(ItemId::from_raw("s"), "spark")));
        assert_eq!(
            catalog.satiety(&Item::new(ItemId::from_raw("l"), "loaf")),
            Some(150)
        );
    }

    #[test]
    fn metadata_validation_rejects_undeclared_keys_and_values() {
        let catalog = catalog();
        let ok = Item::new(ItemId::from_raw("l"), "cloth").with_metadata("grade", "kersey");
        assert!(catalog.validate_seed_item(&ok).is_ok());

        let bad_value = Item::new(ItemId::from_raw("l"), "cloth").with_metadata("grade", "linen");
        assert!(catalog.validate_seed_item(&bad_value).is_err());

        let bad_key = Item::new(ItemId::from_raw("l"), "loaf").with_metadata("colour", "brown");
        assert!(catalog.validate_seed_item(&bad_key).is_err());

        let unknown_kind = Item::new(ItemId::from_raw("x"), "sturgeon");
        assert!(catalog.validate_seed_item(&unknown_kind).is_err());
        // But the lenient path tolerates the ad-hoc kind.
        assert!(catalog.validate_item(&unknown_kind).is_ok());

        let non_stackable = Item::stack(ItemId::from_raw("st"), "stew", 3);
        assert!(catalog.validate_item(&non_stackable).is_err());
    }
}
