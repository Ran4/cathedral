//! Consequences exercised through the real purchase, service and round paths.
use super::*;
use crate::knowledge::{self, FactCatalog, FactView, Telling};

fn coin_word(world: &mut World, vendor: &ActorId, buyer: &ActorId) -> crate::FactKey {
    let row = serde_json::json!({"schema_version":1,"facts":[{
        "id":"refusal.coin","topic":"coin","said":"{subject} owes at the board",
        "subject":[buyer.as_str()],"seeded":[buyer.as_str()],"day":0
    }]});
    let catalog = FactCatalog::from_json(&row.to_string()).unwrap();
    let key = catalog
        .seed_one(world, &crate::FactId::from_raw("refusal.coin"))
        .unwrap();
    knowledge::learn(
        world,
        vendor,
        key,
        Telling {
            from: Some(buyer.clone()),
            hops: 1,
            heat: 1.0,
            view: FactView::default(),
        },
        world.current_time.map(|t| t.game_days()),
    );
    key
}

fn queue(round: &mut Round, buyer: &ActorId, now: f64) {
    let pitch = round.stalls[0].pitch;
    round
        .people
        .entry(buyer.clone())
        .or_insert_with(|| weather_person(pitch))
        .food = Some(FoodErrand {
        stall: 0,
        phase: FoodPhase::Queued,
    });
    round.stalls[0].queue = vec![buyer.clone()];
    round.stalls[0].serving = Some((buyer.clone(), now));
}

#[test]
fn a_vendor_who_has_heard_of_you_will_not_take_your_coin() {
    let (mut world, mut round, vendor, buyer, stock) = bread_stall_world();
    let key = coin_word(&mut world, &vendor, &buyer);
    let purse = world.wallet_sparks(&buyer);
    assert!(try_purchase(&mut round, &mut world, 0, &buyer).is_none());
    assert_eq!(world.wallet_sparks(&buyer), purse);
    assert_eq!(world.items[&stock].quantity, 3);
    world.knowledge_enabled = false;
    assert!(try_purchase(&mut round, &mut world, 0, &buyer).is_some());
    world.knowledge_enabled = true;
    world.knowledge.invalidate(key);
    assert!(try_purchase(&mut round, &mut world, 0, &buyer).is_some());
}

#[test]
fn a_refused_buyer_does_not_re_queue() {
    let (mut world, mut round, vendor, buyer, _) = bread_stall_world();
    let clock = clock_at(Office::HighWick);
    world.current_time = Some(clock.at(0.0));
    coin_word(&mut world, &vendor, &buyer);
    round.seeded = true;
    queue(&mut round, &buyer, 0.0);
    let navigation = nav();
    for n in 0..20 {
        let now = PURCHASE_SECONDS + 0.1 + n as f64 * 20.0;
        world.current_time = Some(clock.at(now));
        tick(
            &mut round,
            &mut world,
            &navigation,
            &clock,
            now,
            &player(),
            &BTreeSet::new(),
        );
        assert!(
            round.people[&buyer].food.is_none(),
            "the actual decision ladder must not requeue"
        );
    }
    assert_eq!(
        world.characters[&buyer]
            .inbox()
            .iter()
            .filter(|line| line.contains("heard") && line.contains("coin"))
            .count(),
        1
    );
    assert_eq!(
        round
            .food_log
            .iter()
            .filter(|line| line.starts_with("refused_on_word;"))
            .count(),
        1
    );
    let bytes = knowledge_auxiliary_bytes(&round);
    assert!(bytes > 100);
    world.characters.remove(&buyer);
    tick(
        &mut round,
        &mut world,
        &navigation,
        &clock,
        401.0,
        &player(),
        &BTreeSet::new(),
    );
    assert!(round.knowledge_refused_until.is_empty() && round.knowledge_refused_buyers.is_empty());
    assert!(knowledge_auxiliary_bytes(&round) < bytes);
}

