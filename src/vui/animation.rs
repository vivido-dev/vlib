//! Movement: easings, springs, and how a frame asks for the next one.
//!
//! Nothing here runs a timer. An animation is a function of elapsed time, and the view asks the
//! window to be drawn again while it is still moving — so a window with nothing moving sleeps,
//! and a window with something moving wakes as often as the display refreshes and no more.

use std::time::Duration;

/// The pace for a frame that is moving, when the host has not said how fast its display
/// refreshes. Sixty a second is the floor every display clears.
pub const DEFAULT_FRAME: Duration = Duration::from_micros(16_667);

/// A frame is never asked for sooner than this, whatever a display claims: a producer's own track
/// has a record ceiling, and beating against it only produces superseded scenes.
pub const FASTEST_FRAME: Duration = Duration::from_micros(8_000);

/// How a value travels from its start to its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Easing {
    #[default]
    Linear,
    /// Slow to start.
    EaseIn,
    /// Slow to stop.
    EaseOut,
    /// Slow at both ends, which is what most movement wants.
    EaseInOut,
    /// Overshoots and comes back, for something arriving.
    BackOut,
}

impl Easing {
    /// Shape a linear `0..=1` into this curve. The ends are exact: an easing never changes where
    /// something starts or where it stops, only how it gets there.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0., 1.);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t * t,
            Self::EaseOut => {
                let inverted = 1. - t;
                1. - inverted * inverted * inverted
            }
            Self::EaseInOut => {
                if t < 0.5 {
                    4. * t * t * t
                } else {
                    let inverted = -2. * t + 2.;
                    1. - inverted * inverted * inverted / 2.
                }
            }
            Self::BackOut => {
                const OVERSHOOT: f32 = 1.70158;
                let inverted = t - 1.;
                1. + (OVERSHOOT + 1.) * inverted.powi(3) + OVERSHOOT * inverted.powi(2)
            }
        }
    }
}

/// What happens when an animation reaches its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Repeat {
    /// Stop there.
    #[default]
    Once,
    /// Start again from the beginning.
    Loop,
    /// Travel back, then forward, and so on.
    PingPong,
}

/// How something moves: how long it takes, what shape it travels in, and whether it stops.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Animation {
    pub duration: Duration,
    pub easing: Easing,
    pub repeat: Repeat,
}

impl Animation {
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            easing: Easing::default(),
            repeat: Repeat::Once,
        }
    }

    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn repeating(mut self) -> Self {
        self.repeat = Repeat::Loop;
        self
    }

    pub fn ping_pong(mut self) -> Self {
        self.repeat = Repeat::PingPong;
        self
    }

    /// Begin, at the window's current elapsed time.
    pub fn start(self, now: Duration) -> Running {
        Running {
            animation: self,
            started: now,
        }
    }
}

/// An animation that is under way.
///
/// The view owns this, the way it owns everything else that outlives a frame. It holds no clock
/// of its own: where it has got to is a question answered with the window's elapsed time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Running {
    pub animation: Animation,
    started: Duration,
}

impl Running {
    /// How far through it is at `now`, before easing: `0..=1`.
    fn progress(&self, now: Duration) -> f32 {
        if self.animation.duration.is_zero() {
            return 1.;
        }
        let elapsed = now.saturating_sub(self.started).as_secs_f32();
        let cycles = elapsed / self.animation.duration.as_secs_f32();
        match self.animation.repeat {
            Repeat::Once => cycles.clamp(0., 1.),
            Repeat::Loop => cycles.fract(),
            Repeat::PingPong => {
                let position = cycles % 2.;
                if position <= 1. {
                    position
                } else {
                    2. - position
                }
            }
        }
    }

    /// Where it has got to at `now`: `0..=1`, eased.
    pub fn value(&self, now: Duration) -> f32 {
        self.animation.easing.apply(self.progress(now))
    }

    /// The value scaled between two numbers, which is what a caller usually wants.
    pub fn lerp(&self, now: Duration, from: f32, to: f32) -> f32 {
        from + (to - from) * self.value(now)
    }

    /// Whether it has stopped. A looping animation never has.
    pub fn finished(&self, now: Duration) -> bool {
        match self.animation.repeat {
            Repeat::Once => now.saturating_sub(self.started) >= self.animation.duration,
            Repeat::Loop | Repeat::PingPong => false,
        }
    }

