//! Native vector effects. Coordinates are local logical units, times are seconds and
//! effect colors are 24-bit sRGB. The caller owns the canvas transform and clip.

// Rust guideline compliant 2026-10-07
use super::{Electricity, electric_arc, pulse_ring};
use crate::vui::effects::{Surface, Wave};
use std::f64::consts::{PI, TAU};
use vivid_protocol::vector::{
    Brush, Canvas, Color, ColorSpace, Command, Corners, GradientStop, InvalidScene, Path, Point,
    Rect, Scalar, Shadow, Transform,
};

type Result<T = ()> = std::result::Result<T, InvalidScene>;
type Box4 = [f64; 4];

/// Extent of the effect and silhouettes that cast shadows or disperse light.
#[derive(Clone, Copy, Debug)]
pub struct Scene<'a> {
    pub bounds: Rect,
    pub surfaces: &'a [Surface],
}

/// A rounded rectangle in local canvas coordinates.
#[derive(Clone, Copy, Debug)]
pub struct RoundedBox {
    pub rect: [f64; 4],
    pub radius: f64,
}
impl RoundedBox {
    /// Set the logical bounds and the corner radius.
    pub fn new(rect: [f64; 4], radius: f64) -> Self {
        Self { rect, radius }
    }
}
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
fn radial([x, y]: [f64; 2], r: f64, colors: &[(f64, Color)]) -> Result<Brush> {
    Ok(Brush::Radial {
        center: Point::new(x, y)?,
        radius: Scalar::new(r)?,
        stops: stops(colors),
        color_space: ColorSpace::Srgb,
    })
}
fn wash(c: &mut Canvas, bounds: Rect, p: [f64; 2], r: f64, colors: &[(f64, Color)]) -> Result {
    c.fill(Path::rectangle(bounds)?, radial(p, r, colors)?)?;
    Ok(())
}
fn shadow(c: &mut Canvas, b: Box4, r: f64, offset: [f64; 2], blur: f64, color: Color) -> Result {
    c.shadow(Shadow {
        rect: rect(b)?,
        radii: Corners::uniform(r)?,
        offset: Point::new(offset[0], offset[1])?,
        blur: Scalar::new(blur)?,
        spread: Scalar::ZERO,
        inset: false,
        color,
    })?;
    Ok(())
}
fn shadows(
    c: &mut Canvas,
    surfaces: &[Surface],
    [x, y]: [f64; 2],
    height: f64,
    radius: Option<f64>,
) -> Result {
    for surface in surfaces {
        let (shape, r, z) = (surface.rect, surface.radius, f64::from(surface.elevation));
        if z == 0. {
            continue;
        }
        let shift = 0.48 * z / (height - z).max(8.);
        let offset = [
            (shape[0] + shape[2] * 0.5 - x) * shift,
            (shape[1] + shape[3] * 0.5 - y) * shift,
        ];
        shadow(
            c,
            shape,
            r,
            offset,
            radius.map_or(3., |r| 1. + r * 0.65 + 0.12 * offset[0].hypot(offset[1])),
            tint(0x040814, 0.70),
        )?;
    }
    Ok(())
}
/// Cast warm light and soft shadows around a point source. Paint the foreground in the callback.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
/// Errors from the artwork callback also propagate to the caller.
pub fn lamp(
    c: &mut Canvas,
    scene: Scene<'_>,
    position: [f64; 2],
    height: f64,
    radius: f64,
    intensity: f64,
    paint: impl FnOnce(&mut Canvas) -> Result,
) -> Result {
    shadows(c, scene.surfaces, position, height, Some(radius))?;
    paint(c)?;
    wash(
        c,
        scene.bounds,
        position,
        height * 2. + radius * 3.,
        &[
            (0., tint(0xffcd8a, 0.34 * intensity)),
            (0.5, tint(0xffbb78, 0.12 * intensity)),
            (1., tint(0x080a19, 0.44)),
        ],
    )?;
    wash(
        c,
        scene.bounds,
        position,
        radius * 2.5,
        &[
            (0., tint(0xfffee6, 1.)),
            (0.27, tint(0xffe7ad, 0.98)),
            (0.4, tint(0xffcb7c, 0.7)),
            (1., tint(0xffbe71, 0.)),
        ],
    )?;
    Ok(())
}
/// Illuminate a circular beam of the given radius. Ambient and level range from zero to one.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
/// Errors from the artwork callback also propagate to the caller.
pub fn flashlight(
    c: &mut Canvas,
    scene: Scene<'_>,
    position: [f64; 2],
    radius: f64,
    ambient: f64,
    level: f64,
    paint: impl FnOnce(&mut Canvas) -> Result,
) -> Result {
    shadows(c, scene.surfaces, position, 56., None)?;
    paint(c)?;
    wash(
        c,
        scene.bounds,
        position,
        radius,
        &[
            (0., tint(0xffe1aa, 0.32 * level)),
            (0.45, tint(0xffdfaa, 0.15 * level)),
            (0.82, tint(0x080c1a, (1. - ambient) * 0.82)),
            (1., tint(0x080c1a, 1. - ambient)),
        ],
    )?;
    if level < 1. {
        c.fill(
            Path::rectangle(scene.bounds)?,
            Brush::Solid(tint(0x080c1a, (1. - level) * (1. - ambient))),
        )?;
    }
    Ok(())
}
/// Paint shafts and the penumbra behind elevated surfaces. Draw foreground artwork afterwards.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
pub fn god_rays(
    c: &mut Canvas,
    scene: Scene<'_>,
    position: [f64; 2],
    strength: f64,
    softness: f64,
    level: f64,
) -> Result {
    let [x, y] = position;
    let reach = scene.bounds.width.get().max(scene.bounds.height.get());
    let level = level.clamp(0., 1.);
    wash(
        c,
        scene.bounds,
        position,
        reach * (235. / 288.),
        &[
            (0., tint(0xffebc2, strength * 0.45 * level)),
            (0.42, tint(0xf8b78c, strength * 0.18 * level)),
            (1., tint(0xdfa1b6, 0.)),
        ],
    )?;
    // Integrate projected silhouettes over a finite disk for a soft penumbra.
    let samples = 9;
    let attenuate = |opacity: f64| 1. - (1. - opacity * level).powf(1. / f64::from(samples));
    let shade = radial(
        position,
        reach * (193. / 288.),
        &[
            (0., tint(0x151524, attenuate(0.9))),
            (0.55, tint(0x151524, attenuate(0.68))),
            (1., tint(0x151524, attenuate(0.24))),
        ],
    )?;
    for i in 0..samples {
        let angle = f64::from(i) * 2.399963;
        let reach = (4. + softness * 2.6) * (f64::from(i) / 8.).sqrt();
        let (lx, ly) = (x + reach * angle.cos(), y + reach * angle.sin());
        for surface in scene.surfaces {
            let (shape, z) = (surface.rect, surface.elevation);
            if z == 0 {
                continue;
            }
            let [l, t, w, h] = shape;
            if lx >= l && lx <= l + w && ly >= t && ly <= t + h {
                continue;
            }
            let axis = (t + h / 2. - ly).atan2(l + w / 2. - lx);
            let mut corners = [[l, t], [l + w, t], [l + w, t + h], [l, t + h]];
            let delta = |q: [f64; 2]| ((q[1] - ly).atan2(q[0] - lx) - axis + PI).rem_euclid(TAU);
            corners.sort_by(|u, v| delta(*u).total_cmp(&delta(*v)));
            let (u, v) = (corners[0], corners[3]);
            let beam = Path::builder()
                .move_to(u[0], u[1])
                .line_to(v[0], v[1])
                .line_to(lx + (v[0] - lx) * 35., ly + (v[1] - ly) * 35.)
                .line_to(lx + (u[0] - lx) * 35., ly + (u[1] - ly) * 35.)
                .close()
                .build()?;
            c.fill(beam, shade.clone())?;
        }
    }
    // Scattered light loses contrast with distance from the emitter.
    wash(
        c,
        scene.bounds,
        position,
        reach * (135. / 288.),
        &[
            (0., tint(0xffdfac, 0.28 * level)),
            (0.18, tint(0xffc38a, 0.08 * level)),
            (0.54, tint(0x191b29, 0.48)),
            (1., tint(0x191b29, 0.90)),
        ],
    )?;

    Ok(())
}
/// Paint a colored halo behind a surface, then outline its edge after painting its contents.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
/// Errors from the artwork callback also propagate to the caller.
pub fn glow(
    c: &mut Canvas,
    panel: RoundedBox,
    rgb: u32,
    radius: f64,
    intensity: f64,
    paint: impl FnOnce(&mut Canvas) -> Result,
) -> Result {
    shadow(
        c,
        panel.rect,
        panel.radius,
        [0., 0.],
        radius * 2.4,
        tint(rgb, 0.86 * intensity),
    )?;
    paint(c)?;
    c.stroke(
        rounded(panel.rect, panel.radius)?,
        Brush::Solid(tint(rgb, intensity)),
        0.8,
    )?;
    Ok(())
}
/// Stroke any path as a luminous tube with a diffuse halo and a fine bright core.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
pub fn glow_path(c: &mut Canvas, tube: &Path, rgb: u32, radius: f64, intensity: f64) -> Result {
    for (width, opacity, color) in [
        (radius * 1.4, 0.06 * intensity, rgb),
        (radius * 0.65, 0.16 * intensity, rgb),
        (2.6, 0.8 * intensity, rgb),
        (1.05, intensity + 0.14, 0xebfffb),
    ] {
        if width > 0. {
            c.stroke(tube.clone(), Brush::Solid(tint(color, opacity)), width)?;
        }
    }
    Ok(())
}
/// Paint animated flame contours and drifting embers above a horizontal fuel bed.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
pub fn fire(
    c: &mut Canvas,
    bounds: Rect,
    [left, base]: [f64; 2],
    width: f64,
    flame_height: f64,
    embers: f64,
    time: f64,
) -> Result {
    wash(
        c,
        bounds,
        [left + width * 0.5, base - 3.],
        width * (114. / 220.),
        &[(0., tint(0xf55b24, 0.38)), (1., tint(0xa03728, 0.))],
    )?;
    // Nested isotherms share a moving fuel field. Warping each contour by
    // altitude bends the tongues sideways while their roots stay attached.
    let mut plume = Vec::with_capacity(129);
    for j in 0..=128 {
        let u = f64::from(j) / 128.;
        let x = left + width * u;
        let wind = (x * 0.092 - time * 1.8).sin();
        let swirl = x * 0.36 + 1.5 * wind + time * 1.9;
        let noise = 0.52
            + 0.30 * swirl.sin()
            + 0.16 * (swirl * 1.94 - time * 3.3).sin()
            + 0.10 * (x * 0.91 + time * 4.2).sin();
        let envelope = (PI * u).sin().max(0.).powf(0.24);
        plume.push((
            x,
            flame_height * (0.16 + noise.clamp(0., 1.).powf(1.4) * 0.85) * envelope,
            wind,
        ));
    }
    for band in 0..24 {
        let heat = f64::from(band) / 23.;
        let altitude = (1. - heat * 0.94).powf(1.45);
        let mut path = Path::builder().move_to(left, base);
        for &(x, height, wind) in &plume {
            let rise = height * altitude;
            let sway = rise * (0.13 * (rise * 0.15 - time * 2.1 + x * 0.09).sin() + 0.08 * wind);
            path = path.line_to(x + sway, base - rise);
        }
        let red = 174. + 81. * (heat / 0.42).min(1.);
        let green = 35. + 220. * heat.powf(0.90);
        let blue = 25. + 195. * heat.powi(3);
        let rgb = ((red as u32) << 16) | ((green as u32) << 8) | (blue as u32);
        c.fill(
            path.line_to(left + width, base + 2.)
                .line_to(left, base + 2.)
                .close()
                .build()?,
            Brush::Solid(tint(rgb, 0.10 + 0.69 * heat)),
        )?;
    }
    for i in 0..22 {
        let j = f64::from(i);
        let t = (time * (0.32 + (j * 0.76).sin() * 0.12) + j * 0.618).rem_euclid(1.);
        let px = left + width * ((5. + j * 9.8) / 220.) + (t * 5. + j).sin() * 3.;
        let py = base - 5. - t * flame_height * (0.8 + 0.6 * (j * 2.3).sin().abs());
        let r = 0.35 + 0.42 * (1. - t);
        c.fill(
            rounded([px, py, r * 1.4, r * 2.7], r * 0.7)?,
            Brush::Solid(tint(0xffd38a, embers * (1. - t))),
        )?;
    }

    Ok(())
}
fn clipped(c: &mut Canvas, path: Path, paint: impl FnOnce(&mut Canvas) -> Result) -> Result {
    c.push(Command::Save)?;
    let result = c.push(Command::Clip(path)).and_then(paint);
    let restore = c.push(Command::Restore).map(|_| ());
    result.and(restore)
}
fn magnify(
    c: &mut Canvas,
    paint: &mut impl FnMut(&mut Canvas) -> Result,
    clip: Path,
    [x, y]: [f64; 2],
    scale: f64,
) -> Result {
    clipped(c, clip, |c| {
        c.push(Command::Transform(Transform::new([
            scale,
            0.,
            0.,
            scale,
            x * (1. - scale),
            y * (1. - scale),
        ])?))?;
        paint(c)
    })
}

