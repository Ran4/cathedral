//! Perspective, vision, and sound emission (`sim.py:392-482`).

use crate::{
    character::Character, event::DomainEvent, ids::ActorId, math::Vec3, sounds::Sound, world::World,
};

/// How `observer` refers to `subject`, from seeded knowledge. The single place
/// stranger-rendering happens; `knows` is one-directional and the sim never
/// mutates it (names are learned via explicit `remember` actions instead).
pub fn identify(observer: &Character, subject: &Character) -> String {
    if observer.id() == subject.id() || observer.knows().contains(subject.id()) {
        subject.name().to_string()
    } else {
        format!("a stranger (id {})", subject.id())
    }
}

/// [`identify`] for the mutation loops, which hold ids rather than references.
/// Both characters must exist.
pub fn identify_ids(world: &World, observer_id: &ActorId, subject_id: &ActorId) -> String {
    identify(
        &world.characters[observer_id],
        &world.characters[subject_id],
    )
}

/// Whether `subject` is inside `observer`'s horizontal view cone.
///
/// The cone is a compass bearing only: facing is a single yaw, so there is
/// nothing honest to test vertically. A subject directly above or below has no
/// horizontal bearing at all and fails dark (not seen), which keeps an
/// undefined angle from ever attributing a sound. There is no distance limit —
/// range gating comes solely from the recipient list.
pub fn sees(observer: &Character, subject: &Character, view_cone_degrees: f64) -> bool {
    let dx = subject.position_m().x - observer.position_m().x;
    let dz = subject.position_m().z - observer.position_m().z;
    let horizontal = dx.hypot(dz);
    if horizontal < 1e-9 {
        return false;
    }
    // Matches Bevy: yaw 0 faces -Z, and Quat::from_rotation_y(yaw) turns it.
    let facing_x = -observer.facing_yaw().sin();
    let facing_z = -observer.facing_yaw().cos();
    let cosine = (facing_x * dx + facing_z * dz) / horizontal;
    let half_angle = view_cone_degrees.to_radians() / 2.0;
    // The 1e-9 slack makes the cone boundary inclusive.
    cosine >= half_angle.cos() - 1e-9
}

/// Uppercase the first character, keep the rest. Applied to percept lines that
/// begin with an [`identify`] result ("a stranger…" → "A stranger…").
pub fn cap_first(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
    }
}

