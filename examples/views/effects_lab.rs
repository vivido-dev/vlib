//! Interactive vector scenes for the effects gallery.

// Rust guideline compliant 2026-10-07
use serde::{Deserialize, Deserializer};
use std::{io, rc::Rc, time::Duration};
use vivid_protocol::vector::{Command, Path, Rect, Transform};
use vlib::vui::effects::Surface;
use vlib::vui::prelude::*;
use vlib::vui::{Color, Div, canvas::canvas};

#[path = "lab_stage.rs"]
mod stage;

const SPACE: [f64; 2] = [288., 144.];
#[derive(Deserialize)]
struct Knob {
    label: String,
    values: Vec<f64>,
    initial: usize,
}
#[derive(Clone, Copy)]
struct Ink(Color);
impl<'de> Deserialize<'de> for Ink {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        u32::from_str_radix(&s, 16)
            .map(|rgb| Self(Color(rgb << 8 | 255)))
            .map_err(serde::de::Error::custom)
    }
}
#[derive(Deserialize)]
struct Page {
    name: String,
    hint: String,
    background: Ink,
    horizon: Ink,
    position: [f64; 2],
    controls: Vec<Knob>,
    #[serde(deserialize_with = "read_surfaces")]
    surfaces: Vec<Surface>,
    labels: Vec<(f64, f64, f64, String, Ink, u16)>,
}
fn read_surfaces<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Surface>, D::Error> {
    let entries = Vec::<([f64; 4], f64, Ink, u8)>::deserialize(d)?;
    Ok(entries
        .into_iter()
        .map(|(rect, radius, ink, elevation)| Surface {
            rect,
            radius,
            color: ink.0,
            elevation,
            emitter: false,
        })
        .collect())
}

#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub at: [f64; 2],
    pub start: f64,
}

pub struct Lab {
    pages: Vec<Rc<Page>>,
    pub shots: Vec<Shot>,
    pub selected: usize,
    indices: Vec<Vec<usize>>,
    pub position: [f64; 2],
    grab: Option<[f64; 2]>,
    tap: Option<[f64; 2]>,
    wave: Option<Shot>,
    from: f64,
    target: f64,
    changed: f64,
    fire_time_offset: f64,
}

impl Lab {
    pub fn new() -> io::Result<Self> {
        let pages: Vec<Page> = serde_json::from_str(include_str!("../assets/effects_lab.json"))
            .map_err(io::Error::other)?;
        let indices = pages
            .iter()
            .map(|p| p.controls.iter().map(|k| k.initial).collect())
            .collect();
        Ok(Self {
            position: pages[0].position,
            pages: pages.into_iter().map(Rc::new).collect(),
            shots: Vec::new(),
            indices,
            selected: 0,
            grab: None,
            tap: None,
            wave: None,
            from: 1.,
            target: 1.,
            changed: 0.,
            fire_time_offset: 0.,
        })
    }

    fn select(&mut self, index: usize) {
        self.selected = index;
        self.position = self.pages[index].position;
        self.grab = None;
        self.tap = None;
        self.wave = None;
        self.shots.clear();
        self.from = if index == 7 { 0. } else { 1. };
        self.target = self.from;
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
                5 => self.wave = Some(Shot { at: p, start: now }),
                6 | 7 if self.selected == 6 || !charged(p) => {
                    if self.shots.len() == 8 {
                        self.shots.remove(0);
                    }
                    self.shots.push(Shot { at: p, start: now });
                }
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

fn charged([x, y]: [f64; 2]) -> bool {
    (165.0..=249.).contains(&x) && (90.0..=110.).contains(&y)
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
        let selected = self.selected;
        let page = self.pages[selected].clone();
        let knobs = [
            self.value(0),
            self.value(1),
            if page.controls.len() > 2 {
                self.value(2)
            } else {
                0.
            },
        ];
        self.shots
            .retain(|shot| now - shot.start < if selected == 6 { 1.4 } else { 0.42 });
        let level = self.level(now);
        let wave = self.wave.filter(|sh| now - sh.start < 1.);
        let moving = (self.from != self.target && now - self.changed < 0.35)
            || (selected == 4 && knobs[1] > 0.)
            || selected >= 6
            || wave.is_some();
        if moving {
            cx.request_frame_after(Duration::from_millis(33));
        }
        let columns = if cx.size().width.0 >= 720. {
            8
        } else if cx.size().width.0 >= 354. {
            4
        } else {
            2
        };
        let tab_rows = self.pages.len().div_ceil(columns);
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
        let scene_time = if selected == 4 {
            now * knobs[1] + self.fire_time_offset
        } else {
            now
        };
        let buttons = page.controls.iter().enumerate().map(|(i, k)| {
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
                    if selected == 4 && i == 1 {
                        // Keep both thermal effects on the same frame as the speed changes.
                        s.fire_time_offset += cx.elapsed().as_secs_f64() * (v - s.value(i));
                    }
                    cx.notify();
                }))
        });
        let width = (cx.size().width.0 - 32.).max(1.);
        let height = (cx.size().height.0 - (152. + 39. * (tab_rows - 1) as f32))
            .max(1.)
            .min(width * 0.5);
        let artwork = page.clone();
        let shots = self.shots.clone();
        let pos = self.position;
        div()
            .id("lab")
            .flex_col()
            .p(16.)
            .gap(9.)
            .bg(rgb(0x18202a))
            .on_mouse_move(cx.listener(|s: &mut Self, e, cx| {
                if s.selected == 7
                    && let Some(p) = e.position().and_then(|p| Self::local(p, cx))
                {
                    s.fade(charged(p), cx.elapsed().as_secs_f64());
                    cx.notify();
                }
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
                canvas(move |drawing, bounds| {
                    drawing.layer(|c| {
                        let scale = f64::from(bounds.width().0) / SPACE[0];
                        c.push(Command::Transform(Transform::new([
                            scale,
                            0.,
                            0.,
                            scale,
                            f64::from(bounds.origin.x.0),
                            f64::from(bounds.origin.y.0),
                        ])?))?;
                        c.push(Command::Clip(Path::rectangle(Rect::new(
                            0., 0., 288., 144.,
                        )?)?))?;
                        stage::Stage {
                            page: &artwork,
                            index: selected,
                            pos,
                            knobs,
                            level,
                            now: scene_time,
                            wave,
                            shots: &shots,
                        }
                        .draw(c)
                    });
                })
                .id("stage")
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
                    if let UiEvent::Hover { entered } = e
                        && (matches!(s.selected, 1 | 2) || (s.selected == 7 && !entered))
                    {
                        s.fade(*entered, cx.elapsed().as_secs_f64());
                        cx.notify();
                    }
                })),
            )
            .child(text(page.hint.clone()).size(13.).color(rgb(0xc6cdd3)))
            .child(div().flex().gap(6.).children(buttons))
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
