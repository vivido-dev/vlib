//! Scene artwork and interactions for the six-pass effect gallery.

// Rust guideline compliant 2026-10-07
use serde::Deserialize;
use std::{io, rc::Rc, time::Duration};
use vlib::vui::Div;
use vlib::vui::effects::{Effect, EffectScene, Light, Surface, Wave, effect_canvas};
use vlib::vui::prelude::*;

const SPACE: [f64; 2] = [288., 144.];
const RASTER: [usize; 2] = [288, 144];
const COLORS: [[u8; 3]; 4] = [
    [69, 222, 213],
    [233, 109, 164],
    [255, 169, 80],
    [133, 150, 255],
];
#[derive(Deserialize)]
struct Knob {
    label: String,
    values: Vec<f64>,
    initial: usize,
}
#[derive(Deserialize)]
struct Mask {
    origin: [usize; 2],
    scale: usize,
    rows: Vec<String>,
}
type ArtRect = ([f64; 4], f64, String, u8, bool);
#[derive(Deserialize)]
struct Page {
    name: String,
    hint: String,
    background: String,
    position: [f64; 2],
    controls: Vec<Knob>,
    surfaces: Vec<ArtRect>,
    mask: Option<Mask>,
}

fn tint(s: &str) -> io::Result<Color> {
    Ok(rgb(u32::from_str_radix(s, 16).map_err(io::Error::other)?))
}

