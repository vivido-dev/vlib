//! Text: measuring it, shaping it, and keeping the shapes the frame is drawing.
//!
//! The host is the font system. Everything here is a cache in front of it, with two purposes: a
//! frame measures each distinct run once no matter how many elements draw it, and a run that must
//! be drawn as a shaped paragraph holds one retained layout rather than one per frame.
//!
//! Both caches are bounded, and the layouts are released when the scene that drew them has been
//! replaced — a retained layout is the host's memory, not ours.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io;

use vivid_protocol::overlay::wire::text::TextMeasurement;
use vivid_protocol::overlay::wire::text::styled::{
    MAX_TEXT_BATCH, StyledText, TextAlignment, TextOverflow, TextRun, TextStyle, Typography,
};
use vivid_protocol::vector::Scalar;
use vivid_sdk::overlay::{OverlayWindow, RetainedTextLayout};

use crate::vui::geometry::Size;
use crate::vui::node::TextSpec;

/// How many distinct runs a window remembers the measurements of.
const MEASUREMENT_CACHE: usize = 2048;

/// How many measurements may be in flight in one batch request, by bytes rather than count.
const MAX_BATCH_BYTES: usize = 64 * 1024;

/// The key a *measurement* is cached under: everything that changes how big the run is.
///
/// Color is deliberately absent — a recolor does not change a size, and measuring again for one
/// would be a host round trip for nothing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MeasureKey {
    runs: Vec<RunKey>,
    wrap: bool,
    max_width: Option<u32>,
    ellipsis: bool,
    max_lines: Option<u16>,
    align: u8,
    /// Typography is part of what a measurement *is*: a paragraph with a line height of 40 is not
    /// the same shape as one with a line height of 20, and two runs that differ only there must
    /// not share a cached answer.
    letter_spacing: u32,
    word_spacing: u32,
    line_height: Option<u32>,
    ligatures: bool,
    kerning: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RunKey {
    text: String,
    size: u32,
    weight: u16,
    family: String,
    italic: bool,
    underline: bool,
    strikethrough: bool,
}

impl MeasureKey {
    fn of(spec: &TextSpec) -> Self {
        Self {
            runs: spec
                .runs
                .iter()
                .map(|run| RunKey {
                    text: run.text.clone(),
                    size: run.size.to_bits(),
                    weight: run.weight,
                    family: run.family.clone(),
                    italic: run.italic,
                    underline: run.underline,
                    strikethrough: run.strikethrough,
                })
                .collect(),
            wrap: spec.wrap,
            max_width: spec.max_width.map(f32::to_bits),
            ellipsis: spec.ellipsis,
            max_lines: spec.max_lines,
            align: spec.align as u8,
            letter_spacing: spec.letter_spacing.to_bits(),
            word_spacing: spec.word_spacing.to_bits(),
            line_height: spec.line_height.map(f32::to_bits),
            ligatures: spec.ligatures,
            kerning: spec.kerning,
        }
    }
}

/// The key a *shape* is cached under: a measurement, plus everything else the drawn scene
/// carries. A retained layout paints its own color and decorations, so those are part of it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct LayoutKey {
    measure: MeasureKey,
    colors: Vec<u32>,
}

impl LayoutKey {
    fn of(spec: &TextSpec) -> Self {
        Self {
            measure: MeasureKey::of(spec),
            colors: spec.runs.iter().map(|run| run.color.0).collect(),
        }
    }
}

/// Measurements and retained layouts, in front of the host's font system.
#[derive(Default)]
pub(crate) struct TextSystem {
    measurements: HashMap<MeasureKey, TextMeasurement>,
    measurement_order: VecDeque<MeasureKey>,
    /// Shaped paragraphs the host is holding for us, and the frame that last drew each.
    layouts: HashMap<LayoutKey, RetainedTextLayout>,
    layout_order: VecDeque<LayoutKey>,
    /// Keys the frame being painted drew through a retained layout.
    drawn: HashSet<LayoutKey>,
}