    /// Start again from now, which is what an interrupted animation does.
    pub fn restart(&mut self, now: Duration) {
        self.started = now;
    }
}

/// A spring, described the way a physics engine does.
///
/// Worth having over a duration for anything the user drags or flicks: the motion stays
/// continuous through an interruption, where a duration would have to start over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
}

impl Default for Spring {
    fn default() -> Self {
        // Firm and barely overshooting: what a panel sliding into place wants.
        Self {
            stiffness: 180.,
            damping: 22.,
            mass: 1.,
        }
    }
}

impl Spring {
    /// Slower, with a visible overshoot.
    pub fn bouncy() -> Self {
        Self {
            stiffness: 120.,
            damping: 10.,
            mass: 1.,
        }
    }

    /// No overshoot at all, for something that must not look playful.
    pub fn stiff() -> Self {
        Self {
            stiffness: 300.,
            damping: 40.,
            mass: 1.,
        }
    }
}

/// Where a spring is, and how fast it is going.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringState {
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
}

impl SpringState {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            velocity: 0.,
            target: value,
        }
    }

    /// Aim somewhere else, keeping whatever speed it already had. That continuity is the reason
    /// to use a spring: a target that changes mid-flight does not restart the motion.
    pub fn retarget(&mut self, target: f32) {
        self.target = target;
    }

    /// Whether it has settled, near enough that another frame would show nothing new.
    pub fn at_rest(&self) -> bool {
        (self.target - self.value).abs() < 0.001 && self.velocity.abs() < 0.001
    }

    /// Advance by `dt`.
    ///
    /// Integrated in steps of at most 8ms whatever `dt` is, because a window that was not drawn
    /// for a second must not be integrated in one jump — that is how a spring explodes instead of
    /// settling. A gap longer than a quarter second is treated as a pause rather than as motion
    /// nobody saw.
    pub fn advance(&mut self, spring: Spring, dt: Duration) {
        const MAX_STEP: f32 = 0.008;
        let mut remaining = dt.as_secs_f32().min(0.25);
        let mass = spring.mass.max(0.0001);
        while remaining > 0. {
            let step = remaining.min(MAX_STEP);
            let force =
                spring.stiffness * (self.target - self.value) - spring.damping * self.velocity;
            self.velocity += force / mass * step;
            self.value += self.velocity * step;
            remaining -= step;
        }
        if self.at_rest() {
            // Settle exactly, so a value meant to be its target is its target.
            self.value = self.target;
            self.velocity = 0.;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_easing_never_moves_the_ends() {
        for easing in [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::BackOut,
        ] {
            assert!(
                (easing.apply(0.) - 0.).abs() < 0.0001,
                "{easing:?} starts at zero"
            );
            assert!(
                (easing.apply(1.) - 1.).abs() < 0.0001,
                "{easing:?} ends at one"
            );
            // Out of range is clamped rather than extrapolated into nonsense.
            assert_eq!(easing.apply(-5.), easing.apply(0.));
            assert_eq!(easing.apply(5.), easing.apply(1.));
        }
    }

    #[test]
    fn the_easings_are_the_shapes_their_names_claim() {
        // Slow to start: less than half way at the half way point.
        assert!(Easing::EaseIn.apply(0.5) < 0.5);
        // Slow to stop: more than half way.
        assert!(Easing::EaseOut.apply(0.5) > 0.5);
        // Symmetrical about the middle.
        assert!((Easing::EaseInOut.apply(0.5) - 0.5).abs() < 0.0001);
        assert!(
            (Easing::EaseInOut.apply(0.25) + Easing::EaseInOut.apply(0.75) - 1.).abs() < 0.0001
        );
        // Overshoots past its end and comes back.
        assert!(Easing::BackOut.apply(0.7) > 1.);
    }

    #[test]
    fn an_animation_runs_from_its_start_to_its_end_and_stops() {
        let running = Animation::new(Duration::from_millis(100)).start(Duration::from_secs(10));

        assert_eq!(running.value(Duration::from_secs(10)), 0.);
        assert!((running.value(Duration::from_millis(10_050)) - 0.5).abs() < 0.001);
        assert_eq!(running.value(Duration::from_millis(10_100)), 1.);
        assert!(running.finished(Duration::from_millis(10_100)));
        assert!(!running.finished(Duration::from_millis(10_099)));

        // Past the end it stays at the end, rather than running on.
        assert_eq!(running.value(Duration::from_secs(60)), 1.);

        // And it scales between whatever two numbers a caller has.
        assert!((running.lerp(Duration::from_millis(10_050), 20., 40.) - 30.).abs() < 0.01);
    }

    #[test]
    fn a_looping_animation_starts_over_and_never_finishes() {
        let running = Animation::new(Duration::from_millis(100))
            .repeating()
            .start(Duration::ZERO);
        assert!((running.value(Duration::from_millis(50)) - 0.5).abs() < 0.001);
        // A whole cycle later it is back at the beginning.
        assert!(running.value(Duration::from_millis(100)) < 0.001);
        assert!((running.value(Duration::from_millis(150)) - 0.5).abs() < 0.001);
        assert!(!running.finished(Duration::from_secs(1000)));
    }

    #[test]
    fn a_ping_pong_animation_comes_back() {
        let running = Animation::new(Duration::from_millis(100))
            .ping_pong()
            .start(Duration::ZERO);
        assert!((running.value(Duration::from_millis(50)) - 0.5).abs() < 0.001);
        assert!((running.value(Duration::from_millis(100)) - 1.).abs() < 0.001);
        // On the way back.
        assert!((running.value(Duration::from_millis(150)) - 0.5).abs() < 0.001);
        assert!(running.value(Duration::from_millis(200)) < 0.001);
    }

    #[test]
    fn an_animation_with_no_duration_is_simply_finished() {
        let running = Animation::new(Duration::ZERO).start(Duration::ZERO);
        assert_eq!(running.value(Duration::ZERO), 1.);
        assert!(running.finished(Duration::ZERO));
    }

    #[test]
    fn restarting_moves_the_beginning_to_now() {
        let mut running = Animation::new(Duration::from_millis(100)).start(Duration::ZERO);
        assert!(running.finished(Duration::from_millis(200)));
        running.restart(Duration::from_millis(200));
        assert_eq!(running.value(Duration::from_millis(200)), 0.);
        assert!(!running.finished(Duration::from_millis(250)));
    }

    #[test]
    fn a_spring_settles_on_its_target_and_stays_there() {
        let mut state = SpringState::new(0.);
        state.retarget(1.);
        assert!(!state.at_rest());

        // Two seconds of frames is plenty for any of these springs.
        for _ in 0..120 {
            state.advance(Spring::default(), Duration::from_millis(16));
        }
        assert!(state.at_rest(), "{state:?}");
        assert_eq!(state.value, 1., "it settles exactly, not nearly");
        assert_eq!(state.velocity, 0.);
    }

    #[test]
    fn a_bouncy_spring_overshoots_and_a_stiff_one_does_not() {
        let overshoot = |spring: Spring| {
            let mut state = SpringState::new(0.);
            state.retarget(1.);
            let mut highest: f32 = 0.;
            for _ in 0..200 {
                state.advance(spring, Duration::from_millis(16));
                highest = highest.max(state.value);
            }
            highest
        };
        assert!(
            overshoot(Spring::bouncy()) > 1.05,
            "a bouncy spring goes past"
        );
        assert!(overshoot(Spring::stiff()) <= 1.001, "a stiff one does not");
    }

    #[test]
    fn a_spring_that_was_not_drawn_for_a_while_resumes_rather_than_exploding() {
        let mut state = SpringState::new(0.);
        state.retarget(1.);
        // A window hidden for ten seconds. Integrated in one jump this would leave the spring
        // somewhere absurd; it is clamped to a pause instead.
        state.advance(Spring::default(), Duration::from_secs(10));
        assert!(state.value.is_finite());
        assert!(state.value <= 1.2, "it did not fly off: {state:?}");
        assert!(state.velocity.abs() < 10.);
    }

    #[test]
    fn a_spring_keeps_its_speed_through_a_change_of_target() {
        let mut state = SpringState::new(0.);
        state.retarget(1.);
        for _ in 0..6 {
            state.advance(Spring::default(), Duration::from_millis(16));
        }
        let moving = state.velocity;
        assert!(moving > 0., "it is on its way");

        // Aiming somewhere else does not stop it dead — that continuity is the point.
        state.retarget(0.5);
        assert_eq!(state.velocity, moving);
    }
}
