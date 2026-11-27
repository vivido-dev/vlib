//! Software lighting and optical effects for a producer-owned canvas.
//!
//! Build the artwork once, retain the scene, and change the effect as input arrives.
//! Geometry is in canvas-local logical units. Raster dimensions are independent.
//! Outputs are straight-alpha sRGB RGBA8, suitable for a Vivid retained image.

// Rust guideline compliant 2026-10-07
use std::{io, rc::Rc};

use crate::vui::node::NodeKind;
use crate::vui::{Color, Div, ElementId, div};

mod render;

/// Largest RGBA image that fits the SDK's 256 KiB overlay record with its 16-byte header.
const MAX_PIXELS: usize = (256 * 1024 - 16) / 4;
/// Bounds work when rasterizing overlapping geometry.
const MAX_SURFACES: usize = 256;

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// A painted rounded rectangle. Equal sides and a half-width radius form a circle.
#[derive(Clone, Copy, Debug)]
pub struct Surface {
    /// Left, top, width and height in logical units.
    pub rect: [f64; 4],
    pub radius: f64,
    pub color: Color,
    /// Height above the backdrop, in logical units. Zero paints a flat marking.
    pub elevation: u8,
    /// Include this silhouette in the glow and fire source.
    pub emitter: bool,
}

/// Prepared artwork, occluder heights, and emission fields shared between frames.
#[derive(Debug, PartialEq)]
pub struct EffectScene {
    logical: [f64; 2],
    raster: [usize; 2],
    art: Vec<u8>,
    heights: Vec<u8>,
    heights_sum: Vec<u32>,
    emission: Vec<u8>,
    rise: Vec<f64>,
}

impl EffectScene {
    /// Prepare a scene from a background, optional raster artwork, and ordered surfaces.
    ///
    /// An RGBA backdrop replaces the solid background. An optional raster-sized alpha
    /// mask adds emitters to those marked on surfaces. Geometry is clipped to the canvas.
    ///
    /// # Errors
    ///
    /// Returns invalid input for unsupported sizes, byte lengths, or geometry.
    pub fn new(
        logical: [f64; 2],
        raster: [usize; 2],
        background: Color,
        backdrop: Option<&[u8]>,
        surfaces: &[Surface],
        mask: Option<&[u8]>,
    ) -> io::Result<Self> {
        let [w, h] = raster;
        let n = w
            .checked_mul(h)
            .filter(|&n| n > 0 && n <= MAX_PIXELS)
            .ok_or_else(|| invalid("effect raster exceeds its pixel budget"))?;
        if !logical.iter().all(|&v| (0.001..=16384.).contains(&v))
            || backdrop.is_some_and(|b| b.len() != n * 4)
            || mask.is_some_and(|m| m.len() != n)
            || surfaces.len() > MAX_SURFACES
        {
            return Err(invalid("invalid effect canvas extent, artwork or mask"));
        }
        for s in surfaces {
            if !s.rect.iter().all(|v| v.is_finite() && v.abs() <= 32768.)
                || s.rect[2] < 0.
                || s.rect[3] < 0.
                || !(0.0..=32768.).contains(&s.radius)
            {
                return Err(invalid("invalid effect surface geometry"));
            }
        }
        let mut scene = Self {
            logical,
            raster,
            art: backdrop.map_or_else(|| background.0.to_be_bytes().repeat(n), <[u8]>::to_vec),
            heights: vec![0; n],
            heights_sum: vec![0; (w + 1) * (h + 1)],
            emission: mask.map_or_else(|| vec![0; n], <[u8]>::to_vec),
            rise: vec![0.; n],
        };
        for surface in surfaces {
            scene.stamp(surface);
        }
        scene.prepare_fields();
        Ok(scene)
    }