// Repaint just the artwork under the lens. Offset edge colors diverge toward its rim;
// a frosted surface scatters each underlying silhouette with a Gaussian footprint.
fn glass_detail(
    c: &mut Canvas,
    surfaces: &[Surface],
    shape: RoundedBox,
    center: [f64; 2],
    depth: f64,
    dispersion: f64,
    frost: f64,
) -> Result {
    let lens = shape.rect;
    let [_, _, w, h] = lens;
    if dispersion == 0. && frost == 0. {
        return Ok(());
    }
    let [x, y] = center;
    let scale = 1. + depth * 0.008;
    clipped(c, rounded(lens, shape.radius)?, |c| {
        if frost > 0. {
            c.fill(
                rounded(lens, shape.radius)?,
                Brush::Solid(tint(0xe4e6df, 0.65 * frost)),
            )?;
        }
        c.push(Command::Transform(Transform::new([
            scale,
            0.,
            0.,
            scale,
            x * (1. - scale),
            y * (1. - scale),
        ])?))?;
        for surface in surfaces {
            let (b, r, color) = (surface.rect, surface.radius, surface.color);
            let alpha = f64::from(color.0 as u8) / 255.;
            if alpha == 0.
                || b[0] > x + w * 0.5 + 5.
                || b[0] + b[2] < x - w * 0.5 - 5.
                || b[1] > y + h * 0.5 + 5.
                || b[1] + b[3] < y - h * 0.5 - 5.
            {
                continue;
            }
            if frost > 0. {
                shadow(
                    c,
                    b,
                    r,
                    [0., 0.],
                    1. + 4. * frost,
                    tint(color.0 >> 8, 0.84 * frost * alpha),
                )?;
            }
            if dispersion > 0. {
                for (direction, hue) in [(-1., 0xe95791), (1., 0x4cd9ec)] {
                    let dx = (b[0] + b[2] * 0.5 - x).clamp(-w * 0.5, w * 0.5)
                        * dispersion
                        * 0.06
                        * direction;
                    let dy = (b[1] + b[3] * 0.5 - y).clamp(-h * 0.5, h * 0.5)
                        * dispersion
                        * 0.05
                        * direction;
                    c.stroke(
                        rounded([b[0] + dx, b[1] + dy, b[2], b[3]], r)?,
                        Brush::Solid(tint(hue, dispersion * 0.72 * alpha)),
                        0.55 + dispersion * 0.5,
                    )?;
                }
            }
        }
        Ok(())
    })
}

