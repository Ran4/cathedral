//! Pure stored-owner contracts. Historical attribution is not current truth.
use super::*;
pub(crate) fn boundary(
    version: u16,
    now: LogicalTime,
    binding: &context::BindingV1,
    time: Option<WorldTime>,
    c: KnowledgeCheckpointContext<'_>,
) -> Result<()> {
    check(version == 1, "unsupported knowledge version")?;
    checkpoint::logical(OWNER, now.seconds())?;
    check(now == c.now, "knowledge boundary disagreement")?;
    check(
        *binding == context::BindingV1::new(c)?,
        "knowledge context disagreement",
    )?;
    let same = match (time, c.backbone.current_time) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.day == b.day
                && a.weekday == b.weekday
                && a.office == b.office
                && a.fraction.to_bits() == b.fraction.to_bits()
        }
        _ => false,
    };
    check(same, "knowledge sampled time disagrees with backbone")?;
    if let Some(t) = time {
        checkpoint::calendar(OWNER, t.game_days())?;
        check(
            t.fraction.is_finite()
                && (0.0..1.0).contains(&t.fraction)
                && t.weekday == crate::clock::Weekday::of_day(t.day)
                && t.office == WorldTime::from_game_days(t.fraction).office,
            "invalid knowledge sampled time",
        )?;
    }
    Ok(())
}
fn actor(a: &ActorId) -> Result<()> {
    id(a.as_str())
}
fn optional_actor(a: &Option<ActorId>) -> Result<()> {
    if let Some(a) = a {
        actor(a)?;
    }
    Ok(())
}
fn area(a: Option<AreaKey>, c: KnowledgeCheckpointContext<'_>) -> Result<()> {
    check(
        a.is_none_or(|k| usize::from(k.0) < c.areas.areas.len()),
        "knowledge area key outside installed map",
    )
}
fn stamp(t: Option<f64>) -> Result<()> {
    if let Some(t) = t {
        checkpoint::calendar(OWNER, t)?;
    }
    Ok(())
}
fn heat(h: f32) -> Result<()> {
    check(
        h.is_finite() && (0.0..=1.0).contains(&h),
        "invalid knowledge heat",
    )
}
fn names<'a>(v: impl IntoIterator<Item = &'a ActorId>) -> Result<()> {
    let mut n = 0;
    for a in v {
        n += 1;
        check(n <= MAX_NAMES, "knowledge name count")?;
        actor(a)?;
    }
    Ok(())
}
pub(crate) fn knowledge(k: &Knowledge, c: KnowledgeCheckpointContext<'_>) -> Result<()> {
    check(k.live.len() <= FACTS_MAX_LIVE, "live fact count")?;
    // V1 admission retains a complete live-store turnover of fresh handles.
    // Sparse handles and failed-install holes are valid; no contiguous range or
    // current key/sequence equality is inferred. Lifetime exhaustion is still a
    // whole-envelope horizon gate, not repaired by saturating counters.
    check(
        k.next_key <= u32::MAX - (FACTS_MAX_LIVE as u32 + 1)
            && (0..=i64::MAX - (FACTS_MAX_LIVE as i64 + 1)).contains(&k.next_sequence),
        "fact allocator lacks supported headroom",
    )?;
    check(
        k.by_id.len() == k.live.len(),
        "fact id index size disagreement",
    )?;
    let mut sequences = BTreeSet::new();
    for (key, f) in &k.live {
        check(
            *key == f.key && key.0 < k.next_key,
            "fact key or allocator disagreement",
        )?;
        check(
            f.sequence >= 0 && f.sequence < k.next_sequence && sequences.insert(f.sequence),
            "fact sequence or allocator disagreement",
        )?;
        id(f.id.as_str())?;
        check(
            k.by_id.get(&f.id) == Some(key),
            "fact id index disagreement",
        )?;
        names(&f.subject)?;
        names(f.own.keys())?;
        names(&f.seeded)?;
        names(&f.quiet_among)?;
        text(&f.said)?;
        for s in f.own.values() {
            text(s)?;
        }
        if let Some(s) = &f.craft_ear {
            text(s)?;
        }
        area(f.place, c)?;
        stamp(f.minted_game_days)?;
        super::super::source::checkpoint::validate(&f.source)?;
        // Frozen subjects/seeded/quiet/craft and provenance can refer to actors
        // or items no longer present. The next ordinary sweep decides truth.
    }
    for (id_, key) in &k.by_id {
        id(id_.as_str())?;
        check(
            k.live.get(key).is_some_and(|f| f.id == *id_),
            "orphan fact id index",
        )?;
    }
    check(k.holdings.len() <= MAX_NAMES, "holding actor count")?;
    for (who, rows) in k.holdings.iter() {
        actor(who)?;
        check(
            !rows.is_empty() && rows.len() <= HOLDINGS_MAX,
            "holding row count",
        )?;
        check(
            rows.windows(2).all(|p| p[0].key < p[1].key),
            "holding keys must be unique and ordered",
        )?;
        for h in rows {
            let f = k
                .live
                .get(&h.key)
                .ok_or_else(|| error("holding has missing live fact"))?;
            heat(h.heat_at_learn)?;
            stamp(h.learned_on)?;
            optional_actor(&h.from)?;
            optional_actor(&h.view.subject)?;
            area(h.view.place, c)?;
            check(
                (-DAY_OFFSET_MAX..=DAY_OFFSET_MAX).contains(&h.view.day_offset),
                "holding day offset",
            )?;
            check(
                (h.view.subject.is_none() || f.garble.subject)
                    && (h.view.place.is_none() || f.garble.place)
                    && (h.view.day_offset == 0 || f.garble.day),
                "holding view exceeds fact garble mask",
            )?;
            if f.seeded.contains(who) {
                check(
                    h.hops == 0 && h.view.is_pristine(),
                    "seeded holding lost first-hand identity",
                )?;
            }
        }
        // learn/note_occasion accept bounded names without a World existence
        // check. They are name-keyed knowledge, not exclusive entity ownership.
    }
    check(
        k.air.len() <= PlanningWard::ALL.len() * AIR_PER_WARD_MAX,
        "air count",
    )?;
    for ward in PlanningWard::ALL {
        check(
            k.ward_air(ward).count() <= AIR_PER_WARD_MAX,
            "ward air count",
        )?;
    }
    for ((_, key), d) in k.air.iter() {
        check(k.live.contains_key(key), "air has missing live fact")?;
        heat(d.heat)?;
        optional_actor(&d.via)?;
    }
    checkpoint::CalendarAnchorV1::from_legacy(k.last_sweep_game_days)?;
    checkpoint::CalendarAnchorV1::from_legacy(k.last_hearsay_beat_game_days)?;
    check(
        k.occasions.len() <= MAX_NAMES && k.raises.len() <= MAX_NAMES,
        "occasion or raise count",
    )?;
    for (who, o) in &k.occasions {
        actor(who)?;
        optional_actor(&o.subject)?;
        optional_actor(&o.from)?;
        checkpoint::calendar(OWNER, o.at_game_days)?;
    }
    // Raises counts saturate at u8::MAX through the public writer; stale office
    // keys and unused allowances remain until that actor raises again.
    for who in k.raises.keys() {
        actor(who)?;
    }
    check(
        k.player_learned.len() <= PLAYER_RECEIPTS_MAX,
        "player receipt count",
    )?;
    for (fact, r) in &k.player_learned {
        id(fact.as_str())?;
        text(&r.word)?;
        stamp(r.at)?;
        area(r.place, c)?;
        optional_actor(&r.from)?;
        names(&r.mouths_seen)?;
        check(
            r.mouths_seen.len() <= PLAYER_RECEIPTS_MAX,
            "receipt mouth count",
        )?;
        check(
            r.tellings as usize == r.mouths_seen.len() + usize::from(r.unattributed_seen),
            "receipt telling count disagreement",
        )?;
        check(
            r.wards == r.wards_seen.count_ones() as u8,
            "receipt ward count disagreement",
        )?;
        // Receipts retain their own prose/attribution after fact invalidation.
    }
    if let Some((who, keys)) = &k.seated {
        actor(who)?;
        check(
            keys.len() <= KNOWN_SHEET_MAX && keys.iter().all(|key| key.0 < k.next_key),
            "seated key count or allocation disagreement",
        )?;
    }
    check(
        k.player_stage_tellings.len() <= FACTS_MAX_LIVE * STAGE_HOP_MAX_PAIRS,
        "stage telling count",
    )?;
    check(
        k.player_stage_stir.is_some() || k.player_stage_tellings.is_empty(),
        "stage tellings lack stir",
    )?;
    for (key, who) in &k.player_stage_tellings {
        actor(who)?;
        check(
            k.live.contains_key(key),
            "stage telling has missing live fact",
        )?;
    }
    check(
        k.hearsay_raised.len() <= FACTS_MAX_LIVE * MAX_NAMES,
        "hearsay pair count",
    )?;
    for (key, who) in &k.hearsay_raised {
        actor(who)?;
        let f = k
            .live
            .get(key)
            .ok_or_else(|| error("hearsay pair has missing live fact"))?;
        check(
            f.topic == Topic::Law && !f.subject.contains(who),
            "hearsay pair is not a wrong-name law telling",
        )?;
    }
    Ok(())
}
