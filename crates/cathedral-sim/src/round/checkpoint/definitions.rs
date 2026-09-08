//! Pure immutable compatibility checks. Retained mutable legs and bindings are
//! not re-seeded; installed definitions only validate the static declarations.
use super::*;
fn equal(ok: bool, field: &str) -> Result<()> {
    check(ok, field)
}
pub(super) fn validate(r: &Round, c: RoundCheckpointContext<'_>) -> Result<()> {
    if !r.seeded {
        return equal(*r == Round::default(), "unseeded Round is not virgin");
    }
    let nav = c.nav.ok_or_else(|| err("Round navigation unavailable"))?;
    equal(
        nav.sites().len() <= 256
            && nav
                .sites()
                .iter()
                .try_fold(0usize, |n, s| {
                    n.checked_add(s.name.len())
                        .and_then(|n| n.checked_add(s.id.len()))
                })
                .is_some_and(|n| n <= 32 * 1024),
        "resolver site count/text exceeds v1 context policy",
    )?;
    // This fixed, installed input has a separately admitted 4 MiB scratch
    // allowance (including decoded records, resolver maps and comparisons).
    let mut scratch = 128 * 1024usize; // site resolver, comparison vectors and hash nodes
    for input in [FOOD_JSON, ROUNDS_JSON, HOMES_JSON] {
        let cost = aggregate::inspect(input.as_bytes())?;
        scratch += cost.expanded_upper_bytes * 2 + cost.encoded_bytes * 3;
    }
    equal(
        scratch <= DEFINITION_WORKING_BYTES,
        "installed definition scratch exceeds admitted policy",
    )?;
    let food: FoodDoc =
        serde_json::from_str(FOOD_JSON).map_err(|_| err("installed food definitions invalid"))?;
    let rounds: RoundsDoc = serde_json::from_str(ROUNDS_JSON)
        .map_err(|_| err("installed round definitions invalid"))?;
    let homes: HomesDoc =
        serde_json::from_str(HOMES_JSON).map_err(|_| err("installed homes definitions invalid"))?;
    let resolver = PlaceResolver::new(nav);
    let expected_sources: Vec<_> = SOURCES
        .iter()
        .filter_map(|(name, sound)| {
            nav.place(name)
                .map(|p| (*name, *sound, nav.node_point(p.node)))
        })
        .collect();
    equal(
        r.sources.len() == expected_sources.len()
            && r.sources
                .iter()
                .zip(expected_sources)
                .all(|(s, (n, sound, p))| {
                    s.name == n && s.draw_sound == sound && s.draw_point == p
                }),
        "static water source geometry mismatch",
    )?;
    let taverns: Vec<_> = TAVERNS
        .iter()
        .filter_map(|name| resolver.resolve(name))
        .collect();
    equal(r.taverns == taverns, "static tavern geometry mismatch")?;
    let expected_worksites: BTreeMap<_, _> = homes
        .homes
        .get("danqn")
        .map(|oven| {
            (
                "Ansel Quern's common oven".to_string(),
                Vec3::new(oven.point[0], WALK_Y, oven.point[1]),
            )
        })
        .into_iter()
        .collect();
    equal(
        r.worksites == expected_worksites,
        "static worksite geometry mismatch",
    )?;
    equal(
        r.food_trades.len()
            == food
                .trades
                .iter()
                .filter(|(name, spec)| valid_trade(name, spec, c.catalog))
                .count(),
        "static trade inventory count mismatch",
    )?;
    for (name, t) in &r.food_trades {
        let d = food
            .trades
            .get(name)
            .ok_or_else(|| err("trade declaration unavailable"))?;
        equal(
            t.occupations == d.occupations
                && t.listings == d.listings
                && t.restock == d.restock
                && t.per_serving == d.conjure_per_serving,
            "static trade declaration mismatch",
        )?;
    }
    let expected_stalls: Vec<_> = food
        .stalls
        .iter()
        .filter_map(|d| {
            if !r.food_trades.contains_key(&d.trade) {
                return None;
            }
            resolver
                .resolve(&d.site)
                .map(|site| (d, supply_pitch(nav, site, d.pitch_offset)))
        })
        .collect();
    equal(
        r.stalls.len() == expected_stalls.len(),
        "static stall count mismatch",
    )?;
    for (s, (d, pitch)) in r.stalls.iter().zip(expected_stalls) {
        equal(
            s.name == d.name
                && s.site == d.site
                && s.trade == d.trade
                && s.pitch == pitch
                && s.preferred.as_ref().map(ActorId::as_str) == d.preferred_vendor.as_deref()
                && s.open == d.open,
            "static stall declaration mismatch",
        )?;
    }
    for (id, s) in &r.counters {
        let d = food
            .counters
            .iter()
            .find(|d| &d.id == id)
            .ok_or_else(|| err("counter declaration unavailable"))?;
        let site = supply_site(
            &resolver,
            &r.worksites,
            c.backbone.places,
            &d.site,
            d.anchor_actor.as_ref(),
        )
        .ok_or_else(|| err("counter site declaration unavailable"))?;
        equal(
            s.trade == d.trade
                && s.site == d.site
                && s.pitch == supply_pitch(nav, site, d.pitch_offset)
                && s.seller == d.preferred_actor
                && s.offices == d.offices
                && s.required_doing == d.required_doing
                && s.road_party == d.road_party
                && s.worksite_only == d.worksite_only
                && s.radius_m.to_bits() == d.site_radius_m.to_bits(),
            "static counter declaration mismatch",
        )?;
    }
    for (id, group) in &r.counter_groups {
        equal(
            food.counter_groups
                .iter()
                .any(|d| &d.id == id && d.counters == *group),
            "static counter group mismatch",
        )?;
    }
    for p in &r.stock_plans {
        equal(
            food.stock_plans.iter().any(|d| d == p),
            "static stock plan mismatch",
        )?;
    }
    for p in &r.production_plans {
        let d = food
            .production_plans
            .iter()
            .find(|d| d.producer == p.producer)
            .ok_or_else(|| err("production declaration unavailable"))?;
        equal(
            p.max_jobs_per_day == d.max_jobs_per_day,
            "static production daily cap mismatch",
        )?;
        for t in &p.transforms {
            let spec = d
                .transforms
                .iter()
                .find(|s| s.id == t.id)
                .ok_or_else(|| err("transform declaration unavailable"))?;
            let point = supply_site(
                &resolver,
                &r.worksites,
                c.backbone.places,
                &spec.site,
                spec.anchor_actor.as_ref(),
            )
            .ok_or_else(|| err("transform site declaration unavailable"))?;
            equal(
                t == &resolved_transform(spec.clone(), point),
                "static transform recipe/work/point mismatch",
            )?;
        }
        // A previously resolved plan can disappear; paused world jobs outlive it
        // and remain watchdog obligations. Do not recreate missing declarations.
        // Current Work legs can also have changed through a committed round edit.
    }
    let expected_lamps: Vec<_> = LAMP_SQUARES
        .iter()
        .flat_map(|name| {
            resolver
                .resolve_centre(name)
                .into_iter()
                .flat_map(|centre| lamp_ring(nav, centre))
                .map(move |point| (*name, point))
        })
        .collect();
    equal(
        r.lamps.len() == expected_lamps.len(),
        "static lamp count mismatch",
    )?;
    for (l, (name, point)) in r.lamps.iter().zip(expected_lamps) {
        equal(
            l.square == name && l.position == point,
            "static lamp geometry mismatch",
        )?;
        if let Some(id) = &l.keeper {
            equal(
                rounds
                    .lamp_keepers
                    .get(id.as_str())
                    .is_some_and(|s| s.iter().any(|x| x == name)),
                "lamp keeper beat mismatch",
            )?;
        }
    }
    for (id, p) in &r.road_parties {
        let d = rounds
            .road_parties
            .iter()
            .find(|d| &d.id == id)
            .ok_or_else(|| err("road party declaration unavailable"))?;
        equal(
            p.leader == d.leader
                && p.gate == d.gate
                && Some(p.gate_point) == resolver.resolve(&d.gate)
                && p.only_on == d.only_on
                && p.stage_at == d.stage_at
                && p.enter_at == d.enter_at
                && p.return_at == d.return_at
                && p.wallet_floats == d.wallet_float_sparks
                && p.commercial_cargo == d.commercial_cargo
                && p.manifest == d.manifest,
            "static road declaration mismatch",
        )?;
        // The road permanently sheds detained members; preserve order of the
        // remaining subset, while original wallet floats deliberately remain.
        let remaining: Vec<_> = d
            .members
            .iter()
            .filter(|id| p.members.contains(id))
            .collect();
        equal(
            p.members.iter().eq(remaining),
            "road roster is not declared ordered subset",
        )?;
        equal(
            p.legs.len() == d.legs.len()
                && p.legs.iter().zip(&d.legs).all(|(a, b)| {
                    a.from == b.from
                        && Some(a.at) == resolver.resolve(&b.at)
                        && a.label == b.at
                        && a.doing == b.doing
                        && a.only_on == b.only_on
                        && !a.is_home
                }),
            "static road legs mismatch",
        )?;
    }
    for (item, shares) in c.backbone.shares {
        let item = c
            .backbone
            .items
            .get(item)
            .ok_or_else(|| err("restock item missing"))?;
        for share in shares {
            let Some(name) = share.source_id.strip_prefix("legacy_stall:") else {
                continue;
            }; // Manual provenance belongs to Inventory.
            let stall = food
                .stalls
                .iter()
                .find(|s| s.name == name)
                .ok_or_else(|| err("legacy restock source missing"))?;
            let trade = food
                .trades
                .get(&stall.trade)
                .ok_or_else(|| err("legacy restock trade missing"))?;
            equal(
                trade.restock.iter().any(|s| s.matcher().matches(item)),
                "legacy restock source/item mismatch",
            )?;
            // Original vendor is Inventory's current holder, not necessarily the
            // currently bound stall vendor. Rebinding and next restock have cadence.
        }
    }
    Ok(())
}
