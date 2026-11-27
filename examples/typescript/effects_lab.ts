// A six-scene interactive lab for the portable effect canvas.
import { readFileSync } from "node:fs";
import {
  EffectScene,
  Ui,
  rect,
  runMain,
  type Frame,
  type Effect,
  type Light,
  type Surface,
} from "@vivido/vlib/vui";

type XY = [number, number];
interface Page {
  name: string;
  hint: string;
  background: string;
  position: XY;
  controls: { label: string; values: number[]; initial: number }[];
  surfaces: [Surface["rect"], number, string, number, boolean][];
  mask?: { origin: XY; scale: number; rows: string[] };
}
const colors = [
  [69, 222, 213],
  [233, 109, 164],
  [255, 169, 80],
  [133, 150, 255],
] as const;
const names = ["Mint", "Rose", "Amber", "Blue"];
const rgba = (hex: string) => parseInt(hex + "ff", 16);

function scene(page: Page): EffectScene {
  const shapes = page.surfaces.map(
    ([rect, radius, color, elevation, emitter]) => ({
      rect,
      radius,
      color: rgba(color),
      elevation,
      emitter,
    }),
  );
  let mask: Uint8Array | undefined;
  if (page.mask) {
    const {
      origin: [ox, oy],
      rows,
      scale,
    } = page.mask;
    mask = new Uint8Array(288 * 144);
    for (let i = 0; i < mask.length; i++) {
      const x = Math.floor(((i % 288) - ox) / scale),
        y = Math.floor((Math.floor(i / 288) - oy) / scale);
      if (rows[y]?.[x] === "1") mask[i] = 255;
    }
  }
  return new EffectScene(
    [288, 144],
    [288, 144],
    rgba(page.background),
    shapes,
    undefined,
    mask,
  );
}

class Lab {
  readonly pages: Page[] = JSON.parse(
    readFileSync(
      new URL("../../examples/assets/effects_lab.json", import.meta.url),
      "utf8",
    ),
  );
  readonly art = this.pages.map(scene);
  readonly indices = this.pages.map((p) => p.controls.map((k) => k.initial));
  selected = 0;
  position: XY = this.pages[0]!.position;
  grab?: XY;
  tap?: XY;
  wave?: { center: XY; start: number };
  start = 1;
  target = 1;
  changed = 0;
  hovered = false;
  select(i: number): void {
    this.selected = i;
    this.position = this.pages[i]!.position;
    this.grab = undefined;
    this.tap = undefined;
    this.wave = undefined;
    this.start = this.target = 1;
    this.changed = 0;
    this.hovered = false;
  }
  value(i: number): number {
    return this.pages[this.selected]!.controls[i]!.values[
      this.indices[this.selected]![i]!
    ]!;
  }
  level(now: number): number {
    const t = Math.max(0, Math.min(1, (now - this.changed) / 0.35));
    return this.start + (this.target - this.start) * t * t * (3 - 2 * t);
  }
  fade(on: boolean, now: number): void {
    if (this.target !== Number(on)) {
      this.start = this.level(now);
      this.target = Number(on);
      this.changed = now;
    }
  }
  params(now: number): [Effect, boolean] {
    const a = this.value(0),
      b = this.value(1),
      [x, y] = this.position,
      level = this.level(now);
    const moving = this.start !== this.target && now - this.changed < 0.35;
    const light: Light = {
      position: [x, y],
      height: 32,
      radius: 7,
      color: [255, 206, 152],
      intensity: 1.5,
      ambient: 0.48,
    };
    switch (this.selected) {
      case 0:
        return [
          {
            kind: "lamp",
            light: { ...light, height: a, radius: b, intensity: this.value(2) },
          },
          false,
        ];
      case 1:
        return [
          {
            kind: "flashlight",
            aim: [x, y],
            beam: a,
            light: {
              ...light,
              position: [x + 33, y + 44],
              height: 56,
              intensity: 2.6 * level,
              ambient: b,
            },
          },
          moving,
        ];
      case 2:
        return [
          {
            kind: "godRays",
            light: {
              ...light,
              height: 35,
              radius: 25,
              intensity: a * level,
              ambient: 0.33,
            },
            softness: b,
          },
          moving,
        ];
      case 3:
        return [
          {
            kind: "glow",
            color: colors[a]!,
            radius: b,
            intensity: this.value(2) * level,
          },
          moving,
        ];
      case 4:
        return [
          {
            kind: "fire",
            height: a,
            speed: b,
            embers: this.value(2),
            time: b ? now : 0,
            seed: 19,
          },
          b > 0,
        ];
      default: {
        const wave =
          this.wave && now - this.wave.start < 1
            ? { center: this.wave.center, age: now - this.wave.start }
            : undefined;
        return [
          {
            kind: "liquidGlass",
            center: [x, y],
            size: [80, 42],
            depth: a,
            dispersion: b,
            frost: this.value(2),
            wave,
          },
          !!wave,
        ];
      }
    }
  }
  press(p: XY): void {
    this.tap = p;
    const dx = p[0] - this.position[0],
      dy = p[1] - this.position[1];
    if (
      (this.selected === 0 &&
        Math.hypot(dx, dy) <= Math.max(this.value(1), 13)) ||
      (this.selected === 5 &&
        Math.hypot(Math.max(0, Math.abs(dx) - 19), dy) <= 21)
    )
      this.grab = [dx, dy];
  }
  click(now: number): void {
    const p = this.tap;
    this.tap = undefined;
    if (!p) return;
    if (this.selected === 5) this.wave = { center: p, start: now };
    else if (
      this.selected === 3 &&
      p[0] >= 24 &&
      p[0] <= 151 &&
      p[1] >= 36 &&
      p[1] <= 108
    )
      this.fade(!this.target, now);
  }
}

