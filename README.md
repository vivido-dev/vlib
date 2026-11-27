# vlib

Libraries over Vivid 1.5, one namespace each. Today that is `vlib::vui` — `vlib.vui` in Python,
`@vivido/vlib/vui` in TypeScript — a declarative UI toolkit over Vivid 1.5 pane overlays: state
that renders itself, laid out by taffy and painted into one display list per frame.

It exists so a pane overlay can be written the way a UI is written —

```rust
impl Render for Counter {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_col()
            .gap(12.)
            .p(20.)
            .bg(rgb(0x1e1e2e))
            .child(text(format!("Clicked {} times", self.count)).size(24.).color(Colors::WHITE))
            .child(
                div()
                    .id("increment")
                    .px(14.)
                    .py(8.)
                    .rounded(6.)
                    .bg(rgb(0x4a5fd0))
                    .cursor(CursorShape::Pointer)
                    .hover(|style| style.bg(rgb(0x5a6fe0)))
                    .on_click(cx.listener(|state: &mut Self, _event, cx| {
                        state.count += 1;
                        cx.notify();
                    }))
                    .child(text("Increment").color(Colors::WHITE)),
            )
    }
}
```

— rather than as a display list, without owning the window. The host still owns the window, the
hit testing, the pointer shape, and the frame budget.

## Running an example

Against a live Vivido, using the same environment as any producer:

```sh
cargo run --example hello_world
```

A window asks for focus when it opens, and **escape or `q` ends the loop** — a floating window has
no dismissal of its own. A window where something takes typed text spends `q` on typing, as
`input` and `tab_stop` do; escape still ends those.

| Example | What it shows |
|---|---|
| `hello_world` | Text and a button, and the smallest app skeleton there is |
| `tree` | A box inside a box, 64 levels deep |
| `scrollable` | Content taller than its box, scrolled by the wheel |
| `opacity` | A subtree painted as one translucent group |
| `shadow` | The shadow token scale, plus offsets, spread and inset |
| `gradient` | Linear fills at several angles and stop sets |
| `grid_layout` | A page laid out on grid tracks |
| `pattern` | A tiled background next to a gradient |
| `active_state_bug` | A button that must not stay pressed when the pointer leaves it |
| `text_layout` | Alignment, weights, slant, and decorations |
| `text_wrapper` | Wrapping, ellipsis, and a line ceiling |
| `input` | A text field: typing, selection, clipboard, and composition |
| `tab_stop` | Tab and shift-tab moving focus between fields |
| `focus_visible` | A focus ring the keyboard shows and a click does not |
| `popover` | A floating panel anchored to the button that opened it |
| `anchor` | Panels placed against each corner of their parent |
| `drag_drop` | Tiles dragged onto a tray, with pointer capture |
| `window_shadow` | A frame whose edges ask the host to resize |
| `mouse_pressure` | Marks as wide as the device was pressed |
| `uniform_list` | Ten thousand rows, of which a dozen exist |
| `list_example` | A log pinned to its newest line, with a scrollbar |
| `data_table` | A table with a fixed header and virtualized rows |
| `image` | One picture, fitted into its box four ways |
| `svg` | One document, rasterized at each size it is drawn |
| `gif_viewer` | An animation playing at its own frame rate |
| `image_gallery` | A grid of pictures against a bounded upload budget |
| `effects_lab` | Drag a lamp or lens, tune fire and neon, send a pulse, or strike an arc |
| `animation` | Easings side by side, and a spring that follows clicks |
| `a11y` | A panel that describes itself: roles, values, states, actions |
| `painting` | Freeform paths: polygons, curves, an even-odd hole, dashes, and freehand drawing |
| `paths_bench` | A thousand star paths in one frame, against the frame's three ceilings |
| `text` | A type specimen: a size ramp, weights, families, and the typography the wire carries |
| `image_loading` | A picture that arrives, and one whose bytes are not a picture |
| `window_movable` | A drag-to-move title bar beside the resize edges |

## The same examples in Python and TypeScript

Each language has its own guide: [Python](README-python.md), [TypeScript](README-typescript.md).

Every Rust example above also exists in Python and TypeScript, against the toolkit's
counterparts in those languages — `vlib.vui` in the `vlib` Python package, `@vivido/vlib/vui` on npm.
The same name means the same UI in all three.

