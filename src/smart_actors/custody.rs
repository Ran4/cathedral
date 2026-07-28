//! The player half of custody (`features/law_and_order.md` M4c/M4d).
//!
//! Everything hard about M4 is hard because of the player specifically: they run
//! at 12 m/s against every officer's 1.8, and the host — not the sim — owns
//! their feet. So three things live here and nowhere else.
//!
//! **The grab reflex.** `seize` is the officer declaring intent; the grab that
//! enforces it fires mechanically and instantly, with no provider round trip.
//! Same split as the npc_bodies gaze reflex: reflexes are code, decisions are
//! prompts. It cannot be sim-side — the sim reads the player at
//! `POSITION_UPDATE_HZ = 10`, which is 1.2 m of travel per sample at run speed
//! before the return trip, so a 3 m radius decided over there would be wrong by
//! most of its own radius.
//!
//! **The tether.** A held player keeps their camera — losing your view is
//! nausea, losing your feet is drama — and keeps their input, clamped to
//! [`cathedral_sim::custody::CUSTODY_TETHER_M`] around the grip point. The clamp
//! is applied to the **desired** position in `controller.rs`, *before* the swept
//! solve, so collision always wins: put a market stall between you and the
//! officer and the grip breaks by itself, a free and discoverable escape that
//! costs nothing to build.
//!
//! **The strain meter.** Pull, don't mash: hold a movement direction away from
//! the officer and a meter fills over ~5 s, draining three times as fast the
//! moment you stop. Escape is meant to be easy enough to be a real choice —
//! what should make you hesitate is the consequence, not the difficulty.
//!
//! The sim learns about all three from three commands, and it is those commands
//! — never a sim-side distance check — that earn the holder their percept and
//! their turn.

use bevy::prelude::*;
use cathedral_sim::custody::{CUSTODY_LEASH_M, CUSTODY_REACH_M, CUSTODY_TETHER_M};

use crate::controller::{ControllerInput, PhysicalPosition, PlayerController};

use super::{
    SmartActorSet,
    bridge::{BridgeCommand, BridgeHandle},
    hud::SmartActorHudState,
    model::ActorId,
};

/// How much faster the meter drains than it fills, the moment you stop pulling.
/// Stop for a second and most of it is gone: the mechanic is *pull*, not mash.
const STRAIN_DRAIN_FACTOR: f32 = 3.0;
/// Outward speed below which the player is leaning on the grip rather than
/// pulling against it.
const PULLING_SPEED_MPS: f32 = 0.4;
/// The outward speed that counts as *moving away* rather than milling about
/// beside your escort. A person walks at 2.1 m/s and the player walks at 8, so
/// this separates "stepped aside to look at a stall" from "left".
const FLEEING_SPEED_MPS: f32 = 2.5;

/// What the sim last said about the player's standing, mirrored for the tether,
/// the reflex and the HUD. Purely a projection: nothing here is authoritative,
/// and the sim is told about every change through [`BridgeCommand`].
#[derive(Resource, Debug, Clone, Default)]
pub struct PlayerCustodyState {
    /// `None` while the law has no hands on the player, which is nearly always.
    pub custody: Option<CustodyView>,
    /// `0..=1`. Filled by pulling away from the grip, drained by not.
    pub strain: f32,
    /// The words against the player, kept here beside the custody they are
    /// drawn with. Both halves of the standing line have to live in one place
    /// for [`law_standing_hud`] to be its one writer — see there for why it
    /// must be.
    notices: Vec<cathedral_sim::engine::PlayerNotice>,
    /// Whether the sim has already been told this pull started, so it is told
    /// once and not at 120 Hz.
    struggling_reported: bool,
}

/// The law's hands, as the host needs them.
#[derive(Debug, Clone, PartialEq)]
pub struct CustodyView {
    pub holder_ids: Vec<ActorId>,
    pub officer_id: Option<ActorId>,
    pub officer_name: String,
    pub station_name: String,
    /// The grip point the tether clamps around — and, while nobody has hold
    /// yet, the escorting officer's own position, which is what [`grab_reflex`]
    /// measures arm's reach against. The sim republishes the whole standing
    /// line whenever it moves, so unlike the mirror it is never stale.
    pub anchor_m: Vec3,
    /// The leash has been broken and the officer is coming to take hold. See
    /// [`grab_reflex`] for why this is a latch from the sim rather than a
    /// distance the host could measure for itself.
    pub closing: bool,
    /// How long an unbroken pull must last to tear free of *this* grip — the
    /// sim's number, carrying every modifier (see the module docs).
    pub strain_seconds: f32,
    pub held: bool,
    pub committed: bool,
    /// The posted gaol fee, so the committed line can always name a door.
    pub fee_sparks: u32,
    /// The bell they were told they go at, as the city says it — `"Lamplight"`.
    /// `None` at a station, where the honest answer is "when the keeper says".
    pub release_office: Option<String>,
    /// What the keeper's book says, which is never a name.
    pub booked_as: Option<String>,
}

impl PlayerCustodyState {
    /// The tether the controller clamps the player's *desired* position
    /// against, or `None` when the player's feet are their own. Flying is never
    /// custody: developer flying is not a jailbreak, and pinning a debug camera
    /// would make the mechanic impossible to inspect.
    pub fn tether(&self, flying: bool) -> Option<(Vec3, f32)> {
        if flying {
            return None;
        }
        self.custody
            .as_ref()
            .filter(|custody| custody.held)
            .map(|custody| (custody.anchor_m, CUSTODY_TETHER_M as f32))
    }

    /// Let the player go when the engine dies (`drain_bridge_messages`'s
    /// `Disconnected` arm). Of everything the sim projects into the host this is
    /// the one thing that holds the player's *feet*: the tether clamps them to
    /// [`CUSTODY_TETHER_M`] around a grip point that stops moving the moment the
    /// sim does, and the `LawStanding` that ends a custody can never arrive
    /// afterwards — the drain refuses every message once the engine is known
    /// dead. A hold with nobody left to end it is a permanent one, and the only
    /// way out of it would be the developer flying key.
    ///
    /// The words against them go the same way: a notice names the door that
    /// clears it, and with the sim gone there is no longer anybody to walk
    /// through it. Clearing this also takes the standing line off the screen,
    /// because [`law_standing_hud`] composes that line from nothing else.
    pub(super) fn clear_on_disconnect(&mut self) {
        *self = Self::default();
    }

