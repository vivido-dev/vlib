//! Thermal emission and perimeter plasma on the native vector canvas.

// Rust guideline compliant 2026-10-07
use super::{Result, RoundedBox, clipped, radial, rounded, saved, tint, vertical, wash};
use std::f64::consts::{PI, TAU};
use vivid_protocol::vector::{Brush, Canvas, Command, Path, Rect, Transform};

fn thermal_color(heat: f64) -> u32 {
    const RAMP: [[u8; 3]; 6] = [
        [107, 30, 16],
        [198, 60, 17],
        [250, 117, 24],
        [255, 181, 56],
        [255, 226, 134],
        [255, 248, 219],
    ];
    let value = heat.clamp(0., 1.) * 5.;
    let i = (value as usize).min(4);
    let a = value - i as f64;
    let channel = |k: usize| {
        (f64::from(RAMP[i][k]) * (1. - a) + f64::from(RAMP[i + 1][k]) * a).round() as u32
    };
    channel(0) << 16 | channel(1) << 8 | channel(2)
}

fn sway(z: f64, phase: f64, time: f64) -> f64 {
    // Backtrace a rising parcel through the large drift and smaller curling eddies.
    let age = time - z * 1.15;
    z * (0.15 * (phase + 2.1 * age).sin() + 0.085 * (1.73 * phase - 3.7 * age + 5.3 * z).sin())
}