Those two are deliberately smaller than the Rust crate. They carry the contract — the host
does the hit testing, a click is a release on the element the press started on, leaving a
pressed element cancels it, the repaint loop with its wake deadlines, list arithmetic,
easings and springs, a text field — but **no layout engine and no image decoding**. There is
no taffy and no `image` behind them, so geometry is the caller's arithmetic and pixels are
the caller's to produce. What they are is the contract without the machinery.

Run them inside a Vivid-enabled pane:

```sh
python examples/python/hello_world.py [--duration 10]

npm run build && npm run build:examples   # once: the package's dist/, and the examples
node target/examples-typescript/hello_world.js [--duration 10]

# or run an example from its TypeScript source, once the package is built
bun run examples/typescript/hello_world.ts [--duration 10]
```

**Escape or `q` closes any of them**, and `--duration` bounds the loop for an unattended run.
These windows are floating, and the protocol dismisses only a *popup* on escape or an outside
press, so a floating example has to give itself a way out. `input` and `tab_stop` spend `q` on
typing, as any window with a field in it must; escape still closes them.

| Example | What it shows |
|---|---|
| `hello_world` | A counter, a button, and the smallest app skeleton |
| `active_state_bug` | A press active exactly while held over its element |
| `focus_visible` | A focus ring the keyboard shows and a click does not |
| `anchor` | Panels placed against each corner of their parent |
| `gradient` | Linear fills at several angles and stop sets |
| `shadow` | The shadow token scale, plus offsets, spread and inset |
| `tree` | A box inside a box, 64 levels deep |
| `pattern` | A tiled background next to a gradient |
| `image` | One picture, fitted into its box four ways |
| `paths_bench` | A thousand star paths against the frame's ceilings |
| `window_shadow` | A frame whose edges ask the host to resize |
| `window_movable` | A title bar that moves the window |
| `grid_layout` | A page laid out on grid tracks |
| `opacity` | A subtree painted as one translucent group |
| `popover` | A floating panel anchored to the button that opened it |
| `drag_drop` | Tiles dragged onto a tray, with pointer capture |
| `mouse_pressure` | Marks as wide as the device was pressed |
| `painting` | Freeform paths: fills, curves, an even-odd hole, dashes, freehand |
| `scrollable` | Content taller than its box, scrolled by the wheel |
| `uniform_list` | Ten thousand rows, of which a dozen exist |
| `list_example` | A log pinned to its newest line, with a scrollbar |
| `data_table` | A table with a fixed header and virtualized rows |
| `image_gallery` | A grid of pictures against a bounded upload budget |
| `effects_lab` | Eight interactive effects drawn at display resolution |
| `animation` | Easings side by side, and a spring that follows clicks |
| `gif_viewer` | An animation playing at its own frame rate |
| `image_loading` | A picture that arrives, and one that never will |
| `text_layout` | Alignment, weights, slant, and decorations |
| `text` | A type specimen, and the typography the wire carries |
| `text_wrapper` | Wrapping, ellipsis, and a line ceiling |
| `input` | A text field: typing, selection, clipboard, and composition |
| `tab_stop` | Tab and shift-tab moving focus between fields |
| `a11y` | A panel that describes itself: roles, values, states, actions |

Deliberate differences from the Rust originals, all for one reason — no new dependencies:

- **`svg` has no port.** Its whole lesson is rasterizing one vector document at several
  device sizes through a real rasterizer; neither stdlib Python nor Node has one.
- **The media examples generate their pixels.** The Rust `image`, `image_gallery`,
  `gif_viewer`, and `image_loading` decode PNG/GIF through the `image` crate; the ports
  compute the same checkerboards, gradients, and animation frames directly as RGBA and
  upload them. The SDK lesson — upload, fit, cache, clock — is unchanged.
- **The gallery owns its own upload budget.** The Rust toolkit has a cache that releases
  whatever was drawn least recently, so its view never thinks about it; the other two have
  no such cache, so `image_gallery` evicts explicitly there. That is the honest version of
  the lesson — someone has to decide what is no longer on screen.
- **The a11y example publishes semantics only live.** An offline session refuses the
  semantics call, which the toolkit treats like any best-effort host request.

The tests drive the toolkit's contracts — press, click, hover-cancel, focus routing, list
arithmetic, easings, springs, gradients, the text field — against dry-run and offline
sessions, which is the same code path a live host drives. What they cannot check is what a
pane does with a frame, and that stays the manual acceptance the Rust examples carry.