    /// Test-only: what the sim publishes once `holders` pairs of hands have hold
    /// of the player at `anchor`, with the sim's own `strain_seconds` for that
    /// grip. Shared with `controller.rs`'s tether tests, so both halves of the
    /// seam are exercised against the same projection.
    #[cfg(test)]
    pub(crate) fn held_at(anchor: Vec3, holders: usize, strain_seconds: f32) -> Self {
        Self {
            custody: Some(CustodyView {
                holder_ids: (0..holders)
                    .map(|index| ActorId(format!("sergeant{index}")))
                    .collect(),
                officer_id: Some(ActorId("sergeant0".into())),
                officer_name: "Havise Ashe".into(),
                station_name: "the Bellstand".into(),
                anchor_m: anchor,
                // Already held, so the closing is over by definition — a hand
                // landing is what clears it.
                closing: false,
                strain_seconds,
                held: true,
                committed: false,
                fee_sparks: 3,
                release_office: None,
                booked_as: None,
            }),
            ..Self::default()
        }
    }
}

pub(super) struct PlayerCustodyPlugin;

impl Plugin for PlayerCustodyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerCustodyState>()
            .add_systems(Update, (grab_reflex, strain_meter).chain())
            // The line itself is drawn after the drain has landed the sim's
            // half of it and before the HUD is presented, because that is the
            // only window in the frame where both halves are current. See
            // [`law_standing_hud`].
            .add_systems(
                PostUpdate,
                law_standing_hud
                    .after(SmartActorSet::DrainBridge)
                    .before(SmartActorSet::Present),
            );
    }
}

/// Take the sim's word for the player's standing. The words themselves reach
/// the screen through [`law_standing_hud`], once the meter has also had its say
/// this frame; nothing here writes the HUD.
pub(super) fn apply_law_standing(
    state: &mut PlayerCustodyState,
    notices: &[cathedral_sim::engine::PlayerNotice],
    custody: Option<CustodyView>,
) {
    let was_held = state.custody.as_ref().is_some_and(|custody| custody.held);
    let now_held = custody.as_ref().is_some_and(|custody| custody.held);
    state.custody = custody;
    state.notices = notices.to_vec();
    if was_held && !now_held {
        // Somebody let go, or the dead-man timer did; the meter starts fresh
        // for the next pair of hands.
        state.strain = 0.0;
        state.struggling_reported = false;
    }
}

/// The committed line's header: where they are kept, who keeps them, and what
/// the book says — which is a description and never a name, because nobody in
/// this city knows the player.
fn committed_header(custody: &CustodyView) -> String {
    let mut line = format!(
        "HELD AT {} — {} keeps you here",
        custody.station_name.to_uppercase(),
        custody.officer_name
    );
    if let Some(booked_as) = &custody.booked_as {
        line.push_str(&format!(", booked as {booked_as}"));
    }
    line
}

/// The doors out of a commitment, in the order you would try them. Its own
/// function because the committed line owes them in *every* state a committed
/// player can be in, hand on the arm included — somebody grabbed heading for
/// the doorway must not lose the bell, the fee and the surety hint at exactly
/// the moment they most need them, which is the "never a mystery box" rule
/// failing in its worst case.
fn committed_doors(custody: &CustodyView) -> String {
    let bell = match &custody.release_office {
        Some(office) => format!("You go at {office}."),
        None => "You go when the keeper says.".to_string(),
    };
    format!(
        "{bell} {} {} the posted fee — offer it, send for someone to stand surety, or talk them round.",
        custody.fee_sparks,
        if custody.fee_sparks == 1 { "spark is" } else { "sparks is" },
    )
}

/// The standing line itself, whole: custody first — it is what the player is
/// doing right now — then the meter, if there is a hand to pull against, then
/// the words against them, worst rung first, each with its own door named.
///
/// The meter belongs *in here* rather than in a second write over the top,
/// because the two are one line: a writer that knew only the strain would drop
/// the notices, and a writer that knew only the sim's word would drop the bar.
fn standing_text(
    notices: &[cathedral_sim::engine::PlayerNotice],
    custody: Option<&CustodyView>,
    strain: f32,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(custody) = custody {
        lines.push(match (custody.held, custody.committed) {
            (_, true) => committed_header(custody),
            (true, false) => format!(
                "HELD BY {} — pull away to struggle free",
                custody.officer_name.to_uppercase()
            ),
            (false, false) => format!(
                "{} HAS TAKEN YOU IN CHARGE — {}",
                custody.officer_name.to_uppercase(),
                custody.station_name
            ),
        });
        // A committed player must always be able to read their way out — this
        // is the one standing line the feature's own "a brand with a visible
        // door is a story; a brand with no door is a bug" rule used to miss.
        // Three doors and a bell, in the order you would try them.
        if custody.committed {
            lines.push(committed_doors(custody));
        }
        // The meter, so pulling reads as progress rather than as nothing
        // happening. A committed player can be taken hold of too — the keeper's
        // answer to somebody walking for the door — and there the bar needs its
        // own invitation, because the header above it is about the room and not
        // about the grip.
        if custody.held {
            let filled = (strain.clamp(0.0, 1.0) * 10.0).round() as usize;
            let bar = format!("[{}{}]", "#".repeat(filled), "-".repeat(10 - filled));
            lines.push(if custody.committed {
                format!("Pull away to struggle free  {bar}")
            } else {
                bar
            });
        }
        // The leash, explained once, the first time it is ever drawn.
        if !custody.committed && !custody.held {
            lines.push(format!(
                "Walk with them, or step away — past {:.0} m they will come and take hold of you.",
                CUSTODY_LEASH_M
            ));
        }
    }
    for notice in notices.iter().take(3) {
        let rung = match notice.rung {
            cathedral_sim::notices::Rung::Word => "WORD",
            cathedral_sim::notices::Rung::Summoned => "SUMMONED",
            cathedral_sim::notices::Rung::Warranted => "WARRANT",
        };
        lines.push(format!(
            "{rung}: {} — to clear it: {}",
            notice.line, notice.clears_when
        ));
    }
    lines.join("\n")
}

