//! The image element.
//!
//! An image is a box with pixels in it. Where the pixels come from is [`ImageSource`]; how they
//! sit in the box is [`ObjectFit`]; and when there are several of them, which one is showing is
//! the window's clock.

use crate::vui::assets::{ImageSource, ObjectFit};
use crate::vui::element::IntoElement;
use crate::vui::node::{ElementId, Node};

/// What an image element draws.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ImageSpec {
    pub source: ImageSource,
    pub fit: ObjectFit,
    /// `0..=1`, multiplied into whatever the pixels already carry.
    pub opacity: f32,
    /// Play an animated source rather than holding its first frame.
    pub playing: bool,
}

/// An image: encoded bytes, an SVG, or pixels somebody already has.
///
/// Sized by its own content unless the box says otherwise, so `img(..)` is as big as the picture
/// and `img(..).size(64., 64.)` is as big as it was told.
pub fn img(source: ImageSource) -> ImgEl {
    ImgEl {
        node: Node::image(ImageSpec {
            source,
            fit: ObjectFit::default(),
            opacity: 1.,
            playing: true,
        }),
    }
}

/// An SVG, rasterized at whatever size it ends up being drawn.
///
/// A vector has no size of its own worth honouring here — it is drawn at the size it is given,
/// so one is worth giving it.
pub fn svg(key: impl Into<String>, bytes: impl Into<Vec<u8>>) -> ImgEl {
    img(ImageSource::svg(key, bytes))
}

#[derive(Debug)]
pub struct ImgEl {
    node: Node,
}

impl ImgEl {
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.node.id = Some(id.into());
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        self.node.layout.width = crate::vui::layout::Length::px(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.node.layout.height = crate::vui::layout::Length::px(height);
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.node.layout.width = crate::vui::layout::Length::px(width);
        self.node.layout.height = crate::vui::layout::Length::px(height);
        self
    }

    pub fn w_full(mut self) -> Self {
        self.node.layout.width = crate::vui::layout::Length::full();
        self
    }

    pub fn h_full(mut self) -> Self {
        self.node.layout.height = crate::vui::layout::Length::full();
        self
    }

    pub fn flex_1(mut self) -> Self {
        self.node.layout.flex_grow = 1.;
        self.node.layout.flex_shrink = 1.;
        self.node.layout.flex_basis = crate::vui::layout::Length::px(0.);
        self
    }

    /// Keep the proportions and fit inside the box. The default.
    pub fn contain(mut self) -> Self {
        self.node.image_mut().fit = ObjectFit::Contain;
        self
    }

    /// Keep the proportions and cover the box, overflowing it.
    pub fn cover(mut self) -> Self {
        self.node.image_mut().fit = ObjectFit::Cover;
        self
    }

    /// Stretch to the box.
    pub fn fill(mut self) -> Self {
        self.node.image_mut().fit = ObjectFit::Fill;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.node.image_mut().opacity = opacity.clamp(0., 1.);
        self
    }

    /// Hold an animated source on its first frame instead of playing it.
    pub fn paused(mut self) -> Self {
        self.node.image_mut().playing = false;
        self
    }

    /// Clip the image to the box, which is what `cover` usually wants.
    pub fn clipped(mut self) -> Self {
        self.node.layout.overflow = crate::vui::layout::OverflowSpec::Hidden;
        self
    }

    pub fn rounded(mut self, radius: f32) -> Self {
        self.node.style.rounded = crate::vui::geometry::Corners::uniform(radius);
        // A rounded image that was not clipped would be a square image with rounded nothing.
        self.node.layout.overflow = crate::vui::layout::OverflowSpec::Hidden;
        self
    }

    pub fn bg(mut self, color: vivid_protocol::vector::Color) -> Self {
        self.node.style.background = Some(crate::vui::style::Background::Solid(color));
        self
    }

    pub fn border(mut self, width: f32, color: vivid_protocol::vector::Color) -> Self {
        self.node.style.border = Some(crate::vui::style::Border::solid(
            crate::vui::geometry::Pixels::new(width),
            color,
        ));
        self
    }

    pub fn cursor(mut self, cursor: vivid_protocol::vector::CursorShape) -> Self {
        self.node.interactions.set_cursor(Some(cursor));
        self
    }

    pub fn on_click(mut self, handler: crate::vui::interactive::Handler) -> Self {
        self.node.interactions.handlers.click = Some(handler);
        self
    }

    pub fn hover(
        mut self,
        style: impl Fn(crate::vui::style::Style) -> crate::vui::style::Style + 'static,
    ) -> Self {
        self.node.interactions.on_hover_style(style);
        self
    }
}

impl crate::vui::a11y::Semantic for ImgEl {
    fn accessibility_mut(&mut self) -> &mut crate::vui::a11y::Accessibility {
        &mut self.node.accessibility
    }
}

impl IntoElement for ImgEl {
    fn into_node(self) -> Node {
        self.node
    }
}