## Effects canvas

Run `cargo run --release --example effects_lab` in a Vivido pane. The eight scenes use native vector paths, gradients, shadows, and text. Each redraw scales the artwork to the current stage, including on HiDPI displays. Select a tab and use its parameter buttons to change the effect.

In **Pulse**, click the stage for a ripple or press the blue arrow to send a ping across to the inbox. **Electricity** has a branching arc between two terminals, current around a card, a button that charges on hover, and a short discharge to the point you click.

Applications can draw the same effects through `vlib::vui::effects::vector`. Use `lamp`, `flashlight`, and `god_rays` with a `Scene` containing your logical bounds and surfaces. Use `glow` for a panel or `glow_path` for lettering, and place `fire` along a baseline. For `liquid_glass`, supply a lens and a callback that replays the background and foreground beneath it. Keep hit regions and input handling outside that callback.

The animation passes take time from your application. Combine `radar`, `pulse`, `ping`, and `ripple` for Pulse, or `electric_arc`, `electric_border`, `charge`, and `zap` for Electricity. Coordinates and stroke widths follow the canvas transform. For example, this draws an animated fire bed across the bottom of a canvas element.

```rust
use vlib::vui::{canvas, effects::vector::{self, Rect}};

let flames = canvas(move |drawing, bounds| {
    drawing.layer(|c| {
        let x = f64::from(bounds.origin.x.0);
        let y = f64::from(bounds.origin.y.0);
        let w = f64::from(bounds.width().0);
        let h = f64::from(bounds.height().0);
        vector::fire(c, Rect::new(x, y, w, h)?, [x, y + h - 2.], w, h * 0.7, 0.8, elapsed)
    });
});
```

The view supplies `elapsed` in seconds and requests frames while the fire is moving. [Gallery composition](examples/views/lab_stage.rs)

For software rendering over an RGBA backdrop or emission mask, build an `EffectScene` once and keep it in the view. Pass its shared handle and current parameters to `vlib::vui::effects::effect_canvas`. The returned element accepts ordinary size and pointer handlers. Geometry and light positions use logical canvas units. The raster can contain up to 65,532 pixels within the SDK's overlay record ceiling. Hosts can negotiate smaller limits.

For example, this glow follows the rounded shape marked as an emitter. Keep the resulting `scene` in application state.

```rust
use std::rc::Rc;
use vlib::vui::{Color, effects::{Effect, EffectScene, Surface, effect_canvas}};

let badge = Surface {
    rect: [16., 16., 64., 32.], radius: 8., color: Color(0x263444ff),
    elevation: 0, emitter: true,
};
let scene = Rc::new(EffectScene::new(
    [96., 64.], [192, 128], Color(0x141920ff), None, &[badge], None,
)?);
let glow = Effect::Glow { color: [69, 222, 213], radius: 6., intensity: 0.9 };
let badge_element = effect_canvas("badge", scene.clone(), glow);
```

The scene also exposes `render` for offscreen use. Fire takes elapsed seconds and a seed. A glass wave takes its center and age in seconds. Schedule redraws while these values change. [Rust gallery source](examples/views/effects_lab.rs)

## Testing a UI without a GPU

`testing` gives a `TestUi` that drives the same loop the application does against the SDK's
in-process presenter, so a test asserts on the display list the host received and on what the view
did with the input:

```rust
let mut ui = TestUi::start(Counter::default(), 420., 240.).unwrap();
ui.click("increment").unwrap();
assert_eq!(ui.read(|view| view.count), 1);
```

The clock is the test's too, so anything that moves can be watched without waiting for it:

```rust
ui.advance(Duration::from_millis(80)).unwrap();
```

`tests/harness.rs` is the worked demonstration: layout, the display list, input, the clock, what
the application says about itself, and the budgets the host enforces.

## Status

Part C of the overlay work, complete except for live-pane acceptance: geometry, style, layout,
elements, the application loop, input routing, focus, key bindings, text shaping, text input,
floating layers, drag and drop, virtualized lists, images and SVG, animation, accessibility,
freeform drawing, and the headless harness.

Every example is driven end to end against the SDK's in-process presenter. The effects lab has also been exercised in a live Vivido pane.
