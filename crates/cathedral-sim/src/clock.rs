//! The world's clock (movement L0).
//!
//! Pure, like the rest of this crate: the sim reads no clock of its own, it is
//! *handed* `now: f64` (monotonic seconds since app start) on every
//! [`Engine::poll`](crate::Engine::poll). So [`WorldClock`] is not a clock at
//! all — it is a projection of `now`, plus a scale and an epoch. No clock is
//! read here, no time crate is pulled in, and every method is a deterministic
//! function of its arguments.
//!
//! The lore names the seven offices and the seven weekdays and gives the
//! offices a rough time of day (`lore/core_lore/trade_and_daily_life.md`,
//! `lore/second_sun/11_glossary_and_naming.md`); the clock hours below are the
//! ones `features/movement/01_the_clock.md` §3 settled on. This module
//! implements that document.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

/// Real seconds between the ordinal strokes a bell rings. The Watch rings one
/// stroke, the Snuffing seven, at 3 s intervals, so a player anywhere in the
/// city learns the hour by counting (`lore/second_sun/design/06` §3).
pub const BELL_STROKE_INTERVAL_SECONDS: f64 = 3.0;

/// A game day that is shorter than this collapses the arithmetic; guard the
/// divisor rather than trust the config.
pub const MIN_SECONDS_PER_DAY: f64 = 1.0;

/// The seven canonical offices that divide the day, in order. Their `as usize`
/// discriminants are their position in the day; [`Office::ordinal`] is how many
/// strokes each rings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Office {
    /// Deep night. One stroke. Rings 02:00.
    Watch,
    /// Before light; working people are already up. Two strokes. Rings 05:00.
    Kindling,
    /// Sunrise; the gates open, the west doors are unbarred. Three. Rings 07:00.
    Dayspring,
    /// Noon; dinner and the market's peak. Four strokes. Rings 12:00.
    HighWick,
    /// Mid-afternoon; work resumes. Five strokes. Rings 15:00.
    Waning,
    /// Sunset; the lamplighters walk, the taverns fill. Six strokes. Rings 18:00.
    Lamplight,
    /// Curfew; the gates shut, the streets clear. Seven strokes. Rings 21:00.
    Snuffing,
}

impl Office {
    /// Every office, in the order their bells ring through a day.
    pub const ALL: [Office; 7] = [
        Office::Watch,
        Office::Kindling,
        Office::Dayspring,
        Office::HighWick,
        Office::Waning,
        Office::Lamplight,
        Office::Snuffing,
    ];

    /// The fraction of the day `[0, 1)` at which this office's bell rings.
    /// 0.0 is midnight, 0.5 is noon.
    pub fn start_fraction(self) -> f64 {
        let hour = match self {
            Office::Watch => 2.0,
            Office::Kindling => 5.0,
            Office::Dayspring => 7.0,
            Office::HighWick => 12.0,
            Office::Waning => 15.0,
            Office::Lamplight => 18.0,
            Office::Snuffing => 21.0,
        };
        hour / 24.0
    }

    /// How many strokes this office rings — its ordinal, 1 through 7.
    pub fn ordinal(self) -> u8 {
        match self {
            Office::Watch => 1,
            Office::Kindling => 2,
            Office::Dayspring => 3,
            Office::HighWick => 4,
            Office::Waning => 5,
            Office::Lamplight => 6,
            Office::Snuffing => 7,
        }
    }