/// Draw turbulent, cooling plumes and embers above a horizontal fuel bed.
///
/// The origin is the left end of the bed. Width and height are in logical units,
/// embers is a strength from zero to one, and time is in seconds.
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
    if width <= 0. || flame_height <= 0. {
        return Ok(());
    }
    wash(
        c,
        bounds,
        [left + width / 2., base],
        width * 0.64,
        &[
            (0., tint(0xff7d29, 0.21)),
            (0.56, tint(0xe34b1d, 0.06)),
            (1., tint(0xc33e16, 0.)),
        ],
    )?;
    let unit = Path::rectangle(Rect::new(-1., -1., 2., 2.)?)?;
    let field = Rect::new(
        left - width * 0.25,
        base - flame_height * 2.,
        width * 1.5,
        flame_height * 2.,
    )?;
    clipped(c, Path::rectangle(field)?, |c| {
        // Render smoke first, then flame and the incandescent cores. Shearing each
        // radial gradient along the local velocity joins the parcels into streams.
        for (layer, (count, slices, altitude, spread, gain, opacity)) in [
            (12, 5, 1.32, 1.30, 0., 0.064),
            (25, 8, 1.02, 1.12, 0.82, 0.68),
            (32, 7, 0.78, 1.12 * 0.78, 1.02, 0.83),
        ]
        .into_iter()
        .enumerate()
        {
            for i in 0..count {
                let seed = f64::from(i) * 2.399963 + layer as f64 * 1.71;
                let u = (f64::from(i) + 0.5 + 0.30 * (seed * 3.7).sin()) / f64::from(count);
                let edge = 1.0_f64.min(u * 12.).min((1. - u) * 12.);
                let phase = u * 32. + 0.65 * seed.sin() + layer as f64 * 0.34;
                for k in (0..slices).rev() {
                    let z = f64::from(k) / f64::from(slices);
                    let age = time - z * 0.86;
                    let fuel = 0.66
                        + 0.21 * (u * 25. + 1.45 * (u * 9. + age).sin() - 2.1 * age).sin()
                        + 0.15 * (u * 53. + age * 3.4).sin()
                        + 0.08 * (u * 11. - age).cos();
                    let flicker = 0.94
                        + 0.15 * (seed + age * 3.9).sin()
                        + 0.08 * (seed * 2.7 - age * 6.7).sin();
                    let rise = flame_height * fuel * flicker * (0.35 + 0.65 * edge) * altitude;
                    let x = left + width * u + rise * sway(z, phase, time);
                    let y = base - z * rise;
                    let pulse = 0.5
                        + 0.5 * (seed + time * 4.5 - z * 10. + 0.6 * (phase + z * 6.).sin()).sin();
                    let half = width / f64::from(count)
                        * spread
                        * (1. - 0.88 * z)
                        * (0.86 + 0.20 * (phase + z * 8. - age * 4.).sin())
                        * (1. - z * 0.58 * (1. - pulse));
                    let length = rise * (0.24 - 0.06 * z);
                    let tilt = (sway(z + 0.02, phase, time) - sway(z - 0.02, phase, time)) / 0.04;
                    let power = opacity * (1. - z).powf(0.75) * (1. - z * 0.75 * (1. - pulse));
                    let rgb = if layer == 0 {
                        0x806958
                    } else {
                        thermal_color(
                            (1. - z).powf(1.35) * gain + 0.085 * (seed + z * 12. - age * 3.).sin(),
                        )
                    };
                    let brush = radial(
                        [0., 0.],
                        1.,
                        &[
                            (0., tint(rgb, power)),
                            (0.35, tint(rgb, power * 0.87)),
                            (0.70, tint(rgb, power * 0.34)),
                            (1., tint(rgb, 0.)),
                        ],
                    )?;
                    let transform = Transform::new([half, 0., length * tilt, -length, x, y])?;
                    saved(c, |c| {
                        c.push(Command::Transform(transform))?
                            .fill(unit.clone(), brush)?;
                        Ok(())
                    })?;
                }
            }
        }
        Ok(())
    })?;
    if embers <= 0. {
        return Ok(());
    }
    for i in 0..22 {
        let j = f64::from(i);
        let seed = j * 2.399963;
        let age = (time * (0.36 + 0.12 * seed.sin()) + j * 0.618034).rem_euclid(1.);
        let drift = (age * 5. + seed).sin() - seed.sin();
        let x = left + width * (j + 0.5) / 22. + flame_height * 0.09 * age * drift;
        let y = base - flame_height * age * (0.8 + 0.44 * seed.sin().abs());
        let alpha = embers
            * 1.0_f64.min(age * 12.).min((1. - age) * 4.)
            * (0.65 + 0.35 * seed.sin().powi(2));
        let r = width.min(flame_height) * (0.004 + 0.006 * (1. - age));
        wash(
            c,
            Rect::new(x - 3. * r, y - 3. * r, 6. * r, 6. * r)?,
            [x, y],
            3. * r,
            &[(0., tint(0xff9440, alpha * 0.35)), (1., tint(0xff7125, 0.))],
        )?;
        c.fill(
            Path::ellipse(Rect::new(x - r, y - 2. * r, 2. * r, 4. * r)?)?,
            vertical(
                y - 2. * r,
                4. * r,
                &[
                    (0., tint(0xffeb96, alpha)),
                    (1., tint(0xff6027, alpha * 0.15)),
                ],
            )?,
        )?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct RimPoint {
    x: f64,
    y: f64,
    nx: f64,
    ny: f64,
}

fn rim_sample(shape: RoundedBox, guide: f64, u: f64) -> RimPoint {
    let [x, y, w, h] = shape.rect;
    let r = shape.radius.max(0.).min(w / 2.).min(h / 2.);
    let radius = r + guide;
    let arc = radius * PI / 2.;
    let mut distance = u * (2. * (w + h - 4. * r) + 4. * arc);
    // Each tangent ends in a quarter circle with a continuous outward normal.
    for (side, (tx, ty)) in [(1., 0.), (0., 1.), (-1., 0.), (0., -1.)]
        .into_iter()
        .enumerate()
    {
        let length = if side % 2 == 0 { w } else { h } - 2. * r;
        let cx = if side < 2 { x + w - r } else { x + r };
        let cy = if side == 0 || side == 3 {
            y + r
        } else {
            y + h - r
        };
        if distance <= length {
            return RimPoint {
                x: cx + radius * ty - (length - distance) * tx,
                y: cy - radius * tx - (length - distance) * ty,
                nx: ty,
                ny: -tx,
            };
        }
        distance -= length;
        if distance <= arc || side == 3 {
            let (sin, cos) = (distance / radius).sin_cos();
            let nx = ty * cos + tx * sin;
            let ny = -tx * cos + ty * sin;
            return RimPoint {
                x: cx + radius * nx,
                y: cy + radius * ny,
                nx,
                ny,
            };
        }
        distance -= arc;
    }
    unreachable!("a full lap ends on the fourth arc")
}

/// Draw flowing violet and cyan filaments around a rounded rectangle.
///
/// Width is the band thickness in logical units. Intensity scales its emission and
/// time is in seconds. Draw the control over the border for a crisp inner edge.
///
/// # Errors
///
/// Returns a canvas error for unsupported geometry or display list limits.
pub fn plasma(c: &mut Canvas, shape: RoundedBox, width: f64, intensity: f64, time: f64) -> Result {
    let [x, y, w, h] = shape.rect;
    if w <= 0. || h <= 0. || width <= 0. || intensity <= 0. {
        return Ok(());
    }
    let r = shape.radius.max(0.).min(w / 2.).min(h / 2.);
    let guide = width * 0.58;
    let rim = rounded(
        [x - guide, y - guide, w + 2. * guide, h + 2. * guide],
        r + guide,
    )?;
    for (spread, opacity) in [
        (3.2, 0.035),
        (2.5, 0.055),
        (1.9, 0.08),
        (1.4, 0.11),
        (0.95, 0.14),
    ] {
        c.stroke(
            rim.clone(),
            Brush::Solid(tint(0x7563ec, opacity * intensity)),
            width * spread,
        )?;
    }
    const COUNT: usize = 128;
    let samples: [RimPoint; COUNT] =
        std::array::from_fn(|k| rim_sample(shape, guide, k as f64 / COUNT as f64));
    for j in 0..6 {
        let phase = f64::from(j) * 2.399963;
        let points: [[f64; 2]; COUNT] = std::array::from_fn(|k| {
            let p = samples[k];
            let a = TAU * k as f64 / COUNT as f64;
            let offset = width
                * (0.31 * (3. * a + phase + time * 1.5 + 0.8 * (2. * a - time).sin()).sin()
                    + 0.19
                        * (7. * a - 2.4 * time + phase * 2. + 0.8 * (3. * a + time).cos()).sin());
            [p.x + p.nx * offset, p.y + p.ny * offset]
        });
        let first = points[0];
        let last = points[COUNT - 1];
        let mut path =
            Path::builder().move_to((last[0] + first[0]) / 2., (last[1] + first[1]) / 2.);
        for (k, &p) in points.iter().enumerate() {
            let q = points[(k + 1) % COUNT];
            path = path.quad_to(p[0], p[1], (p[0] + q[0]) / 2., (p[1] + q[1]) / 2.);
        }
        let path = path.close().build()?;
        c.stroke(
            path.clone(),
            Brush::Solid(tint(0x8473ff, intensity * 0.19)),
            width * 0.41,
        )?;
        c.stroke(
            path,
            Brush::Solid(tint(
                if j % 2 == 1 { 0x9fc4ff } else { 0x9c8bff },
                intensity * 0.48,
            )),
            width * 0.12,
        )?;
        for crest in 0..2 {
            let start = (time * (0.067 + 0.006 * f64::from(j))
                + f64::from(j) * 0.381966
                + f64::from(crest) * 0.5)
                .rem_euclid(1.);
            let mut upper = [[0.; 2]; 17];
            let mut lower = upper;
            for step in 0..17 {
                let pos = (start + step as f64 * 0.007) * COUNT as f64;
                let k = pos.floor() as usize % COUNT;
                let next = (k + 1) % COUNT;
                let f = pos.fract();
                let mix = |p: f64, q: f64| p + (q - p) * f;
                let px = mix(points[k][0], points[next][0]);
                let py = mix(points[k][1], points[next][1]);
                let half = width * 0.125 * (PI * step as f64 / 16.).sin().powi(2);
                let dx = mix(samples[k].nx, samples[next].nx) * half;
                let dy = mix(samples[k].ny, samples[next].ny) * half;
                upper[step] = [px + dx, py + dy];
                lower[step] = [px - dx, py - dy];
            }
            let mut streak = Path::builder().move_to(upper[0][0], upper[0][1]);
            for &[x, y] in upper[1..].iter().chain(lower[1..16].iter().rev()) {
                streak = streak.line_to(x, y);
            }
            c.fill(
                streak.close().build()?,
                Brush::Solid(tint(0xcbf1ff, 0.88 * intensity)),
            )?;
        }
    }
    Ok(())
}
