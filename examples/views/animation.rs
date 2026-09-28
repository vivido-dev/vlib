//! The view behind `animation`: easings, a spring, and a window that sleeps when nothing moves.
//!
//! No timer runs here. Every moving thing is a function of how long the window has been up, and
//! `cx.animate` asks for the next frame while there is still one worth drawing — so the loop
//! wakes at the display's rate while something is moving and not at all when nothing is.

use std::time::Duration;

use vlib::vui::prelude::*;
use vlib::vui::{CursorShape, Easing, Running, div, text};

actions!(animation_example, [Restart]);

pub const TRACK: f32 = 300.;
pub const DOT: f32 = 14.;
pub const SWEEP: Duration = Duration::from_millis(1400);

/// The easings shown, in the order they are drawn.
pub const EASINGS: [(&str, Easing); 4] = [
    ("linear", Easing::Linear),
    ("ease in", Easing::EaseIn),
    ("ease out", Easing::EaseOut),
    ("ease in out", Easing::EaseInOut),
];

pub struct Animated {
    /// One animation, read by every row: the easings differ, not the clock.
    pub sweep: Running,
    /// A spring that follows wherever it was last sent.
    pub spring: SpringState,
    pub spring_config: Spring,
}

impl Animated {
    pub fn new(now: Duration) -> Self {
        Self {
            sweep: Animation::new(SWEEP).ping_pong().start(now),
            spring: SpringState::new(0.),
            spring_config: Spring::bouncy(),
        }
    }
}

impl Default for Animated {
    fn default() -> Self {
        Self::new(Duration::ZERO)
    }
}

impl Render for Animated {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        cx.bind_keys([KeyBinding::parse("space", ActionId::of::<Restart>()).unwrap()]);

        // One call reads the animation and asks for the next frame, so a moving thing cannot be
        // drawn without being scheduled.
        let progress = cx.animate(&self.sweep);
        let travel = TRACK - DOT;

        let rows = EASINGS.map(|(label, easing)| {
            // The same progress, shaped differently: the easings are the comparison.
            let eased = easing.apply(progress);
            div()
                .flex_col()
                .gap(2.)
                .child(text(label).size(10.).color(rgb(0x8080a0)))
                .child(
                    div()
                        .id(format!("track-{label}"))
                        .relative()
                        .w(TRACK)
                        .h(DOT)
                        .rounded(DOT / 2.)
                        .bg(rgb(0x1a1a24))
                        .child(
                            div()
                                .absolute()
                                .top(0.)
                                .left(eased * travel)
                                .size(DOT, DOT)
                                .rounded(DOT / 2.)
                                .bg(rgb(0x8ecbff)),
                        ),
                )
        });

        // The spring is advanced to now and asks for frames until it settles.
        let settled = cx.animate_spring(&mut self.spring, self.spring_config);

        div()
            .id("surface")
            .flex_col()
            .gap(10.)
            .p(20.)
            .size(360., 320.)
            .bg(rgb(0x101018))
            .on_action::<Restart>(cx.listener(|state: &mut Self, _event, cx| {
                let now = cx.elapsed();
                state.sweep.restart(now);
                cx.notify();
            }))
            .children(rows)
            .child(
                text("click the track to send the spring there")
                    .size(10.)
                    .color(rgb(0x8080a0)),
            )
            .child(
                div()
                    .id("spring-track")
                    .relative()
                    .w(TRACK)
                    .h(DOT + 6.)
                    .rounded((DOT + 6.) / 2.)
                    .bg(rgb(0x1a1a24))
                    .cursor(CursorShape::Pointer)
                    .on_click(cx.listener(|state: &mut Self, event, cx| {
                        if let Some(position) = event.position() {
                            let track = cx.bounds_of("spring-track");
                            let within = track.map_or(0., |box_| {
                                (position.x.get() - box_.origin.x.get())
                                    / box_.width().get().max(1.)
                            });
                            // Retargeting mid-flight keeps whatever speed it had, which is the
                            // whole reason this is a spring and not a duration.
                            state.spring.retarget(within.clamp(0., 1.));
                            cx.notify();
                        }
                    }))
                    .child(
                        div()
                            .id("spring-dot")
                            .absolute()
                            .top(3.)
                            .left(settled * travel)
                            .size(DOT, DOT)
                            .rounded(DOT / 2.)
                            .bg(rgb(0x70d0a0)),
                    ),
            )
    }
}