    /// The prompt/HUD label, as the lore writes it.
    pub fn label(self) -> &'static str {
        match self {
            Office::Watch => "the Watch",
            Office::Kindling => "the Kindling",
            Office::Dayspring => "Dayspring",
            Office::HighWick => "High Wick",
            Office::Waning => "the Waning",
            Office::Lamplight => "Lamplight",
            Office::Snuffing => "the Snuffing",
        }
    }

    /// A short, present-tense description of what this hour is like in the city —
    /// what an NPC would feel about the time without reading a clock. Rendered
    /// into the sheet's `you_are.the_hour` (`features/movement/01_the_clock.md`
    /// §7), so the model always knows roughly what time it is and never spends a
    /// turn to learn it.
    pub fn prompt_phrase(self) -> &'static str {
        match self {
            Office::Watch => "deep night; the streets are empty but for the watch",
            Office::Kindling => "before first light; the working city is already stirring",
            Office::Dayspring => "sunrise; the gates are open and the day's work has begun",
            Office::HighWick => "noon; the dinner hour, and the markets are at their busiest",
            Office::Waning => "mid-afternoon; the day's work goes on",
            Office::Lamplight => "sunset; the lamps are being lit and the taverns fill",
            Office::Snuffing => "curfew; the gates are shutting and the streets are clearing",
        }
    }

    /// Parse a config / CLI name (`"dayspring"`, `"high_wick"`, `"highwick"`).
    /// Case-insensitive; spaces and a leading `the ` are ignored.
    pub fn from_config_name(name: &str) -> Option<Office> {
        let key: String = name
            .trim()
            .to_ascii_lowercase()
            .replace(['_', '-', ' '], "");
        let key = key.strip_prefix("the").unwrap_or(&key);
        match key {
            "watch" => Some(Office::Watch),
            "kindling" => Some(Office::Kindling),
            "dayspring" => Some(Office::Dayspring),
            "highwick" => Some(Office::HighWick),
            "waning" => Some(Office::Waning),
            "lamplight" => Some(Office::Lamplight),
            "snuffing" => Some(Office::Snuffing),
            _ => None,
        }
    }
}

/// A fraction this close to a bell counts as having reached it. `2/24 + 3/24`
/// is not bit-identical to `5/24` in IEEE doubles, so a naive `>=` reads 05:00
/// as the Watch rather than the Kindling; the tolerance (~86 µs of a day)
/// closes that gap without ever blurring two offices three hours apart.
const BOUNDARY_EPSILON: f64 = 1e-9;

/// The office ringing at a given fraction of the day — the last one whose bell
/// has rung. Before the Watch (02:00) the night still belongs to the previous
/// day's Snuffing, which is why its span wraps midnight.
fn office_at(fraction: f64) -> Office {
    // `ALL` is in ascending `start_fraction` order; the wrap is the default.
    let mut current = Office::Snuffing;
    for office in Office::ALL {
        if fraction >= office.start_fraction() - BOUNDARY_EPSILON {
            current = office;
        }
    }
    current
}

/// The seven weekdays. `day % 7`, with day 0 a Bellday. Market day decides where
/// the crowd is: Highmarket (3rd) at the Wickmarket and Coswald's Yard,
/// Lowmarket (6th) at the Tallage and Maren's Green
/// (`lore/core_lore/trade_and_daily_life.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Weekday {
    /// The weekly holy day; trades close, the nave fills.
    Bellday,
    Second,
    /// Market day — the Wickmarket and Coswald's Yard.
    Highmarket,
    Fourth,
    Fifth,
    /// Market day — the Tallage and Maren's Green.
    Lowmarket,
    Seventh,
}

impl Weekday {
    /// The week, in order, starting on Bellday.
    pub const ALL: [Weekday; 7] = [
        Weekday::Bellday,
        Weekday::Second,
        Weekday::Highmarket,
        Weekday::Fourth,
        Weekday::Fifth,
        Weekday::Lowmarket,
        Weekday::Seventh,
    ];

    /// Which weekday absolute day `day` falls on. Day 0 is a Bellday; negative
    /// days wrap the same way positive ones do.
    pub fn of_day(day: i64) -> Weekday {
        let index = day.rem_euclid(7) as usize;
        Weekday::ALL[index]
    }

    /// The HUD/prompt label.
    pub fn label(self) -> &'static str {
        match self {
            Weekday::Bellday => "Bellday",
            Weekday::Second => "Second",
            Weekday::Highmarket => "Highmarket",
            Weekday::Fourth => "Fourth",
            Weekday::Fifth => "Fifth",
            Weekday::Lowmarket => "Lowmarket",
            Weekday::Seventh => "Seventh",
        }
    }

    /// Whether trades set out their stalls today, and where the crowd goes.
    pub fn is_market_day(self) -> bool {
        matches!(self, Weekday::Highmarket | Weekday::Lowmarket)
    }
}

