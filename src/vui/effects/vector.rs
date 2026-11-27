//! Resolution-independent optical and animation effects on the overlay vector canvas.

//!
//! Pass application geometry in logical units. Apply the canvas transform and clip once
//! for the destination. A [`Scene`] supplies the light field bounds and elevated surfaces.
//! The foreground callback draws your artwork between the shadow and light passes.
//! [`liquid_glass`] replays a paint-only callback through its lens and optional wave.
//!
//! ```
//! use vlib::vui::{Color, effects::{Surface, vector::{self, Brush, Canvas, Path, Rect, Scene}}};
//!
//! let bounds = Rect::new(0., 0., 320., 180.)?;
//! let panel = Surface {
//!     rect: [40., 50., 100., 45.], radius: 8., color: Color(0x405570ff),
//!     elevation: 10, emitter: false,
//! };
//! let scene = Scene { bounds, surfaces: &[panel] };
//! let mut canvas = Canvas::new();
//! vector::lamp(&mut canvas, scene, [80., 30.], 32., 7., 1.25, |canvas| {
//!     let [x, y, w, h] = panel.rect;
//!     let path = Path::rounded_rectangle(Rect::new(x, y, w, h)?, panel.radius)?;
//!     canvas.fill(path, Brush::Solid(panel.color))?;
//!     Ok(())
//! })?;
//! canvas.validate()?;
//! # Ok::<(), vector::InvalidScene>(())
//! ```

// Rust guideline compliant 2026-10-07
mod passes;
pub use passes::{
    Glass, Pulse, RoundedBox, Scene, charge, electric_border, fire, flashlight, glow, glow_path,
    god_rays, lamp, liquid_glass, ping, plasma, pulse, radar, ripple, zap,
};
use std::f64::consts::PI;
pub use vivid_protocol::vector::{Brush, Canvas, InvalidScene, Path, Rect};
use vivid_protocol::vector::{Cap, Color, Join, StrokeStyle};

fn tint(rgb: u32, opacity: f64) -> Brush {
    Brush::Solid(Color(
        (rgb << 8) | (opacity.clamp(0., 1.) * 255.).round() as u32,
    ))
}

/// Draw a luminous ring with a soft outer stroke and a fine core.
///
/// # Errors
///
/// Returns the wire geometry or display list error when a drawing exceeds its bounds.
pub fn pulse_ring(
    canvas: &mut Canvas,
    [x, y]: [f64; 2],
    radius: f64,
    width: f64,
    rgb: u32,
    opacity: f64,
) -> Result<(), InvalidScene> {
    if radius <= 0. || width <= 0. || opacity <= 0. {
        return Ok(());
    }
    let path = Path::ellipse(Rect::new(x - radius, y - radius, 2. * radius, 2. * radius)?)?;
    for (spread, power) in [(3., 0.10), (1.6, 0.24), (1., 0.88)] {
        canvas.stroke(path.clone(), tint(rgb, opacity * power), width * spread)?;
    }
    Ok(())
}

/// Shape and color of a time-varying, optionally forked discharge.
#[derive(Clone, Copy, Debug)]
pub struct Electricity {
    pub time: f64,
    pub amplitude: f64,
    pub glow: f64,
    /// A 24-bit sRGB color.
    pub rgb: u32,
    pub seed: u32,
    pub forks: bool,
}

/// Draw a branching arc between pinned endpoints in local canvas coordinates.
///
/// # Errors
///
/// Returns the wire geometry or display list error when a drawing exceeds its bounds.
pub fn electric_arc(
    canvas: &mut Canvas,
    [x, y]: [f64; 2],
    end: [f64; 2],
    effect: Electricity,
) -> Result<(), InvalidScene> {
    let (dx, dy) = (end[0] - x, end[1] - y);
    let length = dx.hypot(dy);
    if length < 0.01 || effect.glow <= 0. {
        return Ok(());
    }
    let count = (length / 2.6).ceil().clamp(5., 48.) as u32;
    let mut path = Path::builder().move_to(x, y);
    let mut branches = Path::builder();
    let phase = (effect.time % 4096.) * 12. + f64::from(effect.seed) * 2.7;
    for i in 1..=count {
        let t = f64::from(i) / f64::from(count);
        let j = f64::from(i);
        let noise = (j * 2.31 + phase).sin() * 0.44
            + (j * 0.79 - phase * 0.63).sin() * 0.56
            + (j * 4.3 - phase * 1.4).sin() * 0.24;
        let offset = effect.amplitude * (PI * t).sin() * noise;
        let (px, py) = (
            x + dx * t - dy / length * offset,
            y + dy * t + dx / length * offset,
        );
        path = path.line_to(px, py);
        if effect.forks && i % 8 == 4 && i < count - 2 {
            let side = if i % 16 == 4 {
                -effect.amplitude
            } else {
                effect.amplitude
            };
            let n = f64::from(count);
            branches = branches
                .move_to(px, py)
                .line_to(
                    px + dx / n * 1.5 - dy / length * side,
                    py + dy / n * 1.5 + dx / length * side,
                )
                .line_to(
                    px + dx / n * 3. - dy / length * side * 1.7,
                    py + dy / n * 3. + dx / length * side * 1.7,
                );
        }
    }
    let path = path.build()?;
    for (width, color, power) in [
        (5.4, effect.rgb, 0.07),
        (2.8, effect.rgb, 0.18),
        (1.3, effect.rgb, 0.72),
        (0.43, 0xecf9ff, 1.),
    ] {
        let mut style = StrokeStyle::new(width)?;
        style.cap = Cap::Round;
        style.join = Join::Round;
        canvas.stroke_styled(path.clone(), tint(color, effect.glow * power), style)?;
    }
    if effect.forks && count >= 7 {
        canvas.stroke(
            branches.build()?,
            tint(effect.rgb, effect.glow * 0.68),
            0.38,
        )?;
    }
    Ok(())
}
