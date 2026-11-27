//! Vector artwork and lighting passes in the gallery's 288 by 144 design space.

// Rust guideline compliant 2026-10-07
use super::{Page, Shot};
use vivid_protocol::vector::{
    Brush, Canvas, Color, ColorSpace, Command, GradientStop, InvalidScene, Path, Point, Rect,
    Scalar, Text,
};
use vlib::vui::effects::{
    Wave,
    vector::{self, Electricity, RoundedBox, Scene},
};
type Result<T = ()> = std::result::Result<T, InvalidScene>;
type Box4 = [f64; 4];
const STAGE: Box4 = [0., 0., 288., 144.];
fn tint(rgb: u32, alpha: f64) -> Color {
    Color(rgb << 8 | (alpha.clamp(0., 1.) * 255.).round() as u32)
}
fn rect([x, y, w, h]: Box4) -> Result<Rect> {
    Rect::new(x, y, w, h)
}
fn rounded(b: Box4, r: f64) -> Result<Path> {
    Path::rounded_rectangle(rect(b)?, r)
}
fn stops(colors: &[(f64, Color)]) -> Vec<GradientStop> {
    colors
        .iter()
        .map(|&(at, color)| GradientStop {
            offset: (at * 65535.).round() as u16,
            color,
        })
        .collect()
}
fn vertical(y: f64, h: f64, colors: &[(f64, Color)]) -> Result<Brush> {
    Ok(Brush::Linear {
        start: Point::new(0., y)?,
        end: Point::new(0., y + h)?,
        stops: stops(colors),
        color_space: ColorSpace::Srgb,
    })
}
fn text(c: &mut Canvas, x: f64, y: f64, size: f64, s: &str, rgb: u32, weight: u16) -> Result {
    c.push(Command::Text(Text {
        text: s.to_owned(),
        origin: Point::new(x, y)?,
        size: Scalar::new(size)?,
        family: "sans-serif".into(),
        weight,
        italic: false,
        color: tint(rgb, 1.),
        max_width: None,
    }))?;
    Ok(())
}
fn background(c: &mut Canvas, p: &Page) -> Result {
    c.fill(
        rounded(STAGE, 0.)?,
        vertical(0., 144., &[(0., p.background.0), (1., p.horizon.0)])?,
    )?;
    Ok(())
}
fn art(c: &mut Canvas, p: &Page) -> Result {
    for surface in &p.surfaces {
        let b = surface.rect;
        let path = rounded(b, surface.radius)?;
        c.fill(path.clone(), Brush::Solid(surface.color))?;
        if surface.elevation > 0 || (b[2] > 18. && b[3] > 10.) {
            c.fill(
                path.clone(),
                vertical(
                    b[1],
                    b[3],
                    &[
                        (0., tint(0xffffff, 0.07)),
                        (0.52, tint(0xffffff, 0.)),
                        (1., tint(0x0a1020, 0.12)),
                    ],
                )?,
            )?;
            c.stroke(path, Brush::Solid(tint(0xf1efff, 0.18)), 0.35)?;
        }
    }
    for (x, y, size, label, color, weight) in &p.labels {
        text(c, *x, *y, *size, label, (color.0).0 >> 8, *weight)?;
    }
    Ok(())
}
fn lettering() -> Result<Path> {
    Path::builder()
        .move_to(175., 87.)
        .cubic_to(181., 75., 184., 55., 196., 50.)
        .cubic_to(208., 46., 202., 63., 191., 66.)
        .move_to(176., 72.)
        .line_to(198., 70.)
        .move_to(195., 84.)
        .cubic_to(200., 66., 209., 49., 214., 51.)
        .cubic_to(218., 56., 210., 71., 201., 76.)
        .cubic_to(196., 87., 203., 89., 210., 77.)
        .move_to(215., 67.)
        .cubic_to(206., 90., 214., 90., 225., 69.)
        .cubic_to(215., 93., 224., 86., 230., 77.)
        .move_to(231., 67.)
        .cubic_to(238., 63., 234., 85., 244., 84.)
        .move_to(247., 65.)
        .line_to(227., 87.)
        .build()
}
pub(super) struct Stage<'a> {
    pub page: &'a Page,
    pub index: usize,
    pub pos: [f64; 2],
    pub knobs: [f64; 3],
    pub level: f64,
    pub now: f64,
    pub wave: Option<Shot>,
    pub shots: &'a [Shot],
}

