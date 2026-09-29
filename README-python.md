# vlib.vui for Python

A UI layer over Vivid Protocol 1.5 pane overlays: state that renders itself into one display list
per frame, with the host doing the hit testing.

See the [toolkit's README](README.md) for what it is and the examples it ships, and the SDK's
[pane overlays](../vivid_sdk/docs/OVERLAYS.md) for the layer underneath.

`vlib.vui` is pure Python over `vivid-sdk`'s bindings — no native code and no third-party
dependencies of its own. Python 3.9+ and a working `vivid-sdk` are required.

## What it is, and what it is not

This is the Python counterpart of the `vlib::vui` Rust module, and it is deliberately the smaller
half. It carries the toolkit's **contract**:

- The host does the hit testing. A frame paints hit regions; events come back naming one.
- A press is active exactly while it is held over the element it started on. A release anywhere
  ends it, and a click is a release over the element the press began on.
- Leaving a pressed element cancels the press, because a release over nothing is never reported.
- The wheel goes to whatever is hovered; keys reach the focused element before the window.
- Nothing repaints until something asks, and anything that moves is a function of the window's
  elapsed clock — a wait is a deadline the loop sleeps to, never a timer.

It does **not** carry the machinery:

- **No layout engine.** The Rust crate uses taffy for flex and grid; here every box is arithmetic
  the caller does. `Frame` hands you the geometry it was given and you divide it up.
- **No image decoding.** The Rust crate decodes PNG, JPEG, GIF and WebP and rasterizes SVG; here
  `Frame.upload` takes the RGBA bytes you produced, because the protocol carries raw pixels and
  the standard library has no decoder.

Neither is a gap waiting to be filled behind your back. If you want flex, compute it; if you want
a PNG on screen, decode it before you upload it.

## A window

```python
from vlib.vui import Frame, Rect, Ui, duration


class Counter:
    count = 0


def render(state: Counter, f: Frame) -> None:
    f.box(Rect(0, 0, f.width, f.height), bg=0x1E1E2EFF)
    f.label(f"Clicked {state.count} times", Rect(20, 20, 380, 28), size=24, vcenter=False)
    f.button(
        "increment",
        Rect(20, 60, 120, 32),
        "Increment",
        on_click=lambda _event: (setattr(state, "count", state.count + 1), f.notify()),
    )


Ui(Counter(), render, width=420, height=240).run(duration())
```

`render` runs whenever something asks for a frame. `f.button` paints a box, registers a hit
region, and returns the element's live state, so hover and press restyle it without any state of
your own. `f.notify()` says the view changed; nothing repaints otherwise.

## The pieces

| Name | What it is |
|---|---|
| `Ui` | A window, the host behind it, and the loop connecting them |
| `Frame` | What one paint draws through: regions, boxes, text, pictures, semantics |
| `ElementState` | Hover, press, focus, and whether the keyboard put the focus there |
| `ListState` | The rows a box can show, and no others |
| `TextField` | An editable line: caret, selection, and an input method's composition |
| `Easing` `Running` `Spring` `SpringState` | Curves and physics, on the window's clock |
| `inset` `fit_rect` `gradient_brush` `shadow_spec` | The arithmetic a view repeats |
| `Keys` `Mods` `platform()` | HID usages, modifier bits, and the command key |

`Rect`, `Point`, `Path`, `Brush`, `Canvas`, the event types and the rest of the SDK's overlay
vocabulary are re-exported unchanged, so a view imports from one place.

## Running the examples

Every example in [`examples/python`](examples/python) is the twin of a Rust one beside it, and of
a TypeScript one. They need a Vivid-enabled Vivido pane:

```sh
uv run python examples/python/hello_world.py --duration 10
```

Run this from `vlib/`, with `vivid_sdk/` checked out beside it. The uv configuration
uses that local SDK as an editable dependency; the first run builds its native bindings,
which requires Rust. Omit `--duration 10` to keep the example open.

`uv run` resolves the project dependencies even if you previously installed them with
`uv pip install`. The local source in `pyproject.toml` tells it where to find `vivid-sdk`.

**Escape or `q` closes any of them.** `input` and `tab_stop` spend `q` on typing, as a window with
a field in it must; escape still closes those.

## Verification

```sh
uv run --extra test pytest
uv run --extra test mypy --platform linux
```

The tests drive the contracts above against dry-run sessions, which is the same code path a live
host drives. What they cannot check is what a pane does with a frame.
