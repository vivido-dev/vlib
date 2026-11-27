//! Scalar reference passes over a prepared canvas.

// Rust guideline compliant 2026-10-07
use super::{Effect, EffectScene, Light, Wave, invalid};
use std::io;

pub(super) fn over(pixel: &mut [u8], rgb: &[u8], opacity: f64) {
    let a = opacity.clamp(0., 1.);
    if a == 0. {
        return;
    }
    let tail = f64::from(pixel[3]) / 255. * (1. - a);
    let alpha = a + tail;
    for c in 0..3 {
        pixel[c] = ((f64::from(rgb[c]) * a + f64::from(pixel[c]) * tail) / alpha).round() as u8;
    }
    pixel[3] = (255. * alpha).round() as u8;
}

fn point(p: [f64; 2]) -> bool {
    p.iter().all(|v| v.is_finite() && v.abs() <= 32768.)
}
fn span(v: f64) -> bool {
    (0.001..=32768.).contains(&v)
}
fn light_ok(l: Light) -> bool {
    point(l.position)
        && span(l.height)
        && span(l.radius)
        && (0.0..=16.).contains(&l.intensity)
        && (0.0..=1.).contains(&l.ambient)
}
fn valid(e: Effect) -> bool {
    match e {
        Effect::Lamp(l) => light_ok(l),
        Effect::Flashlight { light, aim, beam } => {
            light_ok(light) && point(aim) && (0.01..=1.56).contains(&beam)
        }
        Effect::GodRays { light, softness } => light_ok(light) && (0.1..=8.).contains(&softness),
        Effect::Glow {
            radius, intensity, ..
        } => span(radius) && (0.0..=16.).contains(&intensity),
        Effect::Fire {
            height,
            speed,
            embers,
            time,
            ..
        } => {
            span(height)
                && (0.0..=16.).contains(&speed)
                && (0.0..=1.).contains(&embers)
                && time.is_finite()
                && time >= 0.
        }
        Effect::LiquidGlass {
            center,
            size,
            depth,
            dispersion,
            frost,
            wave,
        } => {
            point(center)
                && size.into_iter().all(span)
                && (0.0..=256.).contains(&depth)
                && (0.0..=1.).contains(&dispersion)
                && (0.0..=1.).contains(&frost)
                && wave.is_none_or(|w| point(w.center) && w.age.is_finite() && w.age >= 0.)
        }
    }
}
pub(super) fn frame(s: &EffectScene, e: Effect) -> io::Result<Vec<u8>> {
    if !valid(e) {
        return Err(invalid("invalid effect parameters"));
    }
    let mut output = s.art.clone();
    if let Effect::Glow {
        color,
        radius,
        intensity,
    } = e
    {
        s.glow(&mut output, color, radius, intensity);
        return Ok(output);
    }
    let [sx, sy] = s.spacing();
    for (i, pixel) in output.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let x = (i % s.raster[0]) as f64 * sx + sx * 0.5;
        let y = (i / s.raster[0]) as f64 * sy + sy * 0.5;
        match e {
            Effect::Lamp(light)
            | Effect::Flashlight { light, .. }
            | Effect::GodRays { light, .. } => s.light(pixel, i, [x, y], light, e),
            Effect::Glow { .. } => unreachable!("glow is a separable image pass"),
            Effect::Fire {
                height,
                speed,
                embers,
                time,
                seed,
            } => {
                let rise = s.rise[i];
                if rise <= height * 1.8 && rise > 0. {
                    let t = (time % 65536.) * speed;
                    let phase = f64::from(seed % 65521) * 0.063;
                    let u = x / height;
                    let n = (u * 13. + t * 3. + phase + (u * 7. - t * 2.).sin()).sin() * 0.5
                        + (u * 28. - t * 4. + phase + rise / height * 2.).sin() * 0.3
                        + (u * 51. + t * 5.).sin() * 0.2;
                    let heat = (1. - rise / (height * (0.62 + n * 0.4))).clamp(0., 1.);
                    let rgb = [
                        255,
                        (40. + 206. * heat).round() as u8,
                        (155. * heat.powi(3)).round() as u8,
                    ];
                    over(pixel, &rgb, (heat * 3.).min(1.));
                    let cell = (u * 5.).floor();
                    let phase = (cell * 73.7 + phase).sin() * 19.1;
                    let altitude = ((t * 0.52 + phase).rem_euclid(1.) * 1.65 + 0.1) * height;
                    let lateral = ((u * 5.).rem_euclid(1.) - 0.5 - 0.28 * (t * 2. + phase).sin())
                        * height
                        / 5.;
                    let spark = (1.
                        - lateral.hypot((rise - altitude) * 0.7) / (1. + height * 0.025))
                        .max(0.);
                    over(pixel, &[255, 199, 92], embers * spark);
                }
            }
            Effect::LiquidGlass {
                center,
                size,
                depth,
                dispersion,
                frost,
                wave,
            } => {
                s.glass(
                    pixel,
                    [x, y],
                    center,
                    size,
                    [depth, dispersion, frost],
                    wave,
                );
            }
        }
    }
    Ok(output)
}