#[test]
fn a_bound_vendor_refuses_and_the_buyer_is_not_starved() {
    let (mut world, mut round, vendor, buyer, _) = bread_stall_world();
    let clock = clock_at(Office::HighWick);
    world.current_time = Some(clock.at(0.0));
    coin_word(&mut world, &vendor, &buyer);
    round.seeded = true;
    queue(&mut round, &buyer, 0.0);
    // The actual tick services the queue and returns no paid-turn nudge.
    let now = PURCHASE_SECONDS + 0.1;
    let nudges = tick(
        &mut round,
        &mut world,
        &nav(),
        &clock,
        now,
        &player(),
        &warm(&buyer),
    );
    assert!(!nudges.contains(&buyer));
    let inbox = world.characters[&buyer].inbox().to_vec();
    assert_eq!(inbox.len(), 1, "{inbox:?}");
    assert!(inbox[0].contains("heard") && inbox[0].contains("coin"));
    for forbidden in [
        "fact", "hop", "heat", "band", "salience", "topic", "rumor", "rumour", "store",
    ] {
        assert!(
            !inbox[0].contains(forbidden),
            "mechanism word in refusal: {forbidden}"
        );
    }
    assert_eq!(
        round
            .food_log
            .iter()
            .filter(|line| line.starts_with("refused_on_word;"))
            .count(),
        1
    );
    assert!(round.people[&buyer].food.is_none());
    for n in 1..20 {
        queue(&mut round, &buyer, n as f64);
        let nudges = tick(
            &mut round,
            &mut world,
            &nav(),
            &clock,
            n as f64 + now,
            &player(),
            &warm(&buyer),
        );
        assert!(!nudges.contains(&buyer));
    }
    assert_eq!(world.characters[&buyer].inbox().len(), 1);
    assert_eq!(
        round
            .food_log
            .iter()
            .filter(|line| line.starts_with("refused_on_word;"))
            .count(),
        1
    );
    assert!(
        nearest_open_stall(
            &round,
            &world,
            &buyer,
            Vec3::ZERO,
            Office::HighWick,
            Weekday::Bellday,
            false
        )
        .is_none()
    );

    // The clock is paused: another tick at the same game instant cannot age
    // the backoff. No wall-clock sleeps participate in this decision.
    tick(
        &mut round,
        &mut world,
        &nav(),
        &clock,
        now + 19.0,
        &player(),
        &warm(&buyer),
    );
    assert!(round.knowledge_refused_until.contains_key(&buyer));
    let until = round.knowledge_refused_until[&buyer];
    let later = (until - clock.game_days(0.0)) * clock.seconds_per_day() + 0.1;
    world.current_time = Some(clock.at(later));
    tick(
        &mut round,
        &mut world,
        &nav(),
        &clock,
        later,
        &player(),
        &warm(&buyer),
    );
    assert!(
        !round.knowledge_refused_until.contains_key(&buyer),
        "the real tick prunes game days"
    );

    // A second staffed, affordable board wins after the pause, even though
    // the first board remains the nearest and has plenty of bread.
    let other = ActorId::from_raw("other_baker");
    let mut second = world.characters[&vendor].clone();
    second.sheet.id = other.clone();
    second.state.position_m = Vec3::new(5.0, 0.0, 0.0);
    // Share no inventory: the copied seller must own its own bread and float.
    second.state.holds.clear();
    for (id, kind, quantity) in [("other_loaf", "loaf", 3), ("other_purse", "spark", 6)] {
        let id = ItemId::from_raw(id);
        world.add_item(Item::stack(id.clone(), kind, quantity));
        second.state.holds.push(id);
    }
    world.add_character(second);
    let mut stall = round.stalls[0].clone();
    stall.vendor = Some(other.clone());
    stall.pitch = Vec3::new(5.0, 0.0, 0.0);
    stall.queue.clear();
    stall.serving = None;
    round.stalls.push(stall);
    assert_eq!(
        nearest_open_stall(
            &round,
            &world,
            &buyer,
            Vec3::ZERO,
            Office::HighWick,
            Weekday::Bellday,
            false
        ),
        Some(1)
    );
    assert!(
        try_purchase(&mut round, &mut world, 1, &buyer).is_some(),
        "the alternative actually sells to this buyer"
    );
    let key = world
        .knowledge
        .key_of(&crate::FactId::from_raw("refusal.coin"))
        .unwrap();
    knowledge::learn(
        &mut world,
        &other,
        key,
        Telling {
            from: Some(vendor),
            hops: 1,
            heat: 1.0,
            view: Default::default(),
        },
        Some(clock.game_days(later)),
    );
    assert!(
        nearest_open_stall(
            &round,
            &world,
            &buyer,
            Vec3::ZERO,
            Office::HighWick,
            Weekday::Bellday,
            false
        )
        .is_none()
    );
    // The earlier sale fed them. Once that loaf has been eaten, refusing every
    // board still leaves the ordinary household meal path available.
    let loaf = held_edible(&round, &world, &world.characters[&buyer]).unwrap();
    crate::actions::apply_action(
        &mut world,
        &buyer,
        "eat",
        &serde_json::json!({"item_id":loaf.as_str()}),
    )
    .unwrap();
    let home = Vec3::new(30.0, WALK_Y, 0.0);
    round.people.get_mut(&buyer).unwrap().home = Some(home);
    world.characters.get_mut(&buyer).unwrap().state.needs.hunger = 0.0;
    let (decision, _) = decide(
        &round,
        &world,
        &nav(),
        &buyer,
        0,
        at_office(Office::HighWick, Weekday::Bellday),
    );
    assert!(matches!(decision, Decision::Travel(point) if point == home));
    world.knowledge.invalidate(key);
    assert_eq!(
        nearest_open_stall(
            &round,
            &world,
            &buyer,
            Vec3::ZERO,
            Office::HighWick,
            Weekday::Bellday,
            false
        ),
        Some(0)
    );
}
