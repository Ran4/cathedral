//! The ordinary host clock, shared by controller physics and the sim pump.
//! Bevy's Real time remains host wall time. Every other time consumer sees one
//! accepted virtual progression, including unfocused windows and open overlays.
use bevy::{
    prelude::*,
    time::{TimeSystems, time_system},
};
use cathedral_sim::timeline::{AcceptedFrame, AcceptedTime};

#[derive(Resource, Default)]
pub(crate) struct LiveTime {
    pub continuation: AcceptedTime,
    /// This clock, not TimePlugin's clamped wall projection, owns the logical
    /// origin. M3 must restore it with continuation at final adoption.
    accepted_virtual: Time<Virtual>,
    pub last_frame: Option<AcceptedFrame>,
}

pub(crate) struct LiveTimePlugin;

impl Plugin for LiveTimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LiveTime>().add_systems(
            First,
            admit_live_time.after(time_system).in_set(TimeSystems),
        );
    }
}

fn admit_live_time(
    real: Res<Time<Real>>,
    mut live: ResMut<LiveTime>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut time: ResMut<Time>,
) {
    let frame = live.continuation.admit(real.delta());
    live.accepted_virtual.advance_by(frame.accepted_delta);
    *virtual_time = live.accepted_virtual;
    *time = virtual_time.as_generic();
    live.last_frame = Some(frame);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    #[derive(Resource, Default)]
    struct PhysicsProbe {
        steps: usize,
        elapsed: Duration,
    }

    fn step(time: Res<Time>, mut probe: ResMut<PhysicsProbe>) {
        probe.steps += 1;
        probe.elapsed += time.delta();
    }

    #[test]
    fn stalled_frames_keep_physics_calendar_and_generic_time_on_one_stream() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, LiveTimePlugin))
            .insert_resource(Time::<Fixed>::from_hz(120.0))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                500,
            )))
            .init_resource::<PhysicsProbe>()
            .add_systems(FixedUpdate, step);
        app.update(); // Real's initial sample contains no duration.
        app.update();
        let live = app.world().resource::<LiveTime>();
        assert_eq!(live.continuation.elapsed, Duration::from_millis(100));
        assert_eq!(live.continuation.debt, Duration::from_millis(400));
        assert_eq!(
            app.world().resource::<Time<Real>>().delta(),
            Duration::from_millis(500)
        );
        assert!(app.world().resource::<PhysicsProbe>().steps <= 12);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            20,
        )));
        for _ in 0..5 {
            app.update();
        }
        let accepted = app.world().resource::<LiveTime>().continuation;
        assert_eq!(accepted.elapsed, Duration::from_millis(600));
        assert_eq!(accepted.debt, Duration::ZERO);
        assert_eq!(app.world().resource::<Time>().elapsed(), accepted.elapsed);
        assert_eq!(
            app.world().resource::<Time<Virtual>>().elapsed(),
            accepted.elapsed
        );
        assert_eq!(
            app.world().resource::<PhysicsProbe>().elapsed
                + app.world().resource::<Time<Fixed>>().overstep(),
            accepted.elapsed
        );
    }
}