impl Stage<'_> {
    pub fn draw(&self, c: &mut Canvas) -> Result {
        let page = self.page;
        let bounds = rect(STAGE)?;
        let scene = Scene {
            bounds,
            surfaces: &page.surfaces,
        };
        let paint = |c: &mut Canvas| art(c, page);
        let pos = self.pos;
        let now = self.now;
        background(c, page)?;
        match self.index {
            0 => {
                let [height, radius, intensity] = self.knobs;
                vector::lamp(c, scene, pos, height, radius, intensity, paint)
            }
            1 => {
                let [beam, ambient, _] = self.knobs;
                vector::flashlight(c, scene, pos, 20. + 89. * beam, ambient, self.level, paint)
            }
            2 => {
                let [strength, softness, _] = self.knobs;
                vector::god_rays(c, scene, pos, strength, softness, self.level)?;
                paint(c)
            }
            3 => {
                let [palette, radius, power] = self.knobs;
                let rgb = [0x45ded5, 0xe96da4, 0xffa950, 0x8596ff][palette as usize];
                let intensity = power * self.level;
                vector::glow(
                    c,
                    RoundedBox::new([24., 36., 127., 72.], 12.),
                    rgb,
                    radius,
                    intensity,
                    paint,
                )?;
                vector::glow_path(c, &lettering()?, rgb, radius, intensity)
            }
            4 => {
                let [height, _, embers] = self.knobs;
                vector::fire(c, bounds, [17., 99.], 143., height, embers, now)?;
                vector::plasma(
                    c,
                    RoundedBox::new([181., 69., 88., 25.], 12.5),
                    3.1,
                    1.,
                    now,
                )?;
                paint(c)
            }
            5 => {
                let [depth, dispersion, frost] = self.knobs;
                paint(c)?;
                let lens = RoundedBox::new([pos[0] - 40., pos[1] - 21., 80., 42.], 21.);
                vector::liquid_glass(
                    c,
                    scene,
                    vector::Glass {
                        lens,
                        depth,
                        dispersion,
                        frost,
                        wave: self.wave.map(|s| Wave {
                            center: s.at,
                            age: now - s.start,
                        }),
                    },
                    |c| {
                        background(c, page)?;
                        paint(c)
                    },
                )
            }
            6 => {
                let [reach, period, count] = self.knobs;
                vector::radar(
                    c,
                    [51., 35.],
                    9.,
                    reach,
                    now / period,
                    count as u8,
                    0xff748a,
                )?;
                vector::pulse(
                    c,
                    RoundedBox::new([133., 24., 22., 22.], 11.),
                    vector::Pulse::Breathe,
                    now / 2.8,
                    0x5ff4bc,
                )?;
                vector::pulse(
                    c,
                    RoundedBox::new([216., 28., 34., 14.], 7.),
                    vector::Pulse::Heartbeat,
                    now / 1.1,
                    0xff426e,
                )?;
                paint(c)?;
                for shot in self.shots {
                    let mut at = shot.at;
                    let mut age = now - shot.start;
                    if (at[0] - 54.).hypot(at[1] - 100.) <= 12. {
                        if age < 0.45 {
                            vector::ping(c, [66., 100.], [155., 100.], age / 0.45, 0x93c9ff)?;
                            continue;
                        }
                        at = [155., 100.];
                        age -= 0.45;
                    }
                    let inbox = ((154. ..=248.).contains(&at[0]) && (79. ..=124.).contains(&at[1]))
                        .then_some(RoundedBox::new([154., 79., 94., 45.], 5.));
                    vector::ripple(
                        c,
                        at,
                        (reach * 2. + 37.) * 0.95,
                        age / 0.95,
                        0x85beff,
                        inbox,
                    )?;
                }
                Ok(())
            }
            _ => {
                let [amplitude, speed, glow] = self.knobs;
                let arc = Electricity {
                    time: now * speed,
                    amplitude,
                    glow,
                    rgb: 0x7db5ff,
                    seed: 0,
                    forks: true,
                };
                vector::charge(
                    c,
                    RoundedBox::new([165., 90., 84., 20.], 10.),
                    self.level,
                    arc,
                    |c| {
                        paint(c)?;
                        vector::electric_arc(c, [31., 46.], [112., 46.], arc)?;
                        vector::electric_border(c, RoundedBox::new([151., 23., 103., 46.], 5.), arc)
                    },
                )?;
                for (i, s) in self.shots.iter().enumerate() {
                    let start = if s.at[0] < 71.5 {
                        [31., 46.]
                    } else {
                        [112., 46.]
                    };
                    vector::zap(
                        c,
                        start,
                        s.at,
                        (now - s.start) / 0.42,
                        Electricity {
                            seed: i as u32 + 9,
                            ..arc
                        },
                    )?;
                }
                Ok(())
            }
        }
    }
}