impl EffectScene {
    // Two separable exponential blurs preserve fractional coverage and spread light
    // beyond even a one-pixel glyph. Outside the source image emission is zero.
    fn bloom(&self, radius: f64) -> Vec<f64> {
        let [w, h] = self.raster;
        let [sx, sy] = self.spacing();
        let mut f: Vec<_> = self.emission.iter().map(|&a| f64::from(a) / 255.).collect();
        for (lines, length, stride, spacing, pitch) in [(h, w, 1, sx, w), (w, h, w, sy, 1)] {
            let decay = (-spacing / radius).exp();
            for line in 0..lines {
                let start = line * pitch;
                let mut v = 0.;
                for j in 0..length {
                    let i = start + j * stride;
                    v = f[i] + decay * (v - f[i]);
                    f[i] = v;
                }
                // The forward tail outside this edge decays geometrically.
                v /= 1. + decay;
                for j in (0..length).rev() {
                    let i = start + j * stride;
                    v = f[i] + decay * (v - f[i]);
                    f[i] = v;
                }
            }
        }
        f
    }

    fn glow(&self, out: &mut [u8], color: [u8; 3], radius: f64, intensity: f64) {
        let halo = self.bloom(radius);
        let core = self.bloom(radius * 0.20);
        for (i, pixel) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let source = f64::from(self.emission[i]) / 255.;
            let edge = (source - core[i]).abs();
            over(
                pixel,
                &color,
                (edge * 1.9 + (halo[i] - 0.75 * source).max(0.) * 1.8) * intensity,
            );
            over(pixel, &[250, 250, 255], edge * 0.95 * intensity);
        }
    }
    fn index(&self, x: f64, y: f64) -> usize {
        let [w, h] = self.raster;
        ((y * h as f64 / self.logical[1]) as usize).min(h - 1) * w
            + ((x * w as f64 / self.logical[0]) as usize).min(w - 1)
    }

    fn average_height(&self, x: f64, y: f64, r: f64) -> f64 {
        let [w, h] = self.raster;
        let [sx, sy] = self.spacing();
        let x0 = (((x - r) / sx).floor() as usize).min(w - 1);
        let y0 = (((y - r) / sy).floor() as usize).min(h - 1);
        let x1 = (((x + r) / sx).ceil() as usize).clamp(x0 + 1, w);
        let y1 = (((y + r) / sy).ceil() as usize).clamp(y0 + 1, h);
        let stride = w + 1;
        let a = &self.heights_sum;
        let total =
            a[y1 * stride + x1] + a[y0 * stride + x0] - a[y0 * stride + x1] - a[y1 * stride + x0];
        f64::from(total) / ((x1 - x0) * (y1 - y0)) as f64
    }

    fn light(&self, pixel: &mut [u8], i: usize, p: [f64; 2], l: Light, e: Effect) {
        let [x, y] = p;
        let [dx, dy] = [l.position[0] - x, l.position[1] - y];
        let z = f64::from(self.heights[i]);
        let reach = 2. * l.height + l.radius * 3.;
        let fall = 1. / (1. + (dx * dx + dy * dy) / (reach * reach));
        let mut beam = 1.;
        if let Effect::Flashlight {
            aim, beam: angle, ..
        } = e
        {
            let [ax, ay] = [aim[0] - l.position[0], aim[1] - l.position[1]];
            let dz = z - l.height;
            let cosine = (-dx * ax - dy * ay - dz * l.height)
                / ((dx * dx + dy * dy + dz * dz).sqrt()
                    * (ax * ax + ay * ay + l.height * l.height).sqrt())
                .max(1e-6);
            beam = ((cosine - angle.cos()) / (1. - angle.cos()) * 1.5).clamp(0., 1.);
            beam = beam * beam * (3. - 2. * beam);
        }
        let mut shadow: f64 = 0.;
        let mut open = 0.;
        // Midpoint samples bound each ray's work independently of canvas size.
        for step in 0..12 {
            let t = (f64::from(step) + 0.5) / 12.;
            let px = x + dx * t;
            let py = y + dy * t;
            if px < 0. || py < 0. || px >= self.logical[0] || py >= self.logical[1] {
                open += 1.;
                continue;
            }
            let footprint = match e {
                Effect::GodRays { softness, .. } => softness * (0.6 + t * 1.8),
                _ => 0.4 + t * l.radius * 0.65,
            };
            let h = self.average_height(px, py, footprint);
            let clearance = h - (z + (l.height - z) * t);
            shadow = shadow.max((clearance / (1. + l.radius * t)).clamp(0., 1.));
            if let Effect::GodRays { softness, .. } = e {
                open += 1. / (1. + h / softness);
            }
        }
        let mut power = fall * beam * (1. - 0.88 * shadow) * l.intensity;
        if matches!(e, Effect::GodRays { .. }) {
            power = fall * l.intensity * (open / 12f64).powi(2) * if z > 0. { 0.12 } else { 1. };
        }
        for (c, channel) in pixel[..3].iter_mut().enumerate() {
            let tint = f64::from(l.color[c]) / 255.;
            let base = f64::from(*channel);
            *channel = (base * l.ambient + (base * 0.86 + 22.) * tint * power)
                .round()
                .clamp(0., 255.) as u8;
        }
        if matches!(e, Effect::Lamp(_)) {
            let bulb = (-(dx * dx + dy * dy) / (l.radius * l.radius * 0.35)).exp() * l.intensity;
            over(pixel, &l.color, bulb);
        }
    }

    fn glass(
        &self,
        pixel: &mut [u8],
        p: [f64; 2],
        center: [f64; 2],
        size: [f64; 2],
        params: [f64; 3],
        wave: Option<Wave>,
    ) {
        let [depth, dispersion, frost] = params;
        let [x, y] = p;
        let radius = size[0].min(size[1]) * 0.5;
        let [dx, dy] = [x - center[0], y - center[1]];
        let nx = dx - dx.clamp(-size[0] * 0.5 + radius, size[0] * 0.5 - radius);
        let ny = dy - dy.clamp(-size[1] * 0.5 + radius, size[1] * 0.5 - radius);
        let norm = nx.hypot(ny);
        let inside = norm <= radius;
        let mut px = x;
        let mut py = y;
        if let Some(w) = wave
            && w.age < 1.
        {
            let vx = x - w.center[0];
            let vy = y - w.center[1];
            let d = vx.hypot(vy).max(0.001);
            let band = (d - w.age * self.logical[0] * 0.85) / 5.;
            let push = band * (-band * band * 0.5).exp() * 7. * (1. - w.age);
            px += vx / d * push;
            py += vy / d * push;
        }
        if !inside && px == x && py == y {
            return;
        }
        let rim = if inside { (norm / radius).powi(4) } else { 0. };
        let bend = depth * rim / norm.max(0.001);
        px -= nx * bend;
        py -= ny * bend;
        let mid = self.index(px, py) * 4;
        pixel.copy_from_slice(&self.art[mid..mid + 4]);
        if !inside {
            return;
        }
        for (c, channel) in pixel[..3].iter_mut().enumerate() {
            let split = (c as f64 - 1.) * dispersion * bend;
            let u = px + nx * split;
            let v = py + ny * split;
            let i = self.index(u, v) * 4;
            // Dispersion shifts color channels while coverage follows the central ray.
            // A transparent neighboring texel contributes none of its hidden RGB.
            let base = f64::from(self.art[mid + c]);
            let coverage =
                (f64::from(self.art[i + 3]) / f64::from(self.art[mid + 3].max(1))).min(1.);
            let mut value = base + (f64::from(self.art[i + c]) - base) * coverage;
            if frost > 0. {
                let r = 1. + frost * 4.;
                let mut total = 0.;
                let mut weight = 0.;
                for (ox, oy) in [(-r, 0.), (r, 0.), (0., -r), (0., r)] {
                    let k = self.index(u + ox, v + oy) * 4;
                    let a = f64::from(self.art[k + 3]);
                    total += f64::from(self.art[k + c]) * a;
                    weight += a;
                }
                if weight > 0. {
                    value += (total / weight - value) * frost;
                }
            }
            *channel = value.round() as u8;
        }
        let highlight = ((-nx - ny) / norm.max(0.001) * 0.4 + 0.5).clamp(0., 1.);
        over(
            pixel,
            &[225, 245, 255],
            rim.powi(3) * (0.18 + highlight * 0.65) + frost * 0.08,
        );
    }
}