impl Page {
    fn scene(&self) -> io::Result<EffectScene> {
        let surfaces = self
            .surfaces
            .iter()
            .map(|(rect, radius, color, elevation, emitter)| {
                Ok(Surface {
                    rect: *rect,
                    radius: *radius,
                    color: tint(color)?,
                    elevation: *elevation,
                    emitter: *emitter,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let mask = self.mask.as_ref().map(|mask| {
            let mut alpha = vec![0; RASTER[0] * RASTER[1]];
            for (i, a) in alpha.iter_mut().enumerate() {
                let x = i % RASTER[0];
                let y = i / RASTER[0];
                if x >= mask.origin[0]
                    && y >= mask.origin[1]
                    && mask
                        .rows
                        .get((y - mask.origin[1]) / mask.scale)
                        .and_then(|s| s.as_bytes().get((x - mask.origin[0]) / mask.scale))
                        == Some(&b'1')
                {
                    *a = 255;
                }
            }
            alpha
        });
        EffectScene::new(
            SPACE,
            RASTER,
            tint(&self.background)?,
            None,
            &surfaces,
            mask.as_deref(),
        )
    }
}

pub struct Lab {
    pages: Vec<Page>,
    art: Vec<Rc<EffectScene>>,
    pub selected: usize,
    indices: Vec<Vec<usize>>,
    pub position: [f64; 2],
    grab: Option<[f64; 2]>,
    tap: Option<[f64; 2]>,
    wave: Option<([f64; 2], f64)>,
    from: f64,
    target: f64,
    changed: f64,
}

impl Lab {
    pub fn new() -> io::Result<Self> {
        let pages: Vec<Page> = serde_json::from_str(include_str!("../assets/effects_lab.json"))
            .map_err(io::Error::other)?;
        let art = pages
            .iter()
            .map(|p| p.scene().map(Rc::new))
            .collect::<io::Result<_>>()?;
        let indices = pages
            .iter()
            .map(|p| p.controls.iter().map(|k| k.initial).collect())
            .collect();
        Ok(Self {
            position: pages[0].position,
            pages,
            art,
            indices,
            selected: 0,
            grab: None,
            tap: None,
            wave: None,
            from: 1.,
            target: 1.,
            changed: 0.,
        })
    }

    fn select(&mut self, index: usize) {
        self.selected = index;
        self.position = self.pages[index].position;
        self.grab = None;
        self.tap = None;
        self.wave = None;
        self.from = 1.;
        self.target = 1.;
        self.changed = 0.;
    }
    fn value(&self, i: usize) -> f64 {
        self.pages[self.selected].controls[i].values[self.indices[self.selected][i]]
    }
    fn level(&self, now: f64) -> f64 {
        let t = ((now - self.changed) / 0.35).clamp(0., 1.);
        self.from + (self.target - self.from) * (t * t * (3. - 2. * t))
    }
    fn fade(&mut self, on: bool, now: f64) {
        let target = if on { 1. } else { 0. };
        if target != self.target {
            self.from = self.level(now);
            self.target = target;
            self.changed = now;
        }
    }
    fn params(&self, now: f64) -> (Effect, bool) {
        let [x, y] = self.position;
        let a = self.value(0);
        let b = self.value(1);
        let level = self.level(now);
        let moving = self.from != self.target && now - self.changed < 0.35;
        let mut light = Light {
            position: [x, y],
            height: 32.,
            radius: 7.,
            color: [255, 206, 152],
            intensity: 1.5,
            ambient: 0.48,
        };
        match self.selected {
            0 => {
                light.height = a;
                light.radius = b;
                light.intensity = self.value(2);
                (Effect::Lamp(light), false)
            }
            1 => {
                light.position = [x + 33., y + 44.];
                light.height = 56.;
                light.intensity = 2.6 * level;
                light.ambient = b;
                (
                    Effect::Flashlight {
                        light,
                        aim: [x, y],
                        beam: a,
                    },
                    moving,
                )
            }
            2 => {
                light.height = 35.;
                light.radius = 25.;
                light.ambient = 0.33;
                light.intensity = a * level;
                (Effect::GodRays { light, softness: b }, moving)
            }
            3 => (
                Effect::Glow {
                    color: COLORS[a as usize],
                    radius: b,
                    intensity: self.value(2) * level,
                },
                moving,
            ),
            4 => (
                Effect::Fire {
                    height: a,
                    speed: b,
                    embers: self.value(2),
                    time: if b > 0. { now } else { 0. },
                    seed: 19,
                },
                b > 0.,
            ),
            _ => {
                let wave = self
                    .wave
                    .map(|(center, start)| Wave {
                        center,
                        age: now - start,
                    })
                    .filter(|w| w.age < 1.);
                (
                    Effect::LiquidGlass {
                        center: [x, y],
                        size: [80., 42.],
                        depth: a,
                        dispersion: b,
                        frost: self.value(2),
                        wave,
                    },
                    wave.is_some(),
                )
            }
        }
    }
    fn local(p: Point, cx: &Context<Self>) -> Option<[f64; 2]> {
        let b = cx.bounds_of("stage")?;
        Some([
            f64::from((p.x.0 - b.origin.x.0) / b.width().0) * SPACE[0],
            f64::from((p.y.0 - b.origin.y.0) / b.height().0) * SPACE[1],
        ])
    }
    fn press(&mut self, p: [f64; 2]) -> bool {
        self.tap = Some(p);
        let dx = p[0] - self.position[0];
        let dy = p[1] - self.position[1];
        match self.selected {
            0 if dx.hypot(dy) <= self.value(1).max(13.) => self.grab = Some([dx, dy]),
            5 if (dx.abs() - 19.).max(0.).hypot(dy) <= 21. => self.grab = Some([dx, dy]),
            _ => {}
        }
        self.grab.is_some()
    }

    fn click(&mut self, now: f64) {
        if let Some(p) = self.tap.take() {
            match self.selected {
                5 => self.wave = Some((p, now)),
                3 if (24.0..=151.).contains(&p[0]) && (36.0..=108.).contains(&p[1]) => {
                    self.fade(self.target == 0., now)
                }
                _ => {}
            }
        }
    }

    fn motion(&mut self, p: [f64; 2]) -> bool {
        if self
            .tap
            .is_some_and(|start| (p[0] - start[0]).hypot(p[1] - start[1]) > 2.)
        {
            self.tap = None;
        }
        let offset = match self.grab {
            Some(o) => o,
            None if matches!(self.selected, 1 | 2)
                && (0.0..SPACE[0]).contains(&p[0])
                && (0.0..SPACE[1]).contains(&p[1]) =>
            {
                [0., 0.]
            }
            None => return false,
        };
        self.position = [
            (p[0] - offset[0]).clamp(0., SPACE[0]),
            (p[1] - offset[1]).clamp(0., SPACE[1]),
        ];
        true
    }
}

fn button(label: String, active: bool) -> Div {
    div()
        .rounded(6.)
        .px(9.)
        .py(7.)
        .cursor(CursorShape::Pointer)
        .bg(rgb(if active { 0x31565c } else { 0x28313b }))
        .child(
            text(label)
                .size(13.)
                .color(rgb(if active { 0xc7fff1 } else { 0xd1d9df })),
        )
}
impl Render for Lab {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let now = cx.elapsed().as_secs_f64();
        let (effect, moving) = self.params(now);
        if moving {
            cx.request_frame_after(Duration::from_millis(56));
        }
        let selected = self.selected;
        let page = &self.pages[selected];
        let compact = cx.size().width.0 < 520.;
        let columns = if compact { 3 } else { 6 };
        let tabs = self.pages.chunks(columns).enumerate().map(|(row, pages)| {
            div()
                .flex()
                .gap(5.)
                .children(pages.iter().enumerate().map(|(column, p)| {
                    let i = row * columns + column;
                    button(p.name.clone(), i == selected)
                        .id(format!("scene-{i}"))
                        .on_click(cx.listener(move |s: &mut Self, _, cx| {
                            if s.grab.is_some() {
                                let _ = cx.capture_pointer(false);
                            }
                            s.select(i);
                            cx.notify();
                        }))
                }))
        });
        let knobs = page.controls.iter().enumerate().map(|(i, k)| {
            let v = self.value(i);
            let value = if selected == 3 && i == 0 {
                ["Mint", "Rose", "Amber", "Blue"][v as usize].to_owned()
            } else {
                v.to_string()
            };
            button(format!("{}  {}  ›", k.label, value), false)
                .id(format!("knob-{i}"))
                .on_click(cx.listener(move |s: &mut Self, _, cx| {
                    let index = &mut s.indices[selected][i];
                    *index = (*index + 1) % s.pages[selected].controls[i].values.len();
                    cx.notify();
                }))
        });
        let width = (cx.size().width.0 - 32.).max(1.);
        let height = (cx.size().height.0 - if compact { 191. } else { 152. })
            .max(1.)
            .min(width * 0.5);
        div()
            .id("lab")
            .flex_col()
            .p(16.)
            .gap(9.)
            .bg(rgb(0x18202a))
            .on_mouse_move(cx.listener(|s: &mut Self, e, cx| {
                if let Some(p) = e.position().and_then(|p| Self::local(p, cx))
                    && s.motion(p)
                {
                    cx.notify();
                }
            }))
            .on_mouse_up(cx.listener(|s: &mut Self, _, cx| {
                if s.grab.take().is_some() {
                    let _ = cx.capture_pointer(false);
                    cx.notify();
                }
            }))
            .children(tabs)
            .child(
                effect_canvas("stage", self.art[selected].clone(), effect)
                    .size(height * 2., height)
                    .on_mouse_down(cx.listener(|s: &mut Self, e, cx| {
                        if let Some(p) = e.position().and_then(|p| Self::local(p, cx)) {
                            if s.press(p) {
                                let _ = cx.capture_pointer(true);
                            }
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(|s: &mut Self, _, cx| {
                        s.click(cx.elapsed().as_secs_f64());
                        cx.notify();
                    }))
                    .on_hover(cx.listener(|s: &mut Self, e, cx| {
                        if matches!(s.selected, 1 | 2)
                            && let UiEvent::Hover { entered } = e
                        {
                            s.fade(*entered, cx.elapsed().as_secs_f64());
                            cx.notify();
                        }
                    })),
            )
            .child(text(page.hint.clone()).size(13.).color(rgb(0xc6cdd3)))
            .child(div().flex().gap(6.).children(knobs))
            .child(
                div()
                    .id("resize")
                    .deferred()
                    .anchor(vlib::vui::Anchor::BottomRight, Point::new(2., 2.))
                    .size(18., 18.)
                    .resize_region(12)
                    .cursor(CursorShape::ResizeDownRight)
                    .child(text("◢").size(13.).color(rgb(0x849499))),
            )
    }
}
