//! Focused M5 authority tests. These fixtures stay deliberately small: they
//! exercise inventory, presence, transforms, negotiated transfers, and
//! settlement without depending on authored-world pathfinding.

use std::collections::{BTreeMap, BTreeSet};

use cathedral_sim::{
    ActorId, Character, CharacterSheet, Control, EconomicClass, InventoryErrorCode, ItemMatcher,
    LoreCharacterSheet, MarketRequestLine, Offer, Presence, ReservedInput, Round, StockSpec,
    TransformJob, Vec3, World, apply_action,
};
use serde_json::json;

fn actor(id: &str, class: EconomicClass) -> Character {
    let control = if id == "player" {
        Control::Player
    } else {
        Control::Llm
    };
    Character::from_sheet(CharacterSheet {
        pockets: Vec::new(),
        frontbutt: None,
        id: ActorId::from_raw(id),
        name: id.to_uppercase(),
        control,
        back_story: format!("{id} has a durable story."),
        location_description: "the test counter".into(),
        appearance: Default::default(),
        voice_key: None,
        position_m: Vec3::ZERO,
        facing_yaw: 0.0,
        holds: Vec::new(),
        goal: "Keep the tally".into(),
        memories: vec!["A durable memory".into()],
        knows: BTreeSet::new(),
        lore: None,
        presence: Presence::InCity,
        presence_epoch: 0,
        economic_class: class,
    })
}

fn stock(kind: &str, quantity: u32) -> StockSpec {
    StockSpec {
        kind: kind.into(),
        metadata: BTreeMap::new(),
        quantity,
    }
}

fn add_actor(world: &mut World, id: &str, class: EconomicClass) -> ActorId {
    let id = ActorId::from_raw(id);
    world.add_character(actor(id.as_str(), class));
    id
}

fn total(world: &World, matcher: &ItemMatcher) -> u64 {
    world
        .items
        .values()
        .filter(|item| matcher.matches(item))
        .map(|item| u64::from(item.quantity))
        .sum()
}

fn assert_authored_context(source: &str, id: &str, phrases: &[&str]) -> LoreCharacterSheet {
    let sheet = LoreCharacterSheet::from_json_str(source).expect("the authored sheet validates");
    assert_eq!(sheet.id.as_str(), id);
    let context = format!(
        "{} {}",
        sheet.core_character_description, sheet.extended_character_description
    )
    .to_lowercase();
    for phrase in phrases {
        assert!(
            context.contains(&phrase.to_lowercase()),
            "{id}'s prompt context is missing {phrase:?}"
        );
    }
    sheet
}

#[test]
fn authored_supply_chain_context_locks_the_routes_kin_cargo_and_oven() {
    let hugh = assert_authored_context(
        include_str!("../../../lore/characters/merchant/rbrde_hugh_crake.json"),
        "rbrde",
        &[
            "born above a combing shed in Brede",
            "Clemence Crake is your mother's first cousin",
            "Renn brokers your unloading",
            "Betriss Skep",
            "grain and raw wool",
            "broadcloth",
            "the road is honest",
        ],
    );
    for known in ["fp6ck", "fr9ck", "p008s"] {
        assert!(hugh.knows.iter().any(|id| id.as_str() == known));
    }

    let mabile = assert_authored_context(
        include_str!("../../../lore/characters/merchant/rlant_mabile_skell.json"),
        "rlant",
        &[
            "come from Ostrelle",
            "Lantern Road",
            "Ewart Skell is your dead husband's uncle",
            "Betriss Skep",
            "grain inward",
            "kersey outward",
            "long argument with hills",
        ],
    );
    for known in ["e1skl", "p008s"] {
        assert!(mabile.knows.iter().any(|id| id.as_str() == known));
    }

    assert_authored_context(
        include_str!("../../../lore/characters/food_provisioner/p008s_betriss_skep.json"),
        "p008s",
        &["Seven Lofts", "wrong loft", "tally stick", "short credit"],
    );
    assert_authored_context(
        include_str!("../../../lore/characters/miller/e7mil_bertran_hobbe.json"),
        "e7mil",
        &["Wool Gate face", "bread-corn released from Seven Lofts"],
    );
    assert_authored_context(
        include_str!("../../../lore/characters/baker/danqn_ansel_quern.json"),
        "danqn",
        &["common oven", "bake is cut short", "shouts and haggles"],
    );
    assert_authored_context(
        include_str!("../../../lore/characters/baker/davqn_averil_quern.json"),
        "davqn",
        &[
            "night bake",
            "in at the Watch",
            "Kindling",
            "two sparks",
            "Dayspring",
        ],
    );
}