/// Emit one sound: everyone in radius hears it, witnesses see who did it.
///
/// `actor_id` is `None` for world sounds (the town bell), which are never
/// attributable regardless of the catalog row and need an explicit position.
/// Percepts land in recipients' inboxes and, like speech, later graduate into
/// their bounded `recent_history`; the emitter remembers their own act
/// immediately. Returns the transcript line.
///
/// Checks neither `sounds_enabled` nor `actor_emittable` — those are
/// `make_sound`'s (verb-level) checks; world-sound triggers call this directly.
/// Never bumps the public revision: sounds are not public state.
pub fn emit_sound(
    world: &mut World,
    actor_id: Option<&ActorId>,
    sound: &Sound,
    position_m: Option<Vec3>,
) -> String {
    let actor_id = actor_id.cloned();
    let position = position_m.unwrap_or_else(|| {
        let id = actor_id
            .as_ref()
            .expect("a world sound needs an explicit position");
        world.characters[id].position_m()
    });

    let recipients = world.characters_within(position, sound.audible_distance, actor_id.as_ref());
    let witnesses: Vec<ActorId> = match (&actor_id, &sound.seen) {
        (Some(id), Some(_)) => {
            let actor = &world.characters[id];
            recipients
                .iter()
                .filter(|recipient| {
                    sees(
                        &world.characters[*recipient],
                        actor,
                        world.view_cone_degrees,
                    )
                })
                .cloned()
                .collect()
        }
        _ => Vec::new(),
    };

    if let Some(id) = &actor_id {
        // Symmetric with speech: the emitter is excluded from recipients but
        // still remembers their own act.
        let own = match &sound.seen {
            Some(seen) => seen.replace("{actor}", "You"),
            None => sound.heard.clone(),
        };
        world
            .characters
            .get_mut(id)
            .expect("the emitter is in the world")
            .remember_percept(own);
    }

    for recipient in &recipients {
        let percept = match (&actor_id, &sound.seen) {
            (Some(actor), Some(seen)) if witnesses.contains(recipient) => {
                cap_first(&seen.replace("{actor}", &identify_ids(world, recipient, actor)))
            }
            // A percept you didn't see must not leak who it was — no id.
            _ => sound.heard.clone(),
        };
        world
            .characters
            .get_mut(recipient)
            .expect("recipients come from the world")
            .notify_percept(percept);
    }

    world.emit(DomainEvent::sound(
        sound.sound_class.clone(),
        actor_id.clone(),
        sound.sound_id.clone(),
        sound.audible_distance,
        position,
        recipients,
        witnesses,
    ));

    match (&actor_id, &sound.seen) {
        // Omniscient: the transcript uses the real name, not identify().
        (Some(id), Some(seen)) => cap_first(&seen.replace("{actor}", world.characters[id].name())),
        _ => sound.heard.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::{CharacterSheet, Control};
    use std::collections::BTreeSet;

    fn character(id: &str, position: Vec3, facing_yaw: f64) -> Character {
        Character::from_sheet(CharacterSheet {
            id: ActorId::from_raw(id),
            name: id.to_uppercase(),
            control: Control::Llm,
            back_story: "test".into(),
            location_description: "test square".into(),
            appearance: Default::default(),
            voice_key: None,
            position_m: position,
            facing_yaw,
            holds: Vec::new(),
            goal: "None".into(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
            lore: None,
            presence: crate::Presence::InCity,
            presence_epoch: 0,
            economic_class: crate::EconomicClass::Resident,
        })
    }

    /// The yaw that points `observer` straight at `subject` (yaw 0 faces -Z).
    fn yaw_towards(observer: &Character, subject: &Character) -> f64 {
        let dx = subject.position_m().x - observer.position_m().x;
        let dz = subject.position_m().z - observer.position_m().z;
        (-dx).atan2(-dz)
    }

    #[test]
    fn view_cone_boundary_is_inclusive() {
        let actor = character("actor", Vec3::new(0.0, 0.0, 0.0), 0.0);
        let mut observer = character("observer", Vec3::new(0.0, 0.0, 5.0), 0.0);
        let bearing = yaw_towards(&observer, &actor);

        // Exactly on the 45-degree half-angle edge of a 90-degree cone.
        observer.state.facing_yaw = bearing + 45.0f64.to_radians();
        assert!(sees(&observer, &actor, 90.0));

        observer.state.facing_yaw = bearing + 45.5f64.to_radians();
        assert!(!sees(&observer, &actor, 90.0));
    }

    #[test]
    fn view_cone_is_horizontal_only_and_fails_dark_without_a_bearing() {
        let actor = character("actor", Vec3::new(0.0, 0.0, 0.0), 0.0);

        // A balcony observer 15 m up and 3 m out still has a bearing.
        let mut balcony = character("balcony", Vec3::new(0.0, 15.0, 3.0), 0.0);
        balcony.state.facing_yaw = yaw_towards(&balcony, &actor);
        assert!(sees(&balcony, &actor, 135.0));

        // Directly overhead: no horizontal bearing at all, so never a witness.
        let overhead = character("overhead", Vec3::new(0.0, 15.0, 0.0), 0.0);
        assert!(!sees(&overhead, &actor, 359.0));
    }

    #[test]
    fn cap_first_uppercases_only_the_first_character() {
        assert_eq!(
            cap_first("a stranger (id p0) farted."),
            "A stranger (id p0) farted."
        );
        assert_eq!(cap_first(""), "");
    }
}
