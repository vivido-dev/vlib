# @vivido/vlib/vui for TypeScript

A UI layer over Vivid Protocol 1.5 pane overlays: state that renders itself into one display list
per frame, with the host doing the hit testing.

See the [toolkit's README](README.md) for what it is and the examples it ships, and the SDK's
[pane overlays](../vivid_sdk/docs/OVERLAYS.md) for the layer underneath.

`@vivido/vlib/vui` is pure TypeScript over `@vivido/vivid-sdk` — no native code and no
dependencies of its own. Node 20+ is required; it runs under Bun too.

## What it is, and what it is not

This is the TypeScript counterpart of the `vlib::vui` Rust module, and it is deliberately the
smaller half. It carries the toolkit's **contract**:

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
  Node's standard library has no decoder.

Neither is a gap waiting to be filled behind your back. If you want flex, compute it; if you want
a PNG on screen, decode it before you upload it.

## A window

```ts
import { rect, runMain, Ui, type Frame } from "@vivido/vlib/vui";

class Counter {
  count = 0;
}

async function render(state: Counter, f: Frame): Promise<void> {
  f.box(rect(0, 0, f.width, f.height), { bg: 0x1e1e2eff });
  await f.label(`Clicked ${state.count} times`, rect(20, 20, 380, 28), { size: 24, vcenter: false });
  await f.button("increment", rect(20, 60, 120, 32), "Increment", {
    onClick: () => {
      state.count += 1;
      f.notify();
    },
  });
}

export function makeUi(): Ui<Counter> {
  return new Ui(new Counter(), render, { width: 420, height: 240 });
}

await runMain(makeUi, import.meta.url);
```

`render` runs whenever something asks for a frame. Measuring, drawing text and uploading pictures
are round trips to the host, so those are `async` and must be awaited before the frame is
submitted; building boxes and regions is synchronous. `runMain` starts the window only when this
file is the one the runtime was pointed at, so a test can import `makeUi` without opening
anything — hand it `import.meta.url`, which is lexically scoped and cannot be read on your behalf.

## The pieces

| Name | What it is |
|---|---|
| `Ui` | A window, the host behind it, and the loop connecting them |
| `Frame` | What one paint draws through: regions, boxes, text, pictures, semantics |
| `ElementState` | Hover, press, focus, and whether the keyboard put the focus there |
| `ListState` | The rows a box can show, and no others |
| `TextField` | An editable line: caret, selection, and an input method's composition |
| `Easing` `Running` `Spring` `SpringState` | Curves and physics, on the window's clock |
| `inset` `fitRect` `gradientBrush` `rect` | The arithmetic a view repeats |
| `Keys` `Mods` `platform()` | HID usages, modifier bits, and the command key |

The SDK's `overlay` namespace and its types — `Rect`, `Point`, `Path`, `Brush`, `StyledText` and
the rest — are re-exported unchanged, so a view imports from one place.

Region ids and scene revisions are `bigint`, as the wire carries them; text offsets are UTF-16
code units, native to JavaScript strings and to the binding's own cluster geometry.

## Running the examples

Every example in [`examples/typescript`](examples/typescript) is the twin of a Rust one beside it,
and of a Python one. They need a Vivid-enabled Vivido pane, and they need the package built once:
an example imports `@vivido/vlib/vui`, which resolves to this package's `dist/vui/`, and `dist/` is not
checked in. The scripts below are the same under `npm run` and `bun run`.

```sh
npm install                            # once, for the file: dependency on ../vivid_sdk
npm run build                          # once, for dist/ — what the examples import
npm run build:examples
node target/examples-typescript/hello_world.js [--duration 10]

# or run one from its TypeScript source, which skips only the transpile
bun run examples/typescript/hello_world.ts [--duration 10]
```

The `../vivid_sdk` dependency has its own build — `dist/` and the native addon — and this package
typechecks and runs against it.

**Escape or `q` closes any of them.** `input` and `tab_stop` spend `q` on typing, as a window with
a field in it must; escape still closes those.

## Verification

```sh
npm test
npm run typecheck && npm run typecheck:examples
```

The tests drive the contracts above against offline sessions, which is the same code path a live
host drives. What they cannot check is what a pane does with a frame.