impl TextSystem {
    /// Measure whatever this frame needs and is not already known.
    ///
    /// Returns the number of runs the host had to shape.
    pub(crate) fn measure(
        &mut self,
        overlay: &OverlayWindow,
        specs: &[TextSpec],
    ) -> io::Result<usize> {
        let mut wanted: Vec<(MeasureKey, StyledText)> = Vec::new();
        for spec in specs {
            let key = MeasureKey::of(spec);
            if self.measurements.contains_key(&key) || wanted.iter().any(|(k, _)| *k == key) {
                continue;
            }
            wanted.push((key, styled(spec)?));
        }
        if wanted.is_empty() {
            return Ok(0);
        }

        // A host measures a bounded batch, so a frame with more runs than one batch holds is
        // measured in as many requests as it takes. The bounds are the protocol's, not guesses.
        let measured_count = wanted.len();
        let mut batch: Vec<(MeasureKey, StyledText)> = Vec::new();
        let mut bytes = 0usize;
        for (key, text) in wanted {
            let size = text.text().len();
            if !batch.is_empty()
                && (batch.len() >= MAX_TEXT_BATCH || bytes + size > MAX_BATCH_BYTES)
            {
                self.measure_batch(overlay, &mut batch)?;
                bytes = 0;
            }
            bytes += size;
            batch.push((key, text));
        }
        self.measure_batch(overlay, &mut batch)?;
        Ok(measured_count)
    }