async function render(s: Lab, f: Frame): Promise<void> {
  f.box(rect(0, 0, f.width, f.height), { bg: 0x18202aff });
  const page = s.pages[s.selected]!;
  let x = 16;
  const compact = f.width < 520;
  for (const [i, p] of s.pages.entries()) {
    if (compact && i === 3) x = 16;
    const width = 19 + p.name.length * 7;
    await f.button(
      `scene-${i}`,
      rect(x, compact && i >= 3 ? 55 : 16, width, 30),
      p.name,
      {
        base: i === s.selected ? 0x31565cff : 0x28313bff,
        onClick: () => {
          if (s.grab) f.ui.capture(false);
          s.select(i);
          f.notify();
        },
      },
    );
    x += width + 5;
  }
  const height = Math.max(
    1,
    Math.min((f.width - 32) / 2, f.height - (compact ? 191 : 152)),
  );
  const stage = rect(16, compact ? 94 : 55, height * 2, height);
  const hovered = f.region("stage", stage, {
    onMouseDown: (e) => {
      s.press([
        ((e.position.x - stage.x) * 288) / stage.width,
        ((e.position.y - stage.y) * 144) / stage.height,
      ]);
      if (s.grab) f.ui.capture();
      f.notify();
    },
    onClick: () => {
      s.click(f.ui.elapsed());
      f.notify();
    },
  }).hovered;
  if ((s.selected === 1 || s.selected === 2) && hovered !== s.hovered)
    s.fade(hovered, f.elapsed);
  s.hovered = hovered;
  const [effect, moving] = s.params(f.elapsed);
  await f.effectCanvas("stage", stage, s.art[s.selected]!, effect);
  if (moving) f.requestFrameAfter(0.056);
  const y = stage.y + stage.height + 9;
  await f.label(page.hint, rect(16, y, f.width - 32, 20), {
    size: 13,
    color: 0xc6cdd3ff,
  });
  x = 16;
  for (const [i, k] of page.controls.entries()) {
    const v = s.value(i),
      label = `${k.label}  ${s.selected === 3 && i === 0 ? names[v] : v}  ›`;
    const width = 18 + label.length * 7;
    await f.button(`knob-${i}`, rect(x, y + 29, width, 30), label, {
      base: 0x28313bff,
      onClick: () => {
        const indices = s.indices[s.selected]!;
        indices[i] = (indices[i]! + 1) % k.values.length;
        f.notify();
      },
    });
    x += width + 6;
  }
  const grip = rect(
    Math.max(0, f.width - 20),
    Math.max(0, f.height - 20),
    18,
    18,
  );
  await f.label("◢", grip, { size: 13, color: 0x849499ff });
  f.resizeRegion("resize", grip, 12, "resize-down-right");
}

export function makeUi(): Ui<Lab> {
  const s = new Lab(),
    ui = new Ui(s, render, { width: 624, height: 442, title: "Effects lab" });
  ui.onMouseMove = (e) => {
    const b = ui.boundsOf("stage");
    if (!b) return;
    let x = ((e.position.x - b.x) * 288) / b.width,
      y = ((e.position.y - b.y) * 144) / b.height;
    if (s.tap && Math.hypot(x - s.tap[0], y - s.tap[1]) > 2) s.tap = undefined;
    if (s.grab) {
      x -= s.grab[0];
      y -= s.grab[1];
    } else if (
      (s.selected !== 1 && s.selected !== 2) ||
      x < 0 ||
      x >= 288 ||
      y < 0 ||
      y >= 144
    )
      return;
    s.position = [Math.max(0, Math.min(288, x)), Math.max(0, Math.min(144, y))];
    ui.notify();
  };
  ui.onMouseUp = () => {
    if (s.grab) {
      s.grab = undefined;
      ui.capture(false);
      ui.notify();
    }
  };
  return ui;
}

await runMain(makeUi, import.meta.url);