    fn stamp(&mut self, s: &Surface) {
        let [w, h] = self.raster;
        let [sx, sy] = self.spacing();
        let [left, top, width, height] = s.rect;
        if width == 0. || height == 0. {
            return;
        }
        let radius = s.radius.min(width * 0.5).min(height * 0.5);
        let color = s.color.0.to_be_bytes();
        for y in (top / sy).floor().max(0.) as usize..(((top + height) / sy).ceil() as usize).min(h)
        {
            for x in
                (left / sx).floor().max(0.) as usize..(((left + width) / sx).ceil() as usize).min(w)
            {
                let dx = (((x as f64 + 0.5) * sx - left - width * 0.5).abs() - width * 0.5
                    + radius)
                    .max(0.);
                let dy = (((y as f64 + 0.5) * sy - top - height * 0.5).abs() - height * 0.5
                    + radius)
                    .max(0.);
                if dx * dx + dy * dy > radius * radius {
                    continue;
                }
                let i = y * w + x;
                let dst = &mut self.art[i * 4..i * 4 + 4];
                render::over(dst, &color[..3], f64::from(color[3]) / 255.);
                if color[3] > 0 {
                    self.heights[i] = self.heights[i].max(s.elevation);
                }
                if s.emitter {
                    self.emission[i] = self.emission[i].max(color[3]);
                }
            }
        }
    }

    fn spacing(&self) -> [f64; 2] {
        [
            self.logical[0] / self.raster[0] as f64,
            self.logical[1] / self.raster[1] as f64,
        ]
    }

    fn prepare_fields(&mut self) {
        let [w, h] = self.raster;
        let sy = self.spacing()[1];

        // A summed-area table prefilters the blockers over each ray sample's footprint.
        for y in 0..h {
            let mut row = 0;
            for x in 0..w {
                row += u32::from(self.heights[y * w + x]);
                self.heights_sum[(y + 1) * (w + 1) + x + 1] =
                    self.heights_sum[y * (w + 1) + x + 1] + row;
            }
        }
        for i in (0..w * h).rev() {
            self.rise[i] = if self.emission[i] > 0 {
                0.
            } else if i + w < w * h {
                self.rise[i + w] + sy
            } else {
                f64::INFINITY
            };
        }
    }

    /// Pixel dimensions of the rendered image.
    pub fn raster(&self) -> [usize; 2] {
        self.raster
    }

    /// Render a single deterministic frame at the time supplied in the effect.
    ///
    /// # Errors
    ///
    /// Returns invalid input for nonfinite or out-of-range parameters.
    pub fn render(&self, effect: Effect) -> io::Result<Vec<u8>> {
        render::frame(self, effect)
    }
}

/// A colored point source above the canvas, in logical units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub position: [f64; 2],
    pub height: f64,
    pub radius: f64,
    pub color: [u8; 3],
    pub intensity: f64,
    pub ambient: f64,
}

/// An expanding lens distortion, active for one second from age zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wave {
    pub center: [f64; 2],
    pub age: f64,
}

/// Parameters for one canvas pass. Positions and lengths use logical units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    Lamp(Light),
    /// Beam is the half-angle in radians.
    Flashlight {
        light: Light,
        aim: [f64; 2],
        beam: f64,
    },
    GodRays {
        light: Light,
        softness: f64,
    },
    Glow {
        color: [u8; 3],
        radius: f64,
        intensity: f64,
    },
    Fire {
        height: f64,
        speed: f64,
        embers: f64,
        time: f64,
        seed: u32,
    },
    LiquidGlass {
        center: [f64; 2],
        size: [f64; 2],
        depth: f64,
        dispersion: f64,
        frost: f64,
        wave: Option<Wave>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EffectSpec {
    pub scene: Rc<EffectScene>,
    pub effect: Effect,
}

/// Display a prepared effect scene inside a standard interactive layout element.
pub fn effect_canvas(id: impl Into<ElementId>, scene: Rc<EffectScene>, effect: Effect) -> Div {
    let mut element = div()
        .id(id)
        .size(scene.logical[0] as f32, scene.logical[1] as f32);
    element.node.kind = NodeKind::Effect(EffectSpec { scene, effect });
    element
}