    fn measure_batch(
        &mut self,
        overlay: &OverlayWindow,
        batch: &mut Vec<(MeasureKey, StyledText)>,
    ) -> io::Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        let texts: Vec<StyledText> = batch.iter().map(|(_, text)| text.clone()).collect();
        let measured = overlay.measure_text_batch(&texts)?;
        if measured.len() != batch.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "host returned a different number of measurements than runs",
            ));
        }
        for ((key, _), measurement) in batch.drain(..).zip(measured) {
            self.insert(key, measurement);
        }
        Ok(())
    }

    /// The key a run is measured under. Public to the crate so a caller can look a measurement
    /// up without going back through the spec.
    pub(crate) fn key_of(spec: &TextSpec) -> MeasureKey {
        MeasureKey::of(spec)
    }

    fn insert(&mut self, key: MeasureKey, measurement: TextMeasurement) {
        if self.measurements.insert(key.clone(), measurement).is_none() {
            self.measurement_order.push_back(key);
        }
        while self.measurement_order.len() > MEASUREMENT_CACHE {
            if let Some(oldest) = self.measurement_order.pop_front() {
                self.measurements.remove(&oldest);
            }
        }
    }

    /// The measured size of a run, if this window has measured it.
    pub(crate) fn size(&self, spec: &TextSpec) -> Option<Size> {
        self.measurements.get(&MeasureKey::of(spec)).map(size_of)
    }

    /// Everything the host said about a run, geometry included. This is what a caret is placed
    /// from: the clusters are the shape, and a caret sits between two of them.
    pub(crate) fn measurement(&self, spec: &TextSpec) -> Option<&TextMeasurement> {
        self.measurements.get(&MeasureKey::of(spec))
    }

    /// Shape every paragraph a frame is about to draw, in as few requests as the host allows.
    ///
    /// Retaining one at a time is a request per paragraph, and a table of a hundred cells is a
    /// hundred round trips for one frame. The host takes a batch for exactly this reason, and
    /// measurement already used it; this is the same bargain for the shapes.
    pub(crate) fn prepare_shapes(
        &mut self,
        overlay: &OverlayWindow,
        specs: &[TextSpec],
        budget: usize,
    ) -> io::Result<usize> {
        let mut wanted: Vec<(LayoutKey, StyledText)> = Vec::new();
        for spec in specs {
            if !needs_shape(spec) {
                continue;
            }
            let key = LayoutKey::of(spec);
            if self.layouts.contains_key(&key) || wanted.iter().any(|(held, _)| *held == key) {
                continue;
            }
            if self.layouts.len() + wanted.len() >= budget {
                // The host holds a bounded number. Past it a paragraph is drawn plainly, which
                // loses its styling and keeps its frame.
                break;
            }
            wanted.push((key, styled(spec)?));
        }
        if wanted.is_empty() {
            return Ok(0);
        }

        let shaped = wanted.len();
        for chunk in wanted.chunks(MAX_TEXT_BATCH) {
            let texts: Vec<StyledText> = chunk.iter().map(|(_, text)| text.clone()).collect();
            let layouts = overlay.layout_text_batch(&texts)?;
            if layouts.len() != chunk.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "host returned a different number of layouts than paragraphs",
                ));
            }
            for ((key, _), layout) in chunk.iter().zip(layouts) {
                self.insert(key.measure.clone(), layout.measurement().clone());
                self.layouts.insert(key.clone(), layout);
                self.layout_order.push_back(key.clone());
            }
        }
        Ok(shaped)
    }

    /// Shape a run into a paragraph the host keeps, and answer the identity the scene draws
    /// through.
    ///
    /// `None` means the host is already holding as many layouts as it will, and the caller draws
    /// the run the plain way instead.
    pub(crate) fn retain(
        &mut self,
        overlay: &OverlayWindow,
        spec: &TextSpec,
        budget: usize,
    ) -> io::Result<Option<u64>> {
        let key = LayoutKey::of(spec);
        if let Some(layout) = self.layouts.get(&key) {
            self.drawn.insert(key);
            return Ok(Some(layout.id()));
        }
        if self.layouts.len() >= budget {
            return Ok(None);
        }
        // Reached only for a paragraph the frame did not know it would draw — a fallback, since
        // `prepare_shapes` has already batched everything the tree asked for.
        let layout = overlay.layout_text(&styled(spec)?)?;
        let id = layout.id();
        self.insert(key.measure.clone(), layout.measurement().clone());
        self.layouts.insert(key.clone(), layout);
        self.layout_order.push_back(key.clone());
        self.drawn.insert(key);
        Ok(Some(id))
    }

    /// Let go of the layouts the frame that just finished did not draw, and of the measurements
    /// of runs nothing is drawing any more.
    ///
    /// Called once a submission has been replaced: until then the host may still be presenting
    /// the scene that references them.
    pub(crate) fn advance(&mut self, overlay: &OverlayWindow) -> io::Result<()> {
        let keep: HashSet<LayoutKey> = std::mem::take(&mut self.drawn);
        let stale: Vec<LayoutKey> = self
            .layouts
            .keys()
            .filter(|key| !keep.contains(*key))
            .cloned()
            .collect();
        for key in stale {
            if let Some(layout) = self.layouts.remove(&key) {
                // A host that has already forgotten it is not an error: the point is that we do
                // not hold it either.
                let _ = overlay.release_text_layout(&layout);
            }
            self.layout_order.retain(|held| held != &key);
        }
        Ok(())
    }

    /// Forget everything. A host that changed its fonts or its scale invalidates every
    /// measurement and every shape at once.
    pub(crate) fn invalidate(&mut self, overlay: &OverlayWindow) {
        for (_, layout) in self.layouts.drain() {
            let _ = overlay.release_text_layout(&layout);
        }
        self.measurements.clear();
        self.measurement_order.clear();
        self.layout_order.clear();
        self.drawn.clear();
    }

    /// How many shaped paragraphs the host is holding for this window.
    #[cfg(feature = "testing")]
    pub(crate) fn retained(&self) -> usize {
        self.layouts.len()
    }
}

fn size_of(measurement: &TextMeasurement) -> Size {
    Size::new(
        measurement.width.get() as f32,
        measurement.height.get() as f32,
    )
}