/// A resolved instant. Cheap to compute, so it is computed rather than stored.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldTime {
    /// Whole days since the epoch. Day 0 is a Bellday.
    pub day: i64,
    /// `[0, 1)` through the day. 0.0 is midnight, 0.5 is noon.
    pub fraction: f64,
    /// The last office whose bell has rung.
    pub office: Office,
    /// Which of the seven weekdays.
    pub weekday: Weekday,
}

impl WorldTime {
    /// Clock hour and minute, for a HUD readout.
    pub fn hour_minute(&self) -> (u32, u32) {
        // Round to the minute; a bell rings on the hour, so keep 07:00 at 07:00
        // rather than 06:59 from a hair of floating-point drift.
        let total_minutes = (self.fraction * 24.0 * 60.0).round() as i64;
        let total_minutes = total_minutes.rem_euclid(24 * 60);
        ((total_minutes / 60) as u32, (total_minutes % 60) as u32)
    }
}

/// The world's time: a projection of the host's `now`, plus a scale and an
/// epoch. Copy and cheap; construct one from config and hand it to the engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldClock {
    /// Real seconds per game day (before `scale`). From `config.ron`.
    seconds_per_day: f64,
    /// Where the world's clock stood at `now == 0`, in fractional days. Lets a
    /// run open at Dayspring instead of at midnight.
    epoch_days: f64,
    /// Debug multiplier on top of `seconds_per_day`; the `T` key cycles it so a
    /// whole day can be watched in a minute. 1.0 in a normal game.
    scale: f64,
    /// Night's brightness floor, `[0, 1]`. 0.05 is genuinely dark.
    night_brightness: f64,
}

impl WorldClock {
    /// A clock that reads `start_office` on `start_day` at `now == 0`.
    pub fn new(seconds_per_day: f64, start_office: Office, start_day: i64, night_brightness: f64) -> Self {
        let seconds_per_day = if seconds_per_day.is_finite() {
            seconds_per_day.max(MIN_SECONDS_PER_DAY)
        } else {
            MIN_SECONDS_PER_DAY
        };
        Self {
            seconds_per_day,
            epoch_days: start_day as f64 + start_office.start_fraction(),
            scale: 1.0,
            night_brightness: night_brightness.clamp(0.0, 1.0),
        }
    }

    /// Real seconds per game day, before the debug scale.
    pub fn seconds_per_day(self) -> f64 {
        self.seconds_per_day
    }

    /// The debug time multiplier (1.0 in a normal game).
    pub fn scale(self) -> f64 {
        self.scale
    }

    /// Night's brightness floor.
    pub fn night_brightness(self) -> f64 {
        self.night_brightness
    }

    /// Fractional days since the epoch at `now`.
    fn total_days(self, now: f64) -> f64 {
        self.epoch_days + (now * self.scale) / self.seconds_per_day
    }

    /// Resolve `now` to a [`WorldTime`].
    pub fn at(self, now: f64) -> WorldTime {
        let total = self.total_days(now);
        let whole = total.floor();
        let fraction = total - whole;
        let day = whole as i64;
        WorldTime {
            day,
            fraction,
            office: office_at(fraction),
            weekday: Weekday::of_day(day),
        }
    }

    /// The single number the behaviour ladder and the sun both read: 0.0 at the
    /// dead of night (the `night_brightness` floor), 1.0 in full day.
    pub fn brightness(self, now: f64) -> f64 {
        brightness_at(self.at(now).fraction, self.night_brightness)
    }