#[test]
fn departure_filters_owned_state_and_reentry_uses_a_new_epoch() {
    let mut world = World::new();
    let player = add_actor(&mut world, "player", EconomicClass::Visitor);
    let road = add_actor(&mut world, "road1", EconomicClass::RoadParty);
    world
        .characters
        .get_mut(&road)
        .unwrap()
        .state
        .knows
        .insert(player.clone());
    world
        .characters
        .get_mut(&road)
        .unwrap()
        .state
        .inbox
        .push("Unread city news".into());
    world
        .characters
        .get_mut(&road)
        .unwrap()
        .state
        .pending_history
        .push("Drained news".into());
    world
        .characters
        .get_mut(&road)
        .unwrap()
        .state
        .recent_history
        .push("Old road news".into());
    let grain = world
        .add_stock(&road, &stock("grain", 4), "presence:grain")
        .unwrap();
    world.offers.insert(
        grain.clone(),
        Offer {
            item_id: grain.clone(),
            giver_id: road.clone(),
            target_id: Some(player.clone()),
            created_seq: 1,
            quantity: 1,
        },
    );

    let revision = world.world_revision;
    world
        .transition_presence(
            std::slice::from_ref(&road),
            Presence::BeyondTheWalls,
            &BTreeMap::new(),
        )
        .unwrap();
    assert_eq!(world.world_revision, revision + 1);
    assert_eq!(world.presence_epoch(&road), Some(1));
    assert!(world.characters[&road].state.inbox.is_empty());
    assert!(world.characters[&road].state.pending_history.is_empty());
    assert_eq!(world.characters[&road].recent_history(), ["Old road news"]);
    assert_eq!(world.characters[&road].memories(), ["A durable memory"]);
    assert_eq!(world.characters[&road].goal(), "Keep the tally");
    assert!(world.characters[&road].knows().contains(&player));
    assert!(
        world.offers.is_empty(),
        "departure releases every remaining city offer"
    );

    let absent = world.public_snapshot(&player);
    assert!(absent.actors.iter().all(|actor| actor.id != road));
    assert!(absent.items.iter().all(|item| item.id != grain));
    assert!(absent.offers.is_empty());

    let revision = world.world_revision;
    world
        .transition_presence(
            std::slice::from_ref(&road),
            Presence::InCity,
            &BTreeMap::from([(road.clone(), Vec3::new(7.0, 0.91, 3.0))]),
        )
        .unwrap();
    assert_eq!(world.world_revision, revision + 1);
    assert_eq!(world.presence_epoch(&road), Some(2));
    assert!(
        world
            .public_snapshot(&player)
            .actors
            .iter()
            .any(|actor| actor.id == road)
    );
    world.assert_invariants();
}

#[test]
fn catalog_sale_is_atomic_conserving_and_respects_offers() {
    let mut world = World::new();
    let buyer = add_actor(&mut world, "buyer", EconomicClass::Resident);
    let seller = add_actor(&mut world, "seller", EconomicClass::Resident);
    world
        .credit_sparks(&buyer, 20, "sale:buyer-wallet")
        .unwrap();
    world
        .credit_sparks(&seller, 2, "sale:seller-wallet")
        .unwrap();
    let grain = world
        .add_stock(&seller, &stock("grain", 4), "sale:grain")
        .unwrap();
    world.offers.insert(
        grain.clone(),
        Offer {
            item_id: grain.clone(),
            giver_id: seller.clone(),
            target_id: Some(buyer.clone()),
            created_seq: 1,
            quantity: 1,
        },
    );
    let sparks_before = total(&world, &ItemMatcher::new("spark"));
    let grain_before = total(&world, &ItemMatcher::new("grain"));
    let revision = world.world_revision;

    let receipt = world
        .market_sale(
            &buyer,
            &seller,
            &[MarketRequestLine {
                matcher: ItemMatcher::new("grain"),
                quantity: 4,
            }],
            12,
            "sale:catalog-grain",
        )
        .unwrap();
    assert_eq!(receipt.total_sparks, 9);
    assert_eq!(
        receipt.lines.iter().map(|line| line.quantity).sum::<u32>(),
        3
    );
    assert_eq!(receipt.lines[0].unit_price_sparks, 3);
    assert_eq!(world.uncommitted_quantity(&grain), 0);
    assert_eq!(world.offered_quantity(&grain), 1);
    assert_eq!(total(&world, &ItemMatcher::new("spark")), sparks_before);
    assert_eq!(total(&world, &ItemMatcher::new("grain")), grain_before);
    assert_eq!(world.world_revision, revision + 1);
    let events = world.drain_events();
    assert_eq!(
        events.iter().filter(|event| event.kind == "sale").count(),
        1
    );

    let generic = world
        .add_stock(&seller, &stock("generic", 1), "sale:unpriced")
        .unwrap();
    let buyer_cash = world.wallet_sparks(&buyer);
    let seller_cash = world.wallet_sparks(&seller);
    let revision = world.world_revision;
    let error = world
        .market_sale(
            &buyer,
            &seller,
            &[MarketRequestLine {
                matcher: ItemMatcher::new("generic"),
                quantity: 1,
            }],
            20,
            "sale:must-rollback",
        )
        .unwrap_err();
    assert_eq!(error.code, InventoryErrorCode::UnpricedStock);
    assert_eq!(world.wallet_sparks(&buyer), buyer_cash);
    assert_eq!(world.wallet_sparks(&seller), seller_cash);
    assert_eq!(world.items[&generic].quantity, 1);
    assert_eq!(world.world_revision, revision);
    world.assert_invariants();
}