/// Optical properties of a rounded lens; dispersion and frost range from zero to one.
#[derive(Clone, Copy, Debug)]
pub struct Glass {
    pub lens: RoundedBox,
    pub depth: f64,
    pub dispersion: f64,
    pub frost: f64,
    pub wave: Option<Wave>,
}
/// Refract the existing artwork. The callback replays the background and foreground under each clip.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
/// Errors from the artwork callback also propagate to the caller.
pub fn liquid_glass(
    c: &mut Canvas,
    scene: Scene<'_>,
    glass: Glass,
    mut paint: impl FnMut(&mut Canvas) -> Result,
) -> Result {
    let Glass {
        lens: shape,
        depth,
        dispersion,
        frost,
        wave,
    } = glass;
    let lens = shape.rect;
    rect(lens)?;
    let [l, t, w, h] = lens;
    let pos = [l + w * 0.5, t + h * 0.5];
    let radius = shape.radius.min(w * 0.5).min(h * 0.5);
    let shape = RoundedBox::new(lens, radius);
    if let Some(sh) = wave {
        let t = sh.age;
        if (0.001..1.).contains(&t) {
            let r = t * scene.bounds.width.get().max(scene.bounds.height.get()) * (225. / 288.);
            let [u, v] = sh.center;
            magnify(
                c,
                &mut paint,
                rounded([u - r, v - r, 2. * r, 2. * r], r)?,
                sh.center,
                1. + 0.035 * (1. - t),
            )?;
            pulse_ring(c, sh.center, r, 0.65, 0xffffff, 0.62 * (1. - t))?;
        }
    }

    shadow(c, lens, radius, [0., 2.8], 5., tint(0x17243d, 0.36))?;
    for (inset, factor) in [(0., 0.020), (0.85, 0.013), (2.4, 0.008)] {
        let inset = inset * w.min(h).min(42.) / 42.;
        let bb = [
            lens[0] + inset,
            lens[1] + inset,
            w - 2. * inset,
            h - 2. * inset,
        ];
        magnify(
            c,
            &mut paint,
            rounded(bb, (radius - inset).max(0.))?,
            pos,
            1. + depth * factor,
        )?;
    }
    glass_detail(c, scene.surfaces, shape, pos, depth, dispersion, frost)?;
    c.fill(
        rounded(lens, radius)?,
        vertical(
            lens[1],
            h,
            &[
                (0., tint(0xf4ffff, 0.33 + frost * 0.40)),
                (0.3, tint(0xfaffff, frost * 0.48)),
                (0.76, tint(0xedfaff, frost * 0.48)),
                (1., tint(0xa0d5ed, 0.42 + frost * 0.18)),
            ],
        )?,
    )?;
    c.stroke(
        rounded(lens, radius)?,
        vertical(
            lens[1],
            h,
            &[
                (0., tint(0xffffff, 0.96)),
                (0.45, tint(0xb5fbff, 0.26 + dispersion * 0.65)),
                (1., tint(0x8178d0, 0.46 + dispersion * 0.6)),
            ],
        )?,
        1.0,
    )?;
    Ok(())
}
/// Repeating radar rings around an emitter. At most 64 rings are emitted in a pass.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
pub fn radar(
    c: &mut Canvas,
    center: [f64; 2],
    radius: f64,
    reach: f64,
    phase: f64,
    count: u8,
    rgb: u32,
) -> Result {
    let count = count.min(64);
    for i in 0..count {
        let t = (phase + f64::from(i) / f64::from(count)).rem_euclid(1.);
        pulse_ring(c, center, radius + t * reach, 0.66, rgb, (1. - t).powi(2))?;
    }
    Ok(())
}
/// Cyclic opacity and blur profiles for a surface halo.
#[derive(Clone, Copy, Debug)]
pub enum Pulse {
    Breathe,
    Heartbeat,
}
/// Draw the animated halo behind a surface. Phase is measured in cycles.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
pub fn pulse(c: &mut Canvas, shape: RoundedBox, mode: Pulse, phase: f64, rgb: u32) -> Result {
    let (blur, opacity) = match mode {
        Pulse::Breathe => {
            let b = 0.5 - 0.5 * (phase * TAU).cos();
            (3. + 9. * b, 0.35 + 0.6 * b)
        }
        Pulse::Heartbeat => {
            let beat = phase.rem_euclid(1.);
            let h = (-((beat - 0.14) / 0.10).powi(2)).exp()
                + 0.55 * (-((beat - 0.37) / 0.08).powi(2)).exp();
            (2. + 8. * h, 0.28 + 0.72 * h)
        }
    };
    shadow(
        c,
        shape.rect,
        shape.radius,
        [0., 0.],
        blur,
        tint(rgb, opacity),
    )
}
/// Draw a ping moving along a line, with a fading trail. Progress ranges from zero to one.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
pub fn ping(c: &mut Canvas, start: [f64; 2], end: [f64; 2], progress: f64, rgb: u32) -> Result {
    if !(0.0..=1.).contains(&progress) {
        return Ok(());
    }
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let len = dx.hypot(dy);
    if len == 0. {
        return Ok(());
    }
    for j in (0..7).rev() {
        let j = f64::from(j);
        let r = 1.8 - j * 0.15;
        let t = progress - j * 1.7 / len;
        c.fill(
            rounded(
                [start[0] + dx * t - r, start[1] + dy * t - r, 2. * r, 2. * r],
                r,
            )?,
            Brush::Solid(tint(rgb, 1. - j / 7.)),
        )?;
    }
    Ok(())
}
/// An expanding click ripple, optionally clipped to a shape. Progress ranges from zero to one.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
pub fn ripple(
    c: &mut Canvas,
    center: [f64; 2],
    reach: f64,
    progress: f64,
    rgb: u32,
    clip: Option<RoundedBox>,
) -> Result {
    if !(0.0..1.).contains(&progress) {
        return Ok(());
    }
    let draw = |c: &mut Canvas| {
        pulse_ring(
            c,
            center,
            0.1 + reach * progress,
            1.15,
            rgb,
            (1. - progress).powi(2),
        )
    };
    match clip {
        Some(b) => clipped(c, rounded(b.rect, b.radius)?, draw),
        None => draw(c),
    }
}