    /// The office bells that ring in `(previous, now]`, each paired with the
    /// real `now`-time at which it rings, earliest first.
    ///
    /// Returning the crossings of a *span* rather than testing an instant is the
    /// one non-obvious bit, and it is what makes a bell impossible to miss: at a
    /// high debug scale a whole office can pass inside one frame, and a paused
    /// game can never ring one twice.
    pub fn offices_crossed(self, previous: f64, now: f64) -> Vec<(f64, Office)> {
        if now <= previous || self.scale <= 0.0 {
            return Vec::new();
        }
        let t0 = self.total_days(previous);
        let t1 = self.total_days(now);
        let mut crossings: Vec<(f64, Office)> = Vec::new();
        for office in Office::ALL {
            let start = office.start_fraction();
            // Integer days `d` for which the boundary `d + start` lies in
            // `(t0, t1]`. Begin just below `t0` and step up.
            let mut day = (t0 - start).floor() as i64;
            while (day as f64) + start <= t0 {
                day += 1;
            }
            while (day as f64) + start <= t1 {
                let boundary_days = (day as f64) + start;
                // Invert `total_days`: now = (total - epoch) * spd / scale.
                let instant = (boundary_days - self.epoch_days) * self.seconds_per_day / self.scale;
                crossings.push((instant, office));
                day += 1;
            }
        }
        crossings.sort_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        crossings
    }

    /// The same clock reading the same instant, but running at `new_scale`
    /// afterwards. The epoch is recomputed so `at(now)` is unchanged — the debug
    /// key speeds time up without ever making it jump.
    pub fn with_scale(self, now: f64, new_scale: f64) -> WorldClock {
        let new_scale = if new_scale.is_finite() && new_scale > 0.0 {
            new_scale
        } else {
            1.0
        };
        let target = self.total_days(now);
        WorldClock {
            scale: new_scale,
            epoch_days: target - (now * new_scale) / self.seconds_per_day,
            ..self
        }
    }

    /// Advance the debug scale to the next of 1× / 10× / 60×, wrapping.
    pub fn cycle_scale(self, now: f64) -> WorldClock {
        let next = if self.scale < 5.0 {
            10.0
        } else if self.scale < 30.0 {
            60.0
        } else {
            1.0
        };
        self.with_scale(now, next)
    }
}