/// The wire form of a paragraph.
pub(crate) fn styled(spec: &TextSpec) -> io::Result<StyledText> {
    let runs = spec
        .runs
        .iter()
        .map(|run| {
            Ok(TextRun {
                text: run.text.clone(),
                style: TextStyle {
                    size: scalar(run.size)?,
                    family: run.family.clone(),
                    weight: run.weight,
                    italic: run.italic,
                    color: run.color,
                    underline: run.underline,
                    strikethrough: run.strikethrough,
                },
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    // The spellings are clamped, not refused. The wire carries spacing as an unsigned length and
    // a line height inside a fixed ceiling, and a spacing a few pixels out of range is a value a
    // view got slightly wrong — not a reason to take the window down. Every other style value
    // here is treated the same way.
    let typography = Typography {
        overflow: if spec.ellipsis {
            TextOverflow::Ellipsis
        } else {
            TextOverflow::Clip
        },
        letter_spacing: spacing(spec.letter_spacing)?,
        word_spacing: spacing(spec.word_spacing)?,
        line_height: spec
            .line_height
            .filter(|height| height.is_finite() && *height > 0.)
            .map(|height| scalar(height.clamp(MIN_LINE_HEIGHT, MAX_LINE_HEIGHT)))
            .transpose()?,
        ligatures: spec.ligatures,
        kerning: spec.kerning,
    };
    Ok(StyledText {
        typography,
        runs,
        max_width: match spec.max_width {
            Some(width) => Some(scalar(width)?),
            None => None,
        },
        alignment: alignment(spec.align),
        wrap: spec.wrap,
        max_lines: spec.max_lines,
    })
}

/// Whether a run must be drawn through a retained layout rather than as plain text.
///
/// The plain command carries one run, one size, one color, and nothing else: anything a paragraph
/// can express that it cannot is shaped by the host.
pub(crate) fn needs_shape(spec: &TextSpec) -> bool {
    spec.wrap
        || spec.ellipsis
        || spec.max_lines.is_some()
        || spec.runs.len() > 1
        || spec.align != TextAlign::Start
        || spec.carries_typography()
        || spec
            .runs
            .iter()
            .any(|run| run.underline || run.strikethrough)
}

pub(crate) fn alignment(align: TextAlign) -> TextAlignment {
    match align {
        TextAlign::Start => TextAlignment::Start,
        TextAlign::Center => TextAlignment::Center,
        TextAlign::End => TextAlignment::End,
        TextAlign::Justify => TextAlignment::Justify,
    }
}

/// Where a paragraph's lines sit inside the box that holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
    Justify,
}

fn scalar(value: f32) -> io::Result<Scalar> {
    Scalar::new(value as f64).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("text length rejected: {error}"),
        )
    })
}

/// The largest spacing `Typography::validate` accepts, which is also its own ceiling.
const MAX_SPACING: f32 = 1024.;
/// The smallest line height that is still a line box, and the largest the wire carries.
const MIN_LINE_HEIGHT: f32 = 0.1;
const MAX_LINE_HEIGHT: f32 = 4096.;