/// Trace current around the closed perimeter of a rounded box.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
pub fn electric_border(c: &mut Canvas, shape: RoundedBox, arc: Electricity) -> Result {
    let [x, y, w, h] = shape.rect;
    let r = shape.radius.min(w * 0.5).min(h * 0.5);
    let corners = [
        [x + r, y],
        [x + w - r, y],
        [x + w, y + r],
        [x + w, y + h - r],
        [x + w - r, y + h],
        [x + r, y + h],
        [x, y + h - r],
        [x, y + r],
    ];
    for (i, &a) in corners.iter().enumerate() {
        electric_arc(
            c,
            a,
            corners[(i + 1) % 8],
            Electricity {
                amplitude: arc.amplitude * 0.45,
                glow: arc.glow * 0.72,
                seed: arc.seed.wrapping_add(i as u32),
                forks: false,
                ..arc
            },
        )?;
    }
    Ok(())
}
/// Surround a charged control with a halo and a discharge along its upper edge.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
/// Errors from the artwork callback also propagate to the caller.
pub fn charge(
    c: &mut Canvas,
    shape: RoundedBox,
    level: f64,
    arc: Electricity,
    paint: impl FnOnce(&mut Canvas) -> Result,
) -> Result {
    let [x, y, w, h] = shape.rect;
    let r = shape.radius.min(w * 0.5).min(h * 0.5);
    if level > 0.001 {
        shadow(c, shape.rect, r, [0., 0.], 8., tint(0x5fceff, level * 0.9))?;
    }
    paint(c)?;
    if level > 0.001 {
        c.stroke(
            rounded(shape.rect, r)?,
            Brush::Solid(tint(0xc2f5ff, level)),
            0.7,
        )?;
        electric_arc(
            c,
            [x + r, y],
            [x + w - r, y],
            Electricity {
                amplitude: arc.amplitude * 0.65,
                glow: arc.glow * level,
                ..arc
            },
        )?;
    }
    Ok(())
}
/// A single fading discharge between caller-selected endpoints.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
pub fn zap(
    c: &mut Canvas,
    start: [f64; 2],
    end: [f64; 2],
    progress: f64,
    arc: Electricity,
) -> Result {
    if (0.0..1.).contains(&progress) {
        electric_arc(
            c,
            start,
            end,
            Electricity {
                amplitude: arc.amplitude * 1.6,
                glow: arc.glow * (1. - progress),
                ..arc
            },
        )?;
    }
    Ok(())
}