/// Brightness as a trapezoid pegged to the offices: dark until the Kindling
/// (05:00), a dawn ramp to full by 08:00, full day, a dusk ramp from 17:00 down
/// to the night floor by the Snuffing (21:00). A single float, compared against
/// inline thresholds by every consumer — there is no time-of-day enum in the
/// behaviour code (`features/movement/01_the_clock.md` §5).
fn brightness_at(fraction: f64, night: f64) -> f64 {
    const DAWN_START: f64 = 5.0 / 24.0;
    const DAWN_END: f64 = 8.0 / 24.0;
    const DUSK_START: f64 = 17.0 / 24.0;
    const DUSK_END: f64 = 21.0 / 24.0;
    let day = 1.0;
    if !(DAWN_START..DUSK_END).contains(&fraction) {
        night
    } else if fraction < DAWN_END {
        lerp(night, day, (fraction - DAWN_START) / (DAWN_END - DAWN_START))
    } else if fraction < DUSK_START {
        day
    } else {
        lerp(day, night, (fraction - DUSK_START) / (DUSK_END - DUSK_START))
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// The real `now`-times at which an office rings its ordinal strokes: `ordinal`
/// strokes, [`BELL_STROKE_INTERVAL_SECONDS`] apart, starting at `at`.
pub fn stroke_times(office: Office, at: f64) -> impl Iterator<Item = f64> {
    (0..office.ordinal()).map(move |stroke| at + f64::from(stroke) * BELL_STROKE_INTERVAL_SECONDS)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: f64 = 86_400.0; // seconds_per_day for a real-time (1×) clock

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {a} ≈ {b}");
    }

    #[test]
    fn office_boundaries_are_inclusive_at_the_ring() {
        let f = |hour: f64| hour / 24.0;
        assert_eq!(office_at(f(0.0)), Office::Snuffing, "midnight is still curfew");
        assert_eq!(office_at(f(1.999)), Office::Snuffing);
        assert_eq!(office_at(f(2.0)), Office::Watch);
        assert_eq!(office_at(f(4.999)), Office::Watch);
        assert_eq!(office_at(f(5.0)), Office::Kindling);
        assert_eq!(office_at(f(7.0)), Office::Dayspring);
        assert_eq!(office_at(f(11.999)), Office::Dayspring);
        assert_eq!(office_at(f(12.0)), Office::HighWick);
        assert_eq!(office_at(f(15.0)), Office::Waning);
        assert_eq!(office_at(f(18.0)), Office::Lamplight);
        assert_eq!(office_at(f(20.999)), Office::Lamplight);
        assert_eq!(office_at(f(21.0)), Office::Snuffing);
        assert_eq!(office_at(f(23.5)), Office::Snuffing);
    }

    #[test]
    fn ordinals_run_one_to_seven() {
        assert_eq!(Office::Watch.ordinal(), 1);
        assert_eq!(Office::Snuffing.ordinal(), 7);
        let sum: u8 = Office::ALL.iter().map(|office| office.ordinal()).sum();
        assert_eq!(sum, 1 + 2 + 3 + 4 + 5 + 6 + 7);
    }

    #[test]
    fn a_run_opens_on_its_start_office() {
        let clock = WorldClock::new(3600.0, Office::Dayspring, 0, 0.05);
        let time = clock.at(0.0);
        assert_eq!(time.office, Office::Dayspring);
        assert_eq!(time.day, 0);
        assert_eq!(time.weekday, Weekday::Bellday);
        approx(time.fraction, 7.0 / 24.0);
        assert_eq!(time.hour_minute(), (7, 0));
    }

    #[test]
    fn a_real_time_clock_advances_one_hour_per_hour() {
        // seconds_per_day == 86400 means one game second per real second, so
        // `now` hours past the 02:00 open read straight off the office table.
        let clock = WorldClock::new(DAY, Office::Watch, 0, 0.05);
        assert_eq!(clock.at(0.0).office, Office::Watch, "02:00");
        assert_eq!(clock.at(2.0 * 3600.0).office, Office::Watch, "04:00 is still the Watch");
        assert_eq!(clock.at(3.0 * 3600.0).office, Office::Kindling, "05:00");
        assert_eq!(clock.at(10.0 * 3600.0).office, Office::HighWick, "12:00");
        assert_eq!(clock.at(16.0 * 3600.0).office, Office::Lamplight, "18:00");
        assert_eq!(clock.at(19.0 * 3600.0).office, Office::Snuffing, "21:00");
    }

    #[test]
    fn the_day_rolls_over_and_the_weekday_advances() {
        let clock = WorldClock::new(DAY, Office::Watch, 0, 0.05); // 02:00 on day 0
        // 24 h later is 02:00 on day 1.
        let next = clock.at(24.0 * 3600.0);
        assert_eq!(next.day, 1);
        assert_eq!(next.office, Office::Watch);
        assert_eq!(next.weekday, Weekday::Second);
    }

    #[test]
    fn market_days_land_on_the_third_and_sixth() {
        assert_eq!(Weekday::of_day(0), Weekday::Bellday);
        assert_eq!(Weekday::of_day(2), Weekday::Highmarket);
        assert_eq!(Weekday::of_day(5), Weekday::Lowmarket);
        assert_eq!(Weekday::of_day(7), Weekday::Bellday);
        assert_eq!(Weekday::of_day(-1), Weekday::Seventh);
        assert!(Weekday::Highmarket.is_market_day());
        assert!(!Weekday::Bellday.is_market_day());
    }

    #[test]
    fn a_full_day_crosses_seven_offices_in_order() {
        // One game day in 60 s, opening at Dayspring (07:00 on day 0). The
        // half-second offsets keep the span's ends off any bell so the result
        // does not hang on floating-point equality at a boundary.
        let clock = WorldClock::new(60.0, Office::Dayspring, 0, 0.05);
        let crossed: Vec<Office> = clock
            .offices_crossed(0.5, 60.5)
            .into_iter()
            .map(|(_, office)| office)
            .collect();
        assert_eq!(
            crossed,
            vec![
                Office::HighWick, // 12:00 day 0
                Office::Waning,   // 15:00
                Office::Lamplight,// 18:00
                Office::Snuffing, // 21:00
                Office::Watch,    // 02:00 day 1
                Office::Kindling, // 05:00
                Office::Dayspring,// 07:00 day 1
            ]
        );
    }

    #[test]
    fn no_office_is_skipped_across_a_multi_day_jump() {
        let clock = WorldClock::new(60.0, Office::Watch, 0, 0.05);
        // Three whole days in one call: 21 bells, none lost, none doubled.
        let crossed = clock.offices_crossed(0.0, 3.0 * 60.0);
        assert_eq!(crossed.len(), 21);
        // Instants are sorted and each lands inside the span.
        for pair in crossed.windows(2) {
            assert!(pair[0].0 <= pair[1].0, "crossings are time-ordered");
        }
        assert!(crossed.first().unwrap().0 > 0.0 && crossed.last().unwrap().0 <= 3.0 * 60.0);
    }

    #[test]
    fn an_empty_or_backward_span_rings_nothing() {
        let clock = WorldClock::new(60.0, Office::Dayspring, 0, 0.05);
        assert!(clock.offices_crossed(10.0, 10.0).is_empty());
        assert!(clock.offices_crossed(10.0, 9.0).is_empty());
    }

    #[test]
    fn a_crossing_instant_lands_on_the_office_it_names() {
        let clock = WorldClock::new(60.0, Office::Dayspring, 0, 0.05);
        for (instant, office) in clock.offices_crossed(0.0, 120.0) {
            // The clock at the ring instant reads exactly that office.
            assert_eq!(clock.at(instant).office, office, "at {instant}s");
        }
    }

    #[test]
    fn changing_scale_never_makes_time_jump() {
        let clock = WorldClock::new(3600.0, Office::Dayspring, 0, 0.05);
        let now = 137.0;
        let before = clock.at(now);
        let faster = clock.with_scale(now, 10.0);
        let after = faster.at(now);
        assert_eq!(before.day, after.day);
        approx(before.fraction, after.fraction);
        // ...but a second later it has moved ten times as far.
        let a = clock.at(now + 1.0).fraction - before.fraction;
        let b = faster.at(now + 1.0).fraction - after.fraction;
        approx(b, a * 10.0);
    }

    #[test]
    fn the_debug_key_cycles_one_ten_sixty() {
        let clock = WorldClock::new(3600.0, Office::Dayspring, 0, 0.05);
        let ten = clock.cycle_scale(0.0);
        approx(ten.scale(), 10.0);
        let sixty = ten.cycle_scale(0.0);
        approx(sixty.scale(), 60.0);
        let back = sixty.cycle_scale(0.0);
        approx(back.scale(), 1.0);
    }

    #[test]
    fn brightness_is_dark_at_night_and_full_at_noon() {
        // The clock reads the floor deep in the Watch and full day at noon.
        let clock = WorldClock::new(DAY, Office::Watch, 0, 0.05);
        approx(clock.brightness(0.0), 0.05); // 02:00
        approx(clock.brightness(10.0 * 3600.0), 1.0); // 12:00
        // Midnight (fraction 0) is the floor; noon is full day.
        approx(brightness_at(0.0, 0.05), 0.05);
        approx(brightness_at(0.5, 0.05), 1.0);
        // Dawn and dusk are partway between.
        let dawn = brightness_at(6.5 / 24.0, 0.05);
        assert!(dawn > 0.05 && dawn < 1.0, "dawn ramps: {dawn}");
        let dusk = brightness_at(19.0 / 24.0, 0.05);
        assert!(dusk > 0.05 && dusk < 1.0, "dusk ramps: {dusk}");
        // The floor is configurable.
        approx(brightness_at(0.0, 0.2), 0.2);
    }

    #[test]
    fn the_snuffing_rings_seven_strokes_three_seconds_apart() {
        let strokes: Vec<f64> = stroke_times(Office::Snuffing, 100.0).collect();
        assert_eq!(strokes, vec![100.0, 103.0, 106.0, 109.0, 112.0, 115.0, 118.0]);
        assert_eq!(stroke_times(Office::Watch, 5.0).count(), 1);
    }

    #[test]
    fn office_names_parse_from_config() {
        assert_eq!(Office::from_config_name("dayspring"), Some(Office::Dayspring));
        assert_eq!(Office::from_config_name("High Wick"), Some(Office::HighWick));
        assert_eq!(Office::from_config_name("high_wick"), Some(Office::HighWick));
        assert_eq!(Office::from_config_name("the Snuffing"), Some(Office::Snuffing));
        assert_eq!(Office::from_config_name("nope"), None);
    }
}