/// The reflex. Nobody is ever grabbed out of nowhere: the officer has to walk
/// up to you on their own two feet at 1.8 m/s, in the open, usually talking
/// while they do it — that walk is the whole warning system. What fires here is
/// only the last 3 m of it, and only once you have broken the arrangement by
/// leaving the leash or by moving away.
fn grab_reflex(
    state: Res<PlayerCustodyState>,
    player: Option<Single<(&PlayerController, &PhysicalPosition)>>,
    handle: Option<Res<BridgeHandle>>,
) {
    let (Some(player), Some(handle)) = (player, handle) else {
        return;
    };
    let Some(custody) = state.custody.as_ref() else {
        return;
    };
    let (controller, position) = player.into_inner();
    // Flying is not custody.
    if controller.flying || custody.held || custody.committed {
        return;
    }
    let Some(officer_id) = custody.officer_id.as_ref() else {
        return;
    };
    let here = position.current;
    // Where the officer is *now*, which is [`CustodyView::anchor_m`] and never
    // [`WorldMirror`]: nobody has hold yet — that is checked above — so the sim
    // fills the anchor with the escort's own position and republishes the
    // standing line with every step they take. The mirror cannot answer this at
    // all, because walking rides the hot `Movement` channel and never bumps a
    // revision, so it has the officer frozen wherever the last unrelated
    // snapshot left them: for the whole walk from a seizure to a station that is
    // wrong by far more than the radius being measured here, in both directions.
    // (`hands::hold_the_seized` avoids the same trap by ordering itself after
    // the hot channel has driven the bodies.) A drain lands the anchor in
    // `PostUpdate` and this runs in `Update`, so it is one frame old at worst,
    // which at an officer's 2.1 m/s is three centimetres.
    let officer_at = custody.anchor_m;
    let separation = here.distance(officer_at);
    if separation > CUSTODY_REACH_M as f32 {
        return;
    }
    // Two ways to have broken the arrangement, and only these two. Walking
    // beside your escort is neither, and must stay pleasant.
    //
    // `closing` cannot be recomputed here from `separation`: arm's reach is
    // 3 m and the leash is 8, so by the time the officer is close enough to
    // grab you, you are trivially "inside the leash" again. It is the sim that
    // remembers you left it.
    let outward = (here - officer_at).normalize_or_zero();
    let fleeing = controller.velocity().dot(outward) > FLEEING_SPEED_MPS;
    if !custody.closing && !fleeing {
        return;
    }
    let _ = handle.try_send(BridgeCommand::PlayerGrabbed {
        holder_id: officer_id.clone(),
    });
}

/// Pull, don't mash. The meter is the *player's* answer to `struggle`, and the
/// only thing that lives here is the clock: **how long** a pull must last comes
/// from the sim as [`CustodyView::strain_seconds`]
/// ([`cathedral_sim::custody::strain_seconds`]), so the holders' grip by
/// occupation, the second pair of hands and the player's own drunkenness and
/// weariness are one implementation shared with the cast's roll rather than two
/// that can drift apart.
fn strain_meter(
    time: Res<Time>,
    mut state: ResMut<PlayerCustodyState>,
    input: Option<Res<ControllerInput>>,
    player: Option<Single<(&PlayerController, &PhysicalPosition)>>,
    handle: Option<Res<BridgeHandle>>,
) {
    let (Some(player), Some(handle), Some(input)) = (player, handle, input) else {
        return;
    };
    let (controller, position) = player.into_inner();
    let Some(custody) = state.custody.as_ref().filter(|custody| custody.held) else {
        if state.strain != 0.0 {
            state.strain = 0.0;
            state.struggling_reported = false;
        }
        return;
    };
    let fill_seconds = custody.strain_seconds.max(0.1);
    let anchor = custody.anchor_m;
    let dt = time.delta_secs();

    // Pulling is holding a direction away from the grip, in the world frame the
    // player is actually moving in — not merely pressing a key. Leaning into a
    // wall is not a struggle, and the velocity is the only thing that knows it.
    let outward = (position.current - anchor).normalize_or_zero();
    let pulling = !controller.flying
        && input.movement != Vec2::ZERO
        && controller.velocity().dot(outward) > PULLING_SPEED_MPS;

    state.strain = if pulling {
        (state.strain + dt / fill_seconds).min(1.0)
    } else {
        (state.strain - dt * STRAIN_DRAIN_FACTOR / fill_seconds).max(0.0)
    };
    if pulling && !state.struggling_reported {
        state.struggling_reported = true;
        let _ = handle.try_send(BridgeCommand::PlayerStruggling);
    }

    if state.strain >= 1.0 {
        state.strain = 0.0;
        // `struggling_reported` deliberately stays set. The sim's answer arrives
        // a frame or more later, so this projection still reads "held" while the
        // player is very likely still holding the key — and clearing the latch
        // here would report a *fresh* struggle on the next tick, a third percept
        // against M4d's "exactly two however long it lasts". The one place it is
        // cleared is `apply_law_standing`, when the hold actually ends.
        //
        // The sim clears the custody and raises the unanswerable escape notice;
        // the next `LawStanding` is what actually frees this projection.
        let _ = handle.try_send(BridgeCommand::PlayerBrokeFree);
    }
}

