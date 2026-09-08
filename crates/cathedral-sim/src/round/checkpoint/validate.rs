use super::*;
use crate::character::checkpoint::{metadata, point};
use crate::checkpoint::{calendar, logical};
fn text(v: &str) -> Result<()> {
    check(
        v.len() <= crate::checkpoint::records::MAX_TEXT_BYTES,
        "text exceeds v1 limit",
    )
}
fn name(v: &str) -> Result<()> {
    text(v)?;
    check(!v.trim().is_empty(), "empty identity")
}
fn day(v: i64) -> Result<()> {
    calendar(OWNER, v as f64)
}
fn nonnegative(v: f64, max: f64, reason: &str) -> Result<()> {
    check(v.is_finite() && (0.0..=max).contains(&v), reason)
}
fn unique<T: Ord>(v: &[T], reason: &str) -> Result<()> {
    check(v.iter().collect::<BTreeSet<_>>().len() == v.len(), reason)
}
fn actor<'a>(c: RoundCheckpointContext<'a>, id: &ActorId) -> Result<&'a Character> {
    check(id.is_valid(), "invalid actor id")?;
    c.backbone
        .characters
        .get(id)
        .ok_or_else(|| err("live Round binding actor missing"))
}
fn offices(v: &[Office]) -> Result<()> {
    check(v.len() <= Office::ALL.len(), "office list limit")?;
    unique(v, "duplicate office")
}
fn weekdays(v: &[Weekday]) -> Result<()> {
    check(v.len() <= 7, "weekday list limit")?;
    unique(v, "duplicate weekday")
}
fn leg(l: &RoundLeg) -> Result<()> {
    point(l.at)?;
    name(&l.label)?;
    if let Some(days) = &l.only_on {
        weekdays(days)?;
    }
    Ok(())
}
fn legs(v: &[RoundLeg]) -> Result<()> {
    check(v.len() <= MAX_LEGS, "leg count limit")?;
    for l in v {
        leg(l)?;
    }
    Ok(())
}
fn matcher(
    kind: &crate::item::ItemKind,
    values: &BTreeMap<String, String>,
    c: RoundCheckpointContext<'_>,
) -> Result<()> {
    metadata(values)?;
    let item = Item {
        id: ItemId::from_raw("checkpoint_recipe"),
        kind: kind.clone(),
        quantity: 1,
        metadata: values.clone(),
    };
    c.catalog
        .validate_seed_item(&item)
        .map_err(|_| err("recipe catalog binding invalid"))
}
fn stocks(v: &[StockSpec], c: RoundCheckpointContext<'_>) -> Result<()> {
    check(v.len() <= MAX_LEGS, "stock line count limit")?;
    let mut totals = BTreeMap::<(&crate::item::ItemKind, &BTreeMap<String, String>), u32>::new();
    for s in v {
        check(s.quantity > 0, "zero stock quantity")?;
        matcher(&s.kind, &s.metadata, c)?;
        c.catalog
            .validate_seed_item(
                &s.matcher()
                    .to_item(ItemId::from_raw("checkpoint_stock"), s.quantity),
            )
            .map_err(|_| err("stock quantity/catalog binding invalid"))?;
        let n = totals.entry((&s.kind, &s.metadata)).or_default();
        *n = n
            .checked_add(s.quantity)
            .ok_or_else(|| err("aggregate stock quantity overflow"))?;
    }
    Ok(())
}
pub(crate) fn weather(w: &WeatherShelterIntent, c: RoundCheckpointContext<'_>) -> Result<()> {
    let shelter = c
        .shelters
        .shelters()
        .get(w.shelter)
        .ok_or_else(|| err("shelter index missing"))?;
    point(w.target)?;
    check(
        shelter.access == ShelterAccess::Public,
        "weather reservation is private shelter",
    )?;
    nonnegative(w.release_threshold, 1.0, "invalid shelter threshold")?;
    nonnegative(w.release_after_days, 1.0, "invalid shelter hysteresis")?;
    if let Some(t) = w.below_since_days {
        calendar(OWNER, t)?;
    }
    // Presence, position, office and release deadline can have changed since the
    // last owner pass. They are evaluated at the next ordinary weather tick.
    Ok(())
}
fn queue(
    q: &[ActorId],
    serving: &Option<(ActorId, f64)>,
    c: RoundCheckpointContext<'_>,
) -> Result<()> {
    check(q.len() <= MAX_PEOPLE, "queue count limit")?;
    unique(q, "duplicate queue actor")?;
    for id in q {
        actor(c, id)?;
    }
    if let Some((id, t)) = serving {
        actor(c, id)?;
        logical(OWNER, *t)?;
        // Household priority insertion can put a new drawer ahead of the actor
        // whose atomic service already began; require membership, not q[0].
        check(q.contains(id), "serving actor absent from queue")?;
    }
    Ok(())
}
fn binding(b: &CounterBindingKey, r: &Round, c: RoundCheckpointContext<'_>) -> Result<()> {
    name(&b.counter_id)?;
    check(b.seller.is_valid(), "invalid historical counter seller")?;
    // Historical sessions remain after a seller becomes absent or a party starts
    // another trip. Identity is exact; current eligibility is not a save gate.
    let counter = r
        .counters
        .get(&b.counter_id)
        .ok_or_else(|| err("counter binding missing"))?;
    check(
        counter.seller == b.seller,
        "counter seller binding mismatch",
    )?;
    actor(c, &b.seller)?;
    match &b.session {
        CounterSession::Daily { absolute_day } => {
            day(*absolute_day)?;
            check(
                counter.road_party.is_none(),
                "daily session on road counter",
            )?;
        }
        CounterSession::RoadTrip {
            party_id,
            trip_number,
        } => {
            let party = r
                .road_parties
                .get(party_id)
                .ok_or_else(|| err("counter session party missing"))?;
            check(
                counter.road_party.as_ref() == Some(party_id)
                    && *trip_number <= party.state.trip_number,
                "road counter session mismatch",
            )?;
        }
    }
    Ok(())
}
pub(super) fn validate(r: &Round, c: RoundCheckpointContext<'_>) -> Result<()> {
    check(r.ladder_scratch.is_empty(), "incomplete ladder pass")?;
    // Leave representable room for a complete maximum-population accounting pass.
    // Whole-envelope time/rate horizon validation remains a separate gate.
    let accounting_headroom = MAX_PEOPLE as u64 * u32::MAX as u64;
    for total in [
        r.institutional_payroll_sparks,
        r.household_redistributed_sparks,
        r.road_cash_in_sparks,
        r.road_cash_out_sparks,
    ] {
        check(
            total <= u64::MAX - accounting_headroom,
            "accounting total lacks next-pass headroom",
        )?;
    }

    check(
        r.people.len() <= MAX_PEOPLE
            && r.sources.len() <= MAX_CATALOG_ROWS
            && r.stalls.len() <= MAX_CATALOG_ROWS
            && r.lamps.len() <= MAX_CATALOG_ROWS,
        "Round owner count limit",
    )?;
    check(
        !r.seeded || c.nav.is_some(),
        "seeded Round needs exact navigation",
    )?;
    calendar(OWNER, r.last_game_days)?;
    calendar(OWNER, r.production_last_game_days)?;
    if let Some(t) = r.last_office_days {
        calendar(OWNER, t)?;
    }
    for d in [
        r.lamp_night_day,
        r.last_household_watch_day,
        r.last_household_settlement_day,
    ]
    .into_iter()
    .flatten()
    {
        day(d)?;
    }
    check(r.food_log.len() <= FOOD_LOG_CAP, "food log count limit")?;
    for line in &r.food_log {
        text(line)?;
    }
    let mut water_queued = BTreeSet::new();
    let mut names = BTreeSet::new();
    for (s, source) in r.sources.iter().enumerate() {
        name(&source.name)?;
        point(source.draw_point)?;
        check(names.insert(&source.name), "duplicate water source name")?;
        check(
            SOURCES.contains(&(source.name.as_str(), source.draw_sound)),
            "water source sound/name mismatch",
        )?;
        if let Some(id) = &source.keeper {
            actor(c, id)?;
        }
        logical(OWNER, source.keeper_next_sound)?;
        queue(&source.queue, &source.serving, c)?;
        for id in &source.queue {
            check(water_queued.insert(id), "actor in multiple water queues")?;
            check(
                r.people.get(id).is_some_and(|p| {
                    p.source == Some(s) && matches!(p.phase, Phase::Queued | Phase::Drawing)
                }),
                "water queue/controller binding mismatch",
            )?;
        }
    }
    for p in &r.taverns {
        point(*p)?;
    }
    let mut food_queued = BTreeSet::new();
    let mut vendors = BTreeSet::new();
    names.clear();
    for (s, stall) in r.stalls.iter().enumerate() {
        name(&stall.name)?;
        name(&stall.site)?;
        name(&stall.trade)?;
        point(stall.pitch)?;
        check(names.insert(&stall.name), "duplicate food stall name")?;
        check(
            r.food_trades.contains_key(&stall.trade),
            "stall trade missing",
        )?;
        offices(&stall.open.offices)?;
        if let Some(d) = &stall.open.weekdays {
            weekdays(d)?;
        }
        if let Some(id) = &stall.vendor {
            actor(c, id)?;
        }
        if let Some(id) = &stall.preferred {
            check(id.is_valid(), "invalid authored preferred vendor")?;
        }
        if let Some(id) = &stall.vendor {
            check(vendors.insert(id), "vendor bound to multiple stalls")?;
            check(
                c.backbone.characters[id].state.you_sell
                    == r.sell_listings_from_catalog(c.catalog, s),
                "vendor price projection mismatch",
            )?;
        }
        logical(OWNER, stall.cry_next)?;
        queue(&stall.queue, &stall.serving, c)?;
        for id in &stall.queue {
            check(food_queued.insert(id), "actor in multiple food queues")?;
            check(
                r.people
                    .get(id)
                    .and_then(|p| p.food.as_ref())
                    .is_some_and(|f| f.stall == s && f.phase == FoodPhase::Queued),
                "food queue/controller binding mismatch",
            )?;
        }
    }
    if r.seeded {
        for (id, ch) in c.backbone.characters {
            check(
                ch.state.you_sell.is_empty() || vendors.contains(id),
                "orphan vendor price projection",
            )?;
            check(
                ch.state.resident.is_none() || r.residents.people.contains_key(id),
                "orphan resident projection",
            )?;
        }
    }
    for (key, t) in &r.food_trades {
        name(key)?;
        check(
            t.occupations.len() <= MAX_LEGS && t.listings.len() <= MAX_LEGS,
            "trade list limit",
        )?;
        for o in &t.occupations {
            name(o)?;
        }
        for m in &t.listings {
            matcher(&m.kind, &m.metadata, c)?;
        }
        stocks(&t.restock, c)?;
        if let Some(k) = &t.per_serving {
            matcher(k, &BTreeMap::new(), c)?;
        }
    }
    for (id, p) in &r.people {
        let ch = actor(c, id)?;
        if let Some(home) = p.home {
            point(home)?;
            check(
                c.backbone.household_doors.get(id) == Some(&home),
                "Round household door mismatch",
            )?;
        }
        point(p.base)?;
        legs(&p.legs)?;
        nonnegative(p.leash_m, 1.0e6, "invalid leash")?;
        nonnegative(
            p.leg_lag_share,
            CROWD_LEG_LAG_MAX_SHARE,
            "invalid office lag",
        )?;
        if let Some(s) = p.source {
            check(s < r.sources.len(), "source index missing")?;
        }
        if let Some(v) = p.travel_target {
            point(v)?;
        }
        logical(OWNER, p.next_decision)?;
        if let Some((i, l)) = &p.evening_seed {
            check(*i < p.legs.len(), "evening seed leg index missing")?;
            leg(l)?;
        }
        if let Some(f) = &p.food {
            check(f.stall < r.stalls.len(), "food errand stall missing")?;
            match &f.phase {
                FoodPhase::Queued => {
                    check(food_queued.contains(id), "queued food errand has no queue")?
                }
                FoodPhase::Eating { item, until } => {
                    check(item.is_valid(), "invalid eating item")?;
                    logical(OWNER, *until)?;
                }
                FoodPhase::Approaching => {}
            }
            // Eating retains the attempted item until the next food pass; an
            // intervening transfer/consumption may legitimately remove it.
        }
        if matches!(p.phase, Phase::Queued | Phase::Drawing) {
            check(water_queued.contains(id), "queued water phase has no queue")?;
        }
        if !r.residents.people.contains_key(id) {
            check(
                ch.state.daily_round == p.legs.iter().map(leg_line).collect::<Vec<_>>(),
                "daily round leg projection mismatch",
            )?;
        }
    }
    for map in [&r.lightning_reflex_until, &r.chalk_refused_until] {
        for (id, t) in map {
            actor(c, id)?;
            logical(OWNER, *t)?;
        }
    }
    for (id, t) in &r.knowledge_refused_until {
        actor(c, id)?;
        calendar(OWNER, *t)?;
    }
    for id in &r.knowledge_refused_buyers {
        actor(c, id)?;
    }
    for (id, t) in &r.next_pollen {
        check(id.is_valid(), "invalid historical pollen actor")?;
        CalendarAnchorV1::from_legacy(*t)?;
        if r.people.contains_key(id) {
            check(
                r.pollen_due.contains(&(pollen_due_key(*t), id.clone())),
                "live pollen deadline absent from index",
            )?;
        }
    }
    for (key, id) in &r.pollen_due {
        check(id.is_valid(), "invalid pollen index actor")?;
        let bits = if key >> 63 == 0 {
            !key
        } else {
            key & !(1 << 63)
        };
        CalendarAnchorV1::from_legacy(f64::from_bits(bits))?;
        check(
            pollen_due_key(f64::from_bits(bits)) == *key,
            "noncanonical pollen deadline key",
        )?;
        // Stale re-enrollment entries are intentionally retained until pop.
    }
    for (id, w) in &r.weather_shelter_intents {
        actor(c, id)?;
        weather(w, c)?;
    }
    for l in &r.lamps {
        name(&l.square)?;
        point(l.position)?;
        if let Some(id) = &l.keeper {
            actor(c, id)?;
        }
    }
    for (id, i) in &r.lamp_targets {
        actor(c, id)?;
        check(
            r.lamps
                .get(*i)
                .is_some_and(|l| l.keeper.as_ref() == Some(id)),
            "lamp keeper target mismatch",
        )?;
    }
    for (square, i) in &r.belwyn {
        check(
            r.lamps.get(*i).is_some_and(|l| l.square == *square),
            "Belwyn lamp square mismatch",
        )?;
    }
    for map in [&r.household_reserves, &r.unrelieved_zero_streak] {
        for id in map.keys() {
            actor(c, id)?;
        }
    }
    // Watch sampling and successful settlement markers deliberately differ on
    // failure. Neither is reconstructed from the other or forced to today.
    parties(r, c)?;
    market(r, c)?;
    production(r, c)?;
    crate::round::residents::checkpoint::validate(r, c)?;
    Ok(())
}
fn parties(r: &Round, c: RoundCheckpointContext<'_>) -> Result<()> {
    check(r.road_parties.len() <= MAX_PLANS, "party count limit")?;
    let mut members = BTreeSet::new();
    for (id, p) in &r.road_parties {
        check(id == &p.id && id.is_valid(), "party identity mismatch")?;
        check(
            !p.members.is_empty() && p.members.len() <= MAX_PEOPLE && p.members.contains(&p.leader),
            "party leader/roster mismatch",
        )?;
        unique(&p.members, "duplicate road member")?;
        for id in &p.members {
            let ch = actor(c, id)?;
            check(
                members.insert(id) && !r.people.contains_key(id),
                "road member has duplicate controller",
            )?;
            check(
                ch.state.daily_round == p.legs.iter().map(leg_line).collect::<Vec<_>>(),
                "road daily projection mismatch",
            )?;
        }
        name(&p.gate)?;
        point(p.gate_point)?;
        weekdays(&p.only_on)?;
        legs(&p.legs)?;
        for id in p.wallet_floats.keys() {
            actor(c, id)?;
        }
        for (id, t) in &p.departure_excuses {
            actor(c, id)?;
            logical(OWNER, *t)?;
        }
        if let Some(d) = p.last_trigger_day {
            day(d)?;
        }
        check(
            p.commercial_cargo.len() <= MAX_LEGS,
            "cargo matcher count limit",
        )?;
        for m in &p.commercial_cargo {
            matcher(&m.kind, &m.metadata, c)?;
        }
        stocks(&p.manifest, c)?;
        // Law/presence can change before this owner's next road pass. Former
        // members also remain in historical wallet/excuse maps until reset.
    }
    for (id, loads) in &r.observed_cart_loads {
        check(id.is_valid(), "invalid historical cart id")?;
        unique(loads, "duplicate observed cart load")?;
        check(loads.len() <= 3, "cart load count")?;
    }
    for id in &r.departed_this_tick {
        check(id.is_valid(), "invalid departure notification id")?;
    }
    check(
        r.departed_this_tick.len() <= MAX_PEOPLE,
        "departure notification count",
    )?;
    Ok(())
}
fn market(r: &Round, c: RoundCheckpointContext<'_>) -> Result<()> {
    check(
        r.stock_plans.len() <= MAX_PLANS
            && r.counters.len() <= MAX_PLANS
            && r.counter_groups.len() <= MAX_PLANS,
        "market planner count limit",
    )?;
    for (id, p) in &r.worksites {
        name(id)?;
        point(*p)?;
    }
    for (id, t) in &r.counters {
        name(id)?;
        check(id == &t.id, "counter identity mismatch")?;
        actor(c, &t.seller)?;
        name(&t.trade)?;
        name(&t.site)?;
        point(t.pitch)?;
        offices(&t.offices)?;
        check(
            r.food_trades.contains_key(&t.trade),
            "counter trade missing",
        )?;
        nonnegative(t.radius_m, 1.0e6, "invalid counter radius")?;
        if let Some(p) = &t.road_party {
            check(r.road_parties.contains_key(p), "counter party missing")?;
        }
    }
    for (id, group) in &r.counter_groups {
        name(id)?;
        check(group.len() <= MAX_PLANS, "counter group count")?;
        unique(group, "duplicate counter group member")?;
        for key in group {
            check(
                r.counters.contains_key(key),
                "counter group missing counter",
            )?;
        }
    }
    let mut ids = BTreeSet::new();
    for p in &r.stock_plans {
        name(&p.id)?;
        actor(c, &p.buyer)?;
        check(ids.insert(&p.id), "duplicate stock plan identity")?;
        match &p.source {
            StockSource::Counter(id) => {
                check(r.counters.contains_key(id), "stock source counter missing")?
            }
            StockSource::CounterGroup(id) => check(
                r.counter_groups.contains_key(id),
                "stock source group missing",
            )?,
        }
        check(p.targets.len() <= MAX_LEGS, "stock target count")?;
        for t in &p.targets {
            matcher(&t.kind, &t.metadata, c)?;
            check(t.desired_quantity > 0, "zero stock target")?;
        }
    }
    for (buyer, e) in &r.market_errands {
        actor(c, buyer)?;
        let p = r
            .stock_plans
            .iter()
            .find(|p| p.id == e.plan_id && &p.buyer == buyer)
            .ok_or_else(|| err("market errand plan binding mismatch"))?;
        check(
            e.spent_sparks <= p.max_spend_sparks,
            "market spend exceeds budget",
        )?;
        check(e.bindings_seen.len() <= MAX_PLANS, "market binding count")?;
        unique(&e.bindings_seen, "duplicate market binding")?;
        for b in &e.bindings_seen {
            binding(b, r, c)?;
        }
        if let Some(b) = &e.selected {
            binding(b, r, c)?;
            check(
                e.bindings_seen.contains(b),
                "selected counter not in seen bindings",
            )?;
        }
        if let Some(s) = &e.last_failed_fingerprint {
            text(s)?;
        }
        for t in [e.travel_deadline_real, e.deadline_hold_began_real]
            .into_iter()
            .flatten()
        {
            logical(OWNER, t)?;
        }
        // Waiting-for-open can retain a hold marker before a walk deadline exists.
    }
    for (id, v) in &r.closed_market_visits {
        check(
            id == &v.plan_id && ids.contains(id),
            "closed market plan missing/mismatch",
        )?;
        check(v.bindings_seen.len() <= MAX_PLANS, "closed binding count")?;
        unique(&v.bindings_seen, "duplicate closed binding")?;
        for b in &v.bindings_seen {
            binding(b, r, c)?;
        }
    }
    Ok(())
}
fn production(r: &Round, c: RoundCheckpointContext<'_>) -> Result<()> {
    check(
        r.production_plans.len() <= MAX_PLANS,
        "production plan count limit",
    )?;
    let mut producers = BTreeSet::new();
    for p in &r.production_plans {
        actor(c, &p.producer)?;
        check(
            producers.insert(&p.producer),
            "duplicate production producer",
        )?;
        check(
            p.max_jobs_per_day > 0 && p.transforms.len() <= MAX_LEGS,
            "invalid production plan bounds",
        )?;
        let mut ids = BTreeSet::new();
        for t in &p.transforms {
            name(&t.id)?;
            name(&t.site)?;
            point(t.point)?;
            offices(&t.allowed_offices)?;
            check(ids.insert(&t.id), "duplicate transform spec identity")?;
            check(
                t.work_minutes > 0
                    && t.desired_output_quantity > 0
                    && !t.consumes.is_empty()
                    && !t.produces.is_empty(),
                "empty transform recipe/work",
            )?;
            stocks(&t.consumes, c)?;
            stocks(&t.produces, c)?;
        }
    }
    for ((id, d), n) in &r.production_starts {
        actor(c, id)?;
        day(*d)?;
        check(*n > 0, "zero production start count")?;
    }
    for id in r.production_was_eligible.keys() {
        actor(c, id)?;
    }
    for (id, w) in &r.transform_stall_watch {
        actor(c, id)?;
        name(&w.job_id)?;
        nonnegative(w.progress_work_minutes, 1.0e12, "invalid watchdog progress")?;
        calendar(OWNER, w.unchanged_since_game_days)?;
        // World APIs may complete/replace a job after the production pump. The
        // watch intentionally samples it on the next pass and resets if changed.
    }
    // Inventory admits purpose-neutral jobs without an origin tag. A manual
    // job may name a known spec and use arbitrary valid IDs/input/output lines;
    // Round uses that spec for work/eligibility, while Inventory owns its recipe.
    // Canonical planner output/ID behavior is witnessed through live admission
    // tests, not inferred from a string prefix during validation.
    Ok(())
}