#[test]
fn non_stackable_stock_stays_distinct_and_unknown_wallets_are_rejected() {
    let mut world = World::new();
    let owner = add_actor(&mut world, "owner", EconomicClass::Resident);

    let first = world
        .add_stock(&owner, &stock("stew", 1), "stock:stew:first")
        .unwrap();
    let second = world
        .add_stock(&owner, &stock("stew", 1), "stock:stew:second")
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(world.items[&first].quantity, 1);
    assert_eq!(world.items[&second].quantity, 1);
    assert!(world.characters[&owner].holds().contains(&first));
    assert!(world.characters[&owner].holds().contains(&second));

    let unknown = ActorId::from_raw("unknown");
    assert_eq!(
        world
            .settle_wallet_exact(&unknown, 0, "wallet:unknown:settle")
            .unwrap_err()
            .code,
        InventoryErrorCode::UnknownActor
    );
    assert_eq!(
        world.debit_sparks(&unknown, 0).unwrap_err().code,
        InventoryErrorCode::UnknownActor
    );
    world.assert_invariants();
}

#[test]
fn transform_reservations_capacity_and_completion_replay_are_exact() {
    let mut world = World::new();
    let producer = add_actor(&mut world, "miller", EconomicClass::Resident);
    let helper = add_actor(&mut world, "helper", EconomicClass::Resident);
    let grain = world
        .add_stock(&producer, &stock("grain", 2), "transform:grain")
        .unwrap();
    let flour = world
        .add_stock(
            &producer,
            &stock("flour", u32::MAX - 3),
            "transform:flour-cap",
        )
        .unwrap();
    let inbound = world
        .add_stock(&helper, &stock("flour", 1), "transform:inbound")
        .unwrap();
    let job_id = "miller:mill_grain:7:0";
    world
        .start_transform_job(TransformJob {
            job_id: job_id.into(),
            spec_id: "mill_grain".into(),
            producer: producer.clone(),
            production_day: 7,
            start_slot: 0,
            inputs: vec![ReservedInput {
                item_id: grain.clone(),
                quantity: 1,
            }],
            outputs: vec![stock("flour", 3)],
            progress_work_minutes: 45.0,
        })
        .unwrap();

    let error = world
        .transfer_item_quantity(&producer, &helper, &grain, 2, "transform:whole-transfer")
        .unwrap_err();
    assert_eq!(error.code, InventoryErrorCode::ItemCommitted);
    world
        .transfer_item_quantity(&producer, &helper, &grain, 1, "transform:partial-transfer")
        .unwrap();
    assert_eq!(world.items[&grain].quantity, 1);
    assert_eq!(world.transform_reserved_quantity(&grain), 1);

    let error = world
        .transfer_item_quantity(&helper, &producer, &inbound, 1, "transform:capacity-gift")
        .unwrap_err();
    assert_eq!(error.code, InventoryErrorCode::OutputCapacityReserved);
    assert!(world.characters[&helper].holds().contains(&inbound));

    let receipt = world
        .complete_transform_job_by_id(&producer, job_id, 10)
        .unwrap();
    assert!(!world.items.contains_key(&grain));
    assert_eq!(world.items[&flour].quantity, u32::MAX);
    let replay = world
        .complete_transform_job_by_id(&producer, job_id, 10)
        .unwrap();
    assert_eq!(replay, receipt);
    assert_eq!(world.items[&flour].quantity, u32::MAX);

    world.prune_completed_transforms(12);
    let before = world.items[&flour].quantity;
    let error = world
        .complete_transform_job_by_id(&producer, job_id, 12)
        .unwrap_err();
    assert_eq!(error.code, InventoryErrorCode::NoActiveTransformJob);
    assert_eq!(world.items[&flour].quantity, before);
    world.assert_invariants();
}