/// A spacing in the range the wire accepts. A negative one — tighter tracking, which CSS allows —
/// becomes none rather than a refused frame.
fn spacing(value: f32) -> io::Result<Scalar> {
    if !value.is_finite() {
        return scalar(0.);
    }
    scalar(value.clamp(0., MAX_SPACING))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vui::node::TextRunSpec;
    use crate::vui::style::rgb;

    fn plain(text: &str) -> TextSpec {
        TextSpec::plain(text, 16.)
    }

    #[test]
    fn a_plain_run_is_drawable_without_shaping() {
        assert!(!needs_shape(&plain("hello")));
        assert!(needs_shape(&TextSpec {
            wrap: true,
            ..plain("hello")
        }));
        assert!(needs_shape(&TextSpec {
            align: TextAlign::Center,
            ..plain("hello")
        }));
        assert!(needs_shape(&TextSpec {
            runs: vec![TextRunSpec::default(), TextRunSpec::default()],
            ..plain("")
        }));
        let underlined = TextSpec {
            runs: vec![TextRunSpec {
                underline: true,
                ..TextRunSpec::default()
            }],
            ..plain("")
        };
        assert!(needs_shape(&underlined));
    }

    #[test]
    fn the_cache_key_separates_runs_that_draw_differently() {
        let base = plain("hello");
        assert_eq!(MeasureKey::of(&base), MeasureKey::of(&base.clone()));

        let mut colored = base.clone();
        colored.runs[0].color = rgb(0xff0000);
        // A recolor is not a resize: the measurement stands, and the shape does not.
        assert_eq!(MeasureKey::of(&base), MeasureKey::of(&colored));
        assert_ne!(LayoutKey::of(&base), LayoutKey::of(&colored));

        let mut bigger = base.clone();
        bigger.runs[0].size = 24.;
        assert_ne!(MeasureKey::of(&base), MeasureKey::of(&bigger));

        let wrapped = TextSpec {
            wrap: true,
            max_width: Some(100.),
            ..base.clone()
        };
        assert_ne!(MeasureKey::of(&base), MeasureKey::of(&wrapped));

        let narrowed = TextSpec {
            max_width: Some(50.),
            ..wrapped.clone()
        };
        assert_ne!(MeasureKey::of(&wrapped), MeasureKey::of(&narrowed));
    }

    #[test]
    fn a_paragraph_becomes_the_wire_runs_it_was_built_from() {
        let spec = TextSpec {
            runs: vec![
                TextRunSpec {
                    text: "bold".into(),
                    weight: 700,
                    ..TextRunSpec::default()
                },
                TextRunSpec {
                    text: " plain".into(),
                    ..TextRunSpec::default()
                },
            ],
            wrap: true,
            max_width: Some(120.),
            ellipsis: true,
            max_lines: Some(2),
            align: TextAlign::Center,
            letter_spacing: 1.5,
            word_spacing: 3.,
            line_height: Some(24.),
            ..TextSpec::default()
        };
        let wire = styled(&spec).unwrap();
        assert_eq!(wire.text(), "bold plain");
        assert_eq!(wire.runs.len(), 2);
        assert_eq!(wire.runs[0].style.weight, 700);
        assert_eq!(wire.runs[1].style.weight, 400);
        assert!(wire.wrap);
        assert_eq!(wire.max_width.unwrap().get(), 120.);
        assert_eq!(wire.max_lines, Some(2));
        assert_eq!(wire.typography.overflow, TextOverflow::Ellipsis);
        assert_eq!(wire.alignment, TextAlignment::Center);

        // And the typography reached the wire rather than being dropped on the way there.
        assert_eq!(wire.typography.letter_spacing.get(), 1.5);
        assert_eq!(wire.typography.word_spacing.get(), 3.);
        assert_eq!(wire.typography.line_height.unwrap().get(), 24.);
        // The font's own features stay on unless something turned them off.
        assert!(wire.typography.ligatures);
        assert!(wire.typography.kerning);
    }

    #[test]
    fn a_spacing_the_wire_cannot_carry_is_brought_into_range_rather_than_refused() {
        // Negative tracking is real typography and the wire has no room for it, so it becomes
        // none. A spacing past the ceiling is brought down to it rather than failing the frame.
        let negative = styled(&TextSpec {
            letter_spacing: -1.5,
            ..TextSpec::default()
        })
        .unwrap();
        assert_eq!(negative.typography.letter_spacing.get(), 0.);

        let huge = styled(&TextSpec {
            word_spacing: 5000.,
            ..TextSpec::default()
        })
        .unwrap();
        assert_eq!(huge.typography.word_spacing.get(), 1024.);

        // A line height that is not one is no line height at all.
        let nowhere = styled(&TextSpec {
            line_height: Some(0.),
            ..TextSpec::default()
        })
        .unwrap();
        assert_eq!(nowhere.typography.line_height, None);
    }

    #[test]
    fn typography_is_part_of_what_is_measured_and_what_is_drawn() {
        let plain = TextSpec::plain("hello", 16.);
        assert!(
            !needs_shape(&plain),
            "a plain run is drawn as plain text, which is cheaper"
        );

        // Each of these is a request the plain text command cannot carry, so each has to go
        // through a retained layout — otherwise the view asked for something and silently did
        // not get it.
        for spec in [
            TextSpec {
                letter_spacing: 2.,
                ..plain.clone()
            },
            TextSpec {
                word_spacing: 2.,
                ..plain.clone()
            },
            TextSpec {
                line_height: Some(30.),
                ..plain.clone()
            },
            TextSpec {
                ligatures: false,
                ..plain.clone()
            },
            TextSpec {
                kerning: false,
                ..plain.clone()
            },
        ] {
            assert!(needs_shape(&spec), "{spec:?} is not plain text");
        }

        // And two paragraphs that differ only in line height are not the same measurement.
        assert_ne!(
            MeasureKey::of(&plain),
            MeasureKey::of(&TextSpec {
                line_height: Some(30.),
                ..plain.clone()
            })
        );
    }

    #[test]
    fn a_size_the_wire_cannot_carry_is_refused() {
        let spec = TextSpec {
            runs: vec![TextRunSpec {
                size: f32::NAN,
                ..TextRunSpec::default()
            }],
            ..plain("")
        };
        assert!(styled(&spec).is_err());
    }
}