/// The **one** writer of the standing line, and the reason it is a system at
/// all: its two halves move on different clocks. The sim's words land in
/// [`SmartActorSet::DrainBridge`] and land often — `anchor_m` follows the grip,
/// so an escorting officer republishes them with every step — while the meter
/// that [`strain_meter`] fills moves every frame. Composed anywhere but here,
/// after that drain and before [`SmartActorSet::Present`] draws it, one of the
/// two writes wins the frame and the other is discarded unseen; the bar is the
/// half that used to lose, in precisely the situation it exists for.
fn law_standing_hud(state: Res<PlayerCustodyState>, mut hud: ResMut<SmartActorHudState>) {
    if !state.is_changed() {
        return;
    }
    hud.set_law_standing(standing_text(
        &state.notices,
        state.custody.as_ref(),
        state.strain,
    ));
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use cathedral_sim::{custody::STRAIN_BASE_SECONDS, engine::PlayerNotice, notices::Rung};
    use crossbeam_channel::{Receiver, bounded};

    use crate::smart_actors::model::{
        ActorControl, ActorSnapshot, Position, WorldMirror, WorldSnapshot,
    };

    use super::*;

    /// The fixed step the meter is pumped at in these tests. The real one is the
    /// frame rate; the meter is rate-independent because it integrates `dt`.
    const TICK_SECONDS: f32 = 1.0 / 120.0;
    /// The sim's own number for a second pair of hands
    /// (`cathedral_sim::custody::grip_strength`: two holders are 7×, not 2×).
    /// The host never computes it — it only honours it — so the constant is
    /// written out here rather than derived, and the sim's tests own the 7.
    const TWO_HOLDERS_STRAIN_SECONDS: f32 = STRAIN_BASE_SECONDS as f32 * 7.0;

    fn officer_id() -> ActorId {
        ActorId("sergeant0".into())
    }

    fn actor(id: &str, control: ActorControl, at: Vec3) -> ActorSnapshot {
        ActorSnapshot {
            id: ActorId(id.into()),
            name_for_player: id.into(),
            control,
            position_m: Position::new(at.x, at.y, at.z).expect("a finite test position"),
            facing_yaw: 0.0,
            appearance: Default::default(),
            holds: vec![],
            active_gesture: None,
            statuses: Vec::new(),
            pockets: Vec::new(),
        }
    }

    /// The cold snapshot, and the reason it is here at all: neither custody
    /// system may read a walking officer out of it. It carries a deliberately
    /// separate `snapshot_officer_at` so a test can put the mirror's officer
    /// somewhere the live one is not.
    fn mirror_with(player_at: Vec3, snapshot_officer_at: Vec3) -> WorldMirror {
        let mut mirror = WorldMirror::default();
        mirror
            .replace_snapshot(WorldSnapshot {
                world_revision: 1,
                player_id: ActorId("player".into()),
                actors: vec![
                    actor("player", ActorControl::Player, player_at),
                    actor("sergeant0", ActorControl::Llm, snapshot_officer_at),
                ],
                items: vec![],
                offers: vec![],
                road_carts: vec![],
            })
            .expect("the test snapshot is well formed");
        mirror
    }

    /// Havise Ashe has taken the player in charge but nobody has laid a hand on
    /// them — the compliance path, and the one the reflex watches. The
    /// arrangement is still unbroken: `closing` is false.
    fn in_charge(anchor: Vec3) -> PlayerCustodyState {
        let mut state = PlayerCustodyState::held_at(anchor, 1, STRAIN_BASE_SECONDS as f32);
        let custody = state.custody.as_mut().expect("held_at published custody");
        custody.holder_ids.clear();
        custody.held = false;
        state
    }

    /// The same, after the player crossed the leash: the sim has latched
    /// `closing` and sent the officer to take hold. It stays latched however
    /// close the player comes back — only a hand landing clears it — which is the
    /// entire reason it is the sim's flag and not a distance the host measures.
    fn closing_on_you(anchor: Vec3) -> PlayerCustodyState {
        let mut state = in_charge(anchor);
        state
            .custody
            .as_mut()
            .expect("in charge publishes custody")
            .closing = true;
        state
    }

    /// Everything the two custody systems read, and nothing else: no renderer, no
    /// engine, no provider. The player entity carries a real `PlayerController`
    /// because both systems ask it for the world-frame velocity — pressing a key
    /// into a wall is neither fleeing nor pulling, and only the velocity knows.
    ///
    /// The mirror is inserted even though nothing here may use it: where it puts
    /// the officer is the trap, not the answer (see [`mirror_with`]). Every test
    /// but one passes the same point twice, because the two only diverge once
    /// somebody walks.
    struct CustodyApp {
        app: App,
        commands: Receiver<BridgeCommand>,
    }

    fn custody_app(
        state: PlayerCustodyState,
        controller: PlayerController,
        player_at: Vec3,
        snapshot_officer_at: Vec3,
    ) -> CustodyApp {
        let (sender, commands) = bounded(64);
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<ControllerInput>()
            .insert_resource(state)
            .insert_resource(mirror_with(player_at, snapshot_officer_at))
            .insert_resource(BridgeHandle::new(sender, PathBuf::from("/tmp")));
        app.world_mut().spawn((
            controller,
            PhysicalPosition {
                previous: player_at,
                current: player_at,
            },
        ));
        // The plugin's own order, minus the HUD line, which needs a HUD.
        app.add_systems(Update, (grab_reflex, strain_meter).chain());
        CustodyApp { app, commands }
    }

    impl CustodyApp {
        /// One frame, with the clock stopped: for the reflex, which is a pure
        /// distance-and-velocity decision taken fresh every frame.
        fn frame(&mut self) {
            self.app.update();
        }

        /// `seconds` of held (or released) movement key, at the fixed step.
        fn pump(&mut self, seconds: f32, holding_a_direction: bool) {
            self.app
                .world_mut()
                .resource_mut::<ControllerInput>()
                .movement = if holding_a_direction {
                Vec2::new(0.0, 1.0)
            } else {
                Vec2::ZERO
            };
            for _ in 0..(seconds / TICK_SECONDS).round() as usize {
                self.app
                    .world_mut()
                    .resource_mut::<Time>()
                    .advance_by(Duration::from_secs_f32(TICK_SECONDS));
                self.app.update();
            }
        }

        fn strain(&self) -> f32 {
            self.app.world().resource::<PlayerCustodyState>().strain
        }

        fn sent(&self) -> Vec<BridgeCommand> {
            self.commands.try_iter().collect()
        }
    }

    // ------------------------------------------------------------- the plugin

    /// The custody plugin is added even when smart actors are switched off in
    /// `config.ron` — `controller.rs` reads the tether every fixed step, and with
    /// no engine it is simply always empty. That is also the one build where
    /// `SmartActorSet` is never configured, so the standing line's ordering has
    /// to be a constraint the schedule can satisfy against nothing.
    #[test]
    fn the_plugin_builds_with_the_smart_actor_sets_absent() {
        let mut app = App::new();
        // The two the host already guarantees: the meter's clock, and the HUD
        // seam, which is initialised whether or not smart actors are on.
        app.init_resource::<Time>()
            .init_resource::<SmartActorHudState>()
            .add_plugins(PlayerCustodyPlugin);

        app.update();

        let state = app.world().resource::<PlayerCustodyState>();
        assert!(state.custody.is_none());
    }

    // ------------------------------------------------------------- the tether

    /// `law_and_order.md` M4c: "fly mode ignores custody". Developer flying is
    /// not a jailbreak, and pinning a debug camera would make the mechanic
    /// impossible to inspect — so the tether is absent, not merely generous.
    #[test]
    fn flying_is_not_custody_however_firm_the_grip() {
        let anchor = Vec3::new(4.0, 0.9, -2.0);
        let state = PlayerCustodyState::held_at(anchor, 1, STRAIN_BASE_SECONDS as f32);

        assert_eq!(state.tether(false), Some((anchor, CUSTODY_TETHER_M as f32)));
        assert_eq!(state.tether(true), None);
    }

    /// The compliance path is the whole of M4b: being *in charge* of an officer
    /// leaves your feet your own, and a 100–200 m walk beside a sergeant who
    /// talks the whole way is the content. Only a hand on the arm tethers.
    #[test]
    fn merely_being_in_charge_is_not_a_tether_because_nobody_has_taken_hold() {
        assert_eq!(in_charge(Vec3::new(4.0, 0.9, -2.0)).tether(false), None);
        // And neither is standing free.
        assert_eq!(PlayerCustodyState::default().tether(false), None);
    }

    // -------------------------------------------------------------- the reflex

    /// The pleasant path, and it must stay pleasant: someone walking beside their
    /// escort — stepping aside, looking at a stall — is never grabbed, however
    /// close the officer is.
    #[test]
    fn walking_beside_your_escort_is_never_grabbed() {
        let player_at = Vec3::new(2.0, 0.9, 0.0);
        let officer_at = Vec3::ZERO;
        let mut app = custody_app(
            in_charge(officer_at),
            // Ambling outward at well under the fleeing speed.
            PlayerController::moving_at(Vec3::new(1.5, 0.0, 0.0)),
            player_at,
            officer_at,
        );

        app.frame();

        assert!(app.sent().is_empty());
    }

    /// The last 3 m of the officer's walk, and the whole point of the reflex
    /// being host-side: at run speed the sim's 10 Hz view of the player is 1.2 m
    /// stale before the return trip, so this has to be decided in the frame it
    /// happens in.
    #[test]
    fn running_from_arms_reach_fires_the_grab_in_the_same_frame() {
        let player_at = Vec3::new(2.0, 0.9, 0.0);
        let officer_at = Vec3::ZERO;
        let mut app = custody_app(
            in_charge(officer_at),
            PlayerController::moving_at(Vec3::new(8.0, 0.0, 0.0)),
            player_at,
            officer_at,
        );

        app.frame();

        assert_eq!(
            app.sent(),
            vec![BridgeCommand::PlayerGrabbed {
                holder_id: officer_id()
            }]
        );
    }

    /// "A grab is a reflex at contact range, never a lasso"
    /// (`cathedral_sim::custody::CUSTODY_REACH_M`). Sprinting away from an
    /// officer five metres off is exactly the case the officer has to answer by
    /// walking, on their own two feet, in the open.
    #[test]
    fn the_grab_is_never_a_lasso_beyond_arms_reach() {
        let officer_at = Vec3::ZERO;
        let player_at = Vec3::new(CUSTODY_REACH_M as f32 + 2.0, 0.9, 0.0);
        let mut app = custody_app(
            in_charge(officer_at),
            PlayerController::moving_at(Vec3::new(12.0, 0.0, 0.0)),
            player_at,
            officer_at,
        );

        app.frame();

        assert!(app.sent().is_empty());
    }

    /// The reflex's own copy of the flying rule: a developer flying past a
    /// sergeant at arm's length is not arrested.
    #[test]
    fn flying_past_an_officer_is_never_grabbed() {
        let officer_at = Vec3::ZERO;
        let player_at = Vec3::new(2.0, 0.9, 0.0);
        let mut controller = PlayerController::moving_at(Vec3::new(8.0, 0.0, 0.0));
        controller.flying = true;
        let mut app = custody_app(in_charge(officer_at), controller, player_at, officer_at);

        app.frame();

        assert!(app.sent().is_empty());
    }

    /// The grab is the transition into being held, so a hand already on the arm
    /// must not re-fire it every frame — the sim would be handed a percept and a
    /// priority turn at the frame rate.
    #[test]
    fn a_hand_already_on_your_arm_is_not_grabbed_again() {
        let officer_at = Vec3::ZERO;
        let player_at = Vec3::new(2.0, 0.9, 0.0);
        let fleeing = Vec3::new(8.0, 0.0, 0.0);

        let mut held = custody_app(
            PlayerCustodyState::held_at(officer_at, 1, STRAIN_BASE_SECONDS as f32),
            PlayerController::moving_at(fleeing),
            player_at,
            officer_at,
        );
        held.frame();
        assert!(
            !held
                .sent()
                .iter()
                .any(|command| matches!(command, BridgeCommand::PlayerGrabbed { .. }))
        );

        let mut committed_state = in_charge(officer_at);
        committed_state
            .custody
            .as_mut()
            .expect("in charge publishes custody")
            .committed = true;
        let mut committed = custody_app(
            committed_state,
            PlayerController::moving_at(fleeing),
            player_at,
            officer_at,
        );
        committed.frame();
        assert!(committed.sent().is_empty());
    }

    /// The leash's other door (`law_and_order.md` M4: "cross the leash and the
    /// officer closes… at 3 m the grab fires"). You strayed, the sim latched
    /// `closing` and walked the officer over on their own two feet — that walk is
    /// the whole warning system — and standing still at the end of it does not
    /// save you.
    #[test]
    fn a_strayed_player_the_officer_has_closed_on_is_taken_where_they_stand() {
        // Both standing on the same ground, so the separation is the stride
        // between them and nothing else.
        let officer_at = Vec3::new(0.0, 0.9, 0.0);
        let player_at = Vec3::new(CUSTODY_REACH_M as f32 - 0.1, 0.9, 0.0);
        let mut app = custody_app(
            closing_on_you(officer_at),
            // Not fleeing at all: dead still would do, and this is a shuffle.
            PlayerController::moving_at(Vec3::new(FLEEING_SPEED_MPS - 0.1, 0.0, 0.0)),
            player_at,
            officer_at,
        );

        app.frame();

        assert_eq!(
            app.sent(),
            vec![BridgeCommand::PlayerGrabbed {
                holder_id: officer_id()
            }]
        );
    }

    /// …but the latch is not a lasso either: a closing officer still has to
    /// finish the walk. This is the half of the arrangement the *player* can
    /// still answer — come back, talk to them, pay — right up to arm's reach.
    #[test]
    fn a_closing_officer_still_has_to_get_within_arms_reach() {
        let officer_at = Vec3::ZERO;
        let player_at = Vec3::new(CUSTODY_REACH_M as f32 + 2.0, 0.9, 0.0);
        let mut app = custody_app(
            closing_on_you(officer_at),
            PlayerController::moving_at(Vec3::ZERO),
            player_at,
            officer_at,
        );

        app.frame();

        assert!(app.sent().is_empty());
    }

    /// And with the arrangement unbroken, arm's length is just arm's length. The
    /// flag has to come from the sim precisely because this case and the grabbed
    /// one are the same distance: reach is 3 m and the leash is 8, so by the time
    /// an officer is close enough to take hold, everyone is "inside the leash"
    /// again — only the sim remembers that you left it.
    #[test]
    fn an_unbroken_arrangement_at_arms_reach_is_left_alone() {
        assert!(
            CUSTODY_REACH_M < CUSTODY_LEASH_M,
            "the host cannot recompute `closing` from the separation it can see"
        );
        // Deliberately the *same* stance as the grabbed case above: only the
        // latch differs.
        let officer_at = Vec3::new(0.0, 0.9, 0.0);
        let player_at = Vec3::new(CUSTODY_REACH_M as f32 - 0.1, 0.9, 0.0);
        let mut app = custody_app(
            in_charge(officer_at),
            PlayerController::moving_at(Vec3::new(FLEEING_SPEED_MPS - 0.1, 0.0, 0.0)),
            player_at,
            officer_at,
        );

        app.frame();

        assert!(app.sent().is_empty());
    }

    /// The officer's half of the same 3 m test has to be as live as the player's,
    /// and the cold snapshot cannot supply it: walking rides the hot `Movement`
    /// channel, which never bumps a revision, so a mirror written when the
    /// seizure was declared still has the officer standing at the seizure point
    /// for the whole walk to the station. Both ways round are reachable in an
    /// ordinary arrest, and both are worse than the reflex not existing: the
    /// officer who is really there never takes hold, and the one who is really
    /// thirty metres up the street grabs a player who walked over her ghost.
    #[test]
    fn the_reflex_measures_the_officer_where_they_are_now_and_not_where_a_snapshot_left_them() {
        let player_at = Vec3::new(CUSTODY_REACH_M as f32 - 0.1, 0.9, 0.0);
        let beside_the_player = Vec3::new(0.0, 0.9, 0.0);
        let up_the_street = Vec3::new(30.0, 0.9, 0.0);

        // She strayed after them, closed the distance on her own two feet, and
        // is now at arm's reach — however old the snapshot behind her is.
        let mut closed_on = custody_app(
            closing_on_you(beside_the_player),
            // Standing still: it is the latch that fires this, not the flight.
            PlayerController::moving_at(Vec3::ZERO),
            player_at,
            up_the_street,
        );

        closed_on.frame();

        assert_eq!(
            closed_on.sent(),
            vec![BridgeCommand::PlayerGrabbed {
                holder_id: officer_id()
            }],
            "the officer standing right there never takes hold"
        );

        // And the mirror image: the player has wandered back over the point the
        // snapshot froze her at while she is still up the street, and a grab
        // from thirty metres away is exactly the lasso this reflex is not.
        let mut walked_off = custody_app(
            closing_on_you(up_the_street),
            PlayerController::moving_at(Vec3::ZERO),
            player_at,
            beside_the_player,
        );

        walked_off.frame();

        assert!(
            walked_off.sent().is_empty(),
            "an officer thirty metres off took hold of somebody"
        );
    }

    // --------------------------------------------------------- the strain meter

    /// `law_and_order.md` M4d: escape is meant to be easy enough to be a real
    /// choice — what should make you hesitate is the consequence, not the
    /// difficulty. One ordinary pair of hands is five seconds of unbroken pull.
    #[test]
    fn pulling_for_five_seconds_tears_free_of_one_ordinary_holder() {
        let anchor = Vec3::ZERO;
        let mut app = custody_app(
            PlayerCustodyState::held_at(anchor, 1, STRAIN_BASE_SECONDS as f32),
            PlayerController::moving_at(Vec3::new(6.0, 0.0, 0.0)),
            Vec3::new(1.0, 0.9, 0.0),
            Vec3::new(1.0, 0.9, 0.0),
        );

        app.pump(4.0, true);
        let four_seconds_in = app.sent();
        assert_eq!(
            four_seconds_in,
            vec![BridgeCommand::PlayerStruggling],
            "the sim is told once that the pull started, and told nothing else yet"
        );
        assert!(app.strain() > 0.75 && app.strain() < 0.85);

        app.pump(1.2, true);

        assert_eq!(app.sent(), vec![BridgeCommand::PlayerBrokeFree]);
        // The meter starts fresh; the sim's next `LawStanding` is what actually
        // frees the projection.
        assert!(app.strain() < 0.25);
    }

    /// The one thing that *may* repeat, and deliberately: if the sim declines to
    /// free the player — the lane is starved, the officer keeps hold — a second
    /// unbroken five seconds is a second successful pull, and says so. What still
    /// does not repeat is the struggle percept.
    #[test]
    fn a_hold_the_sim_never_answers_can_be_torn_free_of_twice() {
        let anchor = Vec3::ZERO;
        let mut app = custody_app(
            PlayerCustodyState::held_at(anchor, 1, STRAIN_BASE_SECONDS as f32),
            PlayerController::moving_at(Vec3::new(6.0, 0.0, 0.0)),
            Vec3::new(1.0, 0.9, 0.0),
            Vec3::new(1.0, 0.9, 0.0),
        );

        app.pump(10.2, true);

        let sent = app.sent();
        assert_eq!(
            sent,
            vec![
                BridgeCommand::PlayerStruggling,
                BridgeCommand::PlayerBrokeFree,
                BridgeCommand::PlayerBrokeFree
            ]
        );
    }

    /// `law_and_order.md` M4d: "a struggle produces exactly two percepts however
    /// long it lasts". The awkward frames are the ones *after* the break — the
    /// projection still says held until the sim's next `LawStanding` lands, and
    /// the player is very likely still holding the key — and one struggle is
    /// still all the holder is told about.
    #[test]
    fn a_pull_held_through_the_break_reports_the_struggle_only_once() {
        let anchor = Vec3::ZERO;
        let mut app = custody_app(
            PlayerCustodyState::held_at(anchor, 1, STRAIN_BASE_SECONDS as f32),
            PlayerController::moving_at(Vec3::new(6.0, 0.0, 0.0)),
            Vec3::new(1.0, 0.9, 0.0),
            Vec3::new(1.0, 0.9, 0.0),
        );

        app.pump(5.2, true);

        let sent = app.sent();
        assert_eq!(
            sent.iter()
                .filter(|command| matches!(command, BridgeCommand::PlayerStruggling))
                .count(),
            1,
            "the pull is one struggle from beginning to end: {sent:?}"
        );
        assert_eq!(
            sent,
            vec![
                BridgeCommand::PlayerStruggling,
                BridgeCommand::PlayerBrokeFree
            ]
        );
    }

    /// Pull, don't mash: the meter drains three times as fast as it fills, so a
    /// second of hesitation costs most of it.
    #[test]
    fn letting_go_for_a_second_loses_most_of_the_meter() {
        let anchor = Vec3::ZERO;
        let mut app = custody_app(
            PlayerCustodyState::held_at(anchor, 1, STRAIN_BASE_SECONDS as f32),
            PlayerController::moving_at(Vec3::new(6.0, 0.0, 0.0)),
            Vec3::new(1.0, 0.9, 0.0),
            Vec3::new(1.0, 0.9, 0.0),
        );

        app.pump(4.0, true);
        let pulled = app.strain();
        app.pump(1.0, false);

        assert!(
            app.strain() < pulled * 0.3,
            "a second of stopping lost only {pulled} → {}",
            app.strain()
        );
        assert!(
            app.sent()
                .iter()
                .all(|command| !matches!(command, BridgeCommand::PlayerBrokeFree)),
            "nobody tears free by letting go"
        );
    }

    /// Two hands are not twice one pair — being *dragged* by two people is what
    /// the word means. The number is the sim's
    /// ([`cathedral_sim::custody::grip_strength`]) so the player's meter and the
    /// cast's roll can never drift apart; the host's job is only to honour it.
    #[test]
    fn two_holders_do_not_let_go_inside_five_seconds() {
        let anchor = Vec3::ZERO;
        let mut app = custody_app(
            PlayerCustodyState::held_at(anchor, 2, TWO_HOLDERS_STRAIN_SECONDS),
            PlayerController::moving_at(Vec3::new(6.0, 0.0, 0.0)),
            Vec3::new(1.0, 0.9, 0.0),
            Vec3::new(1.0, 0.9, 0.0),
        );

        app.pump(5.0, true);

        assert!(
            app.sent()
                .iter()
                .all(|command| !matches!(command, BridgeCommand::PlayerBrokeFree))
        );
        assert!(
            app.strain() < 0.2,
            "five seconds against two holders is barely a start, but the meter read {}",
            app.strain()
        );
    }

    /// Everything the struggle's modifiers do — the second pair of hands, a
    /// bailiff's grip, the player's own drunkenness and weariness — arrives here
    /// as this one number, and a harder grip fills the meter proportionally
    /// slower. This is the host half of "drunkenness and weariness slow the fill".
    #[test]
    fn a_grip_the_sim_calls_twice_as_hard_fills_the_meter_half_as_fast() {
        let anchor = Vec3::ZERO;
        let player_at = Vec3::new(1.0, 0.9, 0.0);
        let running = Vec3::new(6.0, 0.0, 0.0);
        let mut ordinary = custody_app(
            PlayerCustodyState::held_at(anchor, 1, STRAIN_BASE_SECONDS as f32),
            PlayerController::moving_at(running),
            player_at,
            player_at,
        );
        let mut drunk = custody_app(
            PlayerCustodyState::held_at(anchor, 1, STRAIN_BASE_SECONDS as f32 * 2.0),
            PlayerController::moving_at(running),
            player_at,
            player_at,
        );

        ordinary.pump(2.0, true);
        drunk.pump(2.0, true);

        assert!((ordinary.strain() - 2.0 * drunk.strain()).abs() < 1.0e-3);
    }

    /// "Pulling is holding a direction away from the grip, in the world frame the
    /// player is actually moving in — not merely pressing a key." Held against a
    /// wall you go nowhere, so the meter does not move and the sim is never told
    /// a struggle started.
    #[test]
    fn leaning_into_a_wall_is_not_pulling() {
        let anchor = Vec3::ZERO;
        let mut app = custody_app(
            PlayerCustodyState::held_at(anchor, 1, STRAIN_BASE_SECONDS as f32),
            // The key is down, but the sweep gave back nothing.
            PlayerController::moving_at(Vec3::ZERO),
            Vec3::new(1.0, 0.9, 0.0),
            Vec3::new(1.0, 0.9, 0.0),
        );

        app.pump(3.0, true);

        assert_eq!(app.strain(), 0.0);
        assert!(app.sent().is_empty());
    }

    /// Whoever let go — the officer, or the dead-man timer nobody chose — the
    /// next pair of hands starts against a fresh meter, and a half-finished pull
    /// is not banked.
    #[test]
    fn losing_the_hold_starts_the_next_pair_of_hands_from_zero() {
        let anchor = Vec3::ZERO;
        let mut app = custody_app(
            PlayerCustodyState::held_at(anchor, 1, STRAIN_BASE_SECONDS as f32),
            PlayerController::moving_at(Vec3::new(6.0, 0.0, 0.0)),
            Vec3::new(1.0, 0.9, 0.0),
            Vec3::new(1.0, 0.9, 0.0),
        );
        app.pump(3.0, true);
        assert!(app.strain() > 0.5);

        {
            let world = app.app.world_mut();
            let mut state = world.resource_mut::<PlayerCustodyState>();
            apply_law_standing(&mut state, &[], None);
        }

        assert_eq!(app.strain(), 0.0);
        // …and a fresh grip has to earn its own `PlayerStruggling`.
        let _ = app.sent();
        {
            let world = app.app.world_mut();
            let mut state = world.resource_mut::<PlayerCustodyState>();
            let restored = PlayerCustodyState::held_at(anchor, 1, STRAIN_BASE_SECONDS as f32)
                .custody
                .expect("held_at publishes custody");
            apply_law_standing(&mut state, &[], Some(restored));
        }
        app.pump(0.5, true);
        assert_eq!(app.sent(), vec![BridgeCommand::PlayerStruggling]);
    }

    // ------------------------------------------------------- the standing line

    fn notice(id: u64, line: &str, rung: Rung, clears_when: &str) -> PlayerNotice {
        PlayerNotice {
            notice_id: id,
            line: line.into(),
            rung,
            clears_when: clears_when.into(),
        }
    }

    /// "A brand with a visible door is a story, a brand with no door is a bug."
    /// Every word against the player names what would clear it, and custody comes
    /// first because it is what the player is doing right now.
    #[test]
    fn the_standing_line_names_the_door_out_of_every_word() {
        let notices = [
            notice(
                1,
                "wanted for the theft of a loaf",
                Rung::Warranted,
                "settle it with Ilse Marle, or pay the keeper",
            ),
            notice(
                2,
                "spoken of for fouling the Wickmarket",
                Rung::Word,
                "make it good with the ward",
            ),
        ];
        let mut state = in_charge(Vec3::ZERO);
        let custody = state.custody.as_mut().expect("in charge publishes custody");
        custody.officer_name = "Havise Ashe".into();
        custody.station_name = "the Bellstand".into();

        let text = standing_text(&notices, state.custody.as_ref(), 0.0);

        assert!(text.contains("HAVISE ASHE HAS TAKEN YOU IN CHARGE — the Bellstand"));
        // The leash is explained in the only state it can still be obeyed in.
        assert!(text.contains(&format!("past {:.0} m", CUSTODY_LEASH_M)));
        for notice in &notices {
            assert!(
                text.contains(&notice.clears_when),
                "the door out of {:?} is never named: {text}",
                notice.line
            );
        }
        assert!(text.contains("WARRANT:") && text.contains("WORD:"));

        // Held, the line is the struggle instead — and the leash sentence goes,
        // because walking with them is no longer on offer. The meter joins the
        // line rather than replacing it: a pull is one more thing true about the
        // player, and the words against them do not stop being true while it
        // lasts.
        let held = PlayerCustodyState::held_at(Vec3::ZERO, 1, STRAIN_BASE_SECONDS as f32);
        let held_text = standing_text(&notices, held.custody.as_ref(), 0.4);
        assert!(held_text.starts_with("HELD BY HAVISE ASHE — pull away to struggle free"));
        assert!(held_text.contains("[####------]"), "{held_text}");
        assert!(!held_text.contains("Walk with them"));
        for notice in &notices {
            assert!(held_text.contains(&notice.clears_when));
        }
    }

    /// `law_and_order.md` M5c: **a standing HUD line must always name what would
    /// free you right now. Never a mystery box.**
    ///
    /// The committed line was the one place in the whole feature where that rule
    /// was unmet — it said who kept you and where, and stopped. It is also the
    /// state a player can sit in for six minutes, so it is the one that could
    /// least afford it. Three doors and a bell, in the order you would try them.
    #[test]
    fn the_committed_line_names_the_bell_the_fee_and_the_book() {
        let mut state = in_charge(Vec3::ZERO);
        let custody = state.custody.as_mut().expect("custody is published");
        custody.committed = true;
        custody.officer_name = "Ede Clove".into();
        custody.station_name = "The Stone House".into();
        custody.release_office = Some("Lamplight".into());
        custody.booked_as = Some("an outland stranger in a grey hood".into());
        custody.fee_sparks = 3;

        let text = standing_text(&[], state.custody.as_ref(), 0.0);

        assert!(text.contains("HELD AT THE STONE HOUSE — Ede Clove keeps you here"));
        // Booked as a description, never a name — nobody in this city knows you.
        assert!(text.contains("booked as an outland stranger in a grey hood"), "{text}");
        // The sentence in the city's own clock, and the bell rings overhead.
        assert!(text.contains("You go at Lamplight."), "{text}");
        // …and the three doors that do not need waiting for it.
        assert!(text.contains("3 sparks is the posted fee"), "{text}");
        assert!(text.contains("stand surety"), "{text}");
        // The leash sentence is for someone who may still walk with them.
        assert!(!text.contains("Walk with them"), "{text}");

        // And a hand on the arm must not cost them the door: a committed player
        // grabbed on their way to the doorway keeps the bell, the fee and the
        // surety hint at exactly the moment they most need them — the "never a
        // mystery box" rule failing in its worst case, and a state the shipped
        // code can reach through the `grab` verb — while the meter and its own
        // invitation join them.
        let custody = state.custody.as_mut().expect("custody is published");
        custody.held = true;
        let while_held = standing_text(&[], state.custody.as_ref(), 0.7);
        assert!(while_held.contains("HELD AT THE STONE HOUSE"), "{while_held}");
        assert!(while_held.contains("You go at Lamplight."), "{while_held}");
        assert!(while_held.contains("3 sparks is the posted fee"), "{while_held}");
        assert!(
            while_held.contains("Pull away to struggle free  [#######---]"),
            "{while_held}"
        );
        let custody = state.custody.as_mut().expect("custody is published");
        custody.held = false;

        // A gate arch makes no promise it cannot keep, and still names a door.
        let custody = state.custody.as_mut().expect("custody is published");
        custody.release_office = None;
        custody.booked_as = None;
        let arch = standing_text(&[], state.custody.as_ref(), 0.0);
        assert!(arch.contains("You go when the keeper says."), "{arch}");
        assert!(arch.contains("3 sparks is the posted fee"), "{arch}");
        assert!(!arch.contains("booked as"), "{arch}");
    }

    /// What the one writer actually puts on the HUD, given a standing the drain
    /// has already landed. Only the words are on trial here; the ordering that
    /// writer exists for is exercised through the whole plugin and the real
    /// schedule by
    /// `smart_actors::tests::the_strain_bar_survives_the_standing_line_a_walking_officer_republishes`.
    fn drawn_standing_line(state: &PlayerCustodyState) -> String {
        let mut app = App::new();
        app.insert_resource(state.clone())
            .init_resource::<SmartActorHudState>()
            .add_systems(Update, law_standing_hud);
        app.update();
        app.world()
            .resource::<SmartActorHudState>()
            .law_standing_text()
            .to_string()
    }

    /// The universal case: nothing stands against you, so nothing is on screen —
    /// and the line the sim resolves is what reaches the HUD, unedited.
    #[test]
    fn the_standing_line_is_empty_when_nothing_stands_against_you() {
        assert_eq!(standing_text(&[], None, 0.0), "");

        let mut state = PlayerCustodyState::default();
        let brand = [notice(
            1,
            "spoken of for fouling the Wickmarket",
            Rung::Word,
            "make it good with the ward",
        )];
        apply_law_standing(&mut state, &brand, None);
        assert_eq!(
            drawn_standing_line(&state),
            standing_text(&brand, None, 0.0)
        );
        assert!(drawn_standing_line(&state).contains("make it good with the ward"));

        // Settled: the panel goes away entirely rather than lingering on a word
        // the player has already answered.
        apply_law_standing(&mut state, &[], None);
        assert!(state.custody.is_none());
        assert_eq!(drawn_standing_line(&state), "");
    }
}