#[test]
fn legacy_provenance_never_follows_a_transfer_or_sweeps_returned_stock() {
    let mut world = World::new();
    let vendor = add_actor(&mut world, "vendor", EconomicClass::Resident);
    let customer = add_actor(&mut world, "custom", EconomicClass::Resident);
    let real = world
        .add_stock(&vendor, &stock("herring", 2), "legacy:real")
        .unwrap();
    let merged = world
        .add_legacy_restock(
            &vendor,
            "legacy_stall:test",
            &stock("herring", 3),
            "legacy:day1",
        )
        .unwrap();
    assert_eq!(merged, real);
    assert_eq!(world.items[&real].quantity, 5);
    assert_eq!(world.legacy_restock_shares(&real)[0].quantity, 3);

    let moved = world
        .transfer_item_quantity(&vendor, &customer, &real, 2, "legacy:out")
        .unwrap();
    assert!(world.legacy_restock_shares(&moved).is_empty());
    assert_eq!(world.legacy_restock_shares(&real)[0].quantity, 1);
    world
        .transfer_item_quantity(&customer, &vendor, &moved, 2, "legacy:return")
        .unwrap();
    assert_eq!(world.items[&real].quantity, 5);
    assert_eq!(world.sweep_legacy_restock("legacy_stall:test").unwrap(), 1);
    assert_eq!(
        world.items[&real].quantity, 4,
        "returned real quantity survives"
    );

    world
        .add_legacy_restock(
            &vendor,
            "legacy_stall:test",
            &stock("herring", 3),
            "legacy:day2",
        )
        .unwrap();
    assert_eq!(world.items[&real].quantity, 7);
    assert_eq!(world.sweep_legacy_restock("legacy_stall:test").unwrap(), 3);
    assert_eq!(world.items[&real].quantity, 4);
    world.assert_invariants();
}

#[test]
fn settlement_uses_spendable_resident_balances_and_leaves_stock_and_visitors_alone() {
    let mut world = World::new();
    let recipient = add_actor(&mut world, "resid", EconomicClass::Resident);
    let penniless = add_actor(&mut world, "zero0", EconomicClass::Resident);
    let visitor = add_actor(&mut world, "visit", EconomicClass::Visitor);
    let road = add_actor(&mut world, "road0", EconomicClass::RoadParty);
    world
        .credit_sparks(&recipient, 5, "settlement:resident")
        .unwrap();
    world
        .credit_sparks(&visitor, 1, "settlement:visitor")
        .unwrap();
    world.credit_sparks(&road, 2, "settlement:road").unwrap();
    let fish = world
        .add_stock(&visitor, &stock("herring", 3), "settlement:stock")
        .unwrap();
    let purse = world.characters[&recipient]
        .holds()
        .iter()
        .find(|id| world.items[*id].kind.as_str() == "spark")
        .cloned()
        .unwrap();
    apply_action(
        &mut world,
        &recipient,
        "offer_item",
        &json!({"item_id": purse.as_str(), "quantity": 3, "target": visitor.as_str()}),
    )
    .unwrap();
    world.drain_events();
    assert_eq!(world.spendable_sparks(&recipient), 2);

    let revision = world.world_revision;
    let receipt = Round::new().settle_households(&mut world, 9).unwrap();
    assert_eq!(receipt.institutional_payroll_sparks, 6);
    assert_eq!(world.spendable_sparks(&recipient), 4);
    assert_eq!(world.spendable_sparks(&penniless), 4);
    assert_eq!(world.wallet_sparks(&visitor), 1);
    assert_eq!(world.wallet_sparks(&road), 2);
    assert_eq!(world.items[&fish].quantity, 3);
    assert_eq!(world.offered_quantity(&purse), 3);
    assert_eq!(world.world_revision, revision + 1);
    world.assert_invariants();
}

#[test]
fn negotiated_transfer_is_traced_but_never_masquerades_as_a_sale() {
    let mut world = World::new();
    let giver = add_actor(&mut world, "giver", EconomicClass::Resident);
    let taker = add_actor(&mut world, "taker", EconomicClass::Resident);
    let cloth = world
        .add_stock(
            &giver,
            &StockSpec {
                kind: "cloth".into(),
                metadata: BTreeMap::from([("grade".into(), "kersey".into())]),
                quantity: 1,
            },
            "negotiated:cloth",
        )
        .unwrap();
    apply_action(
        &mut world,
        &giver,
        "offer_item",
        &json!({"item_id": cloth.as_str(), "target": taker.as_str()}),
    )
    .unwrap();
    world.drain_events();
    apply_action(
        &mut world,
        &taker,
        "accept_offered_item",
        &json!({"item_id": cloth.as_str()}),
    )
    .unwrap();
    let events = world.drain_events();
    assert!(events.iter().any(|event| event.kind == "item_transfer"));
    assert!(events.iter().all(|event| event.kind != "sale"));
    assert!(world.characters[&taker].holds().contains(&cloth));
    world.assert_invariants();
}
