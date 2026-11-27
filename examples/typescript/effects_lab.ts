// Interactive vector effects for a Vivid overlay.
import { readFileSync } from "node:fs";
import { Ui, rect, runMain, type Frame } from "@vivido/vlib/vui";
import { draw, sceneFor, type Page, type Shot, type XY } from "./lab/stage.js";
const names = ["Mint", "Rose", "Amber", "Blue"];
const charged = ([x, y]: XY) => x >= 165 && x <= 249 && y >= 90 && y <= 110;

class Lab {
  readonly pages: Page[] = JSON.parse(
    readFileSync(
      new URL("../../examples/assets/effects_lab.json", import.meta.url),
      "utf8",
    ),
  );
  readonly scenes = this.pages.map(sceneFor);
  shots: Shot[] = [];
  readonly indices = this.pages.map((p) => p.controls.map((k) => k.initial));
  selected = 0;
  position: XY = this.pages[0]!.position;
  grab?: XY;
  tap?: XY;
  wave?: { center: XY; start: number };
  start = 1;
  target = 1;
  changed = 0;
  fireTimeOffset = 0;
  hovered = false;
  select(i: number): void {
    this.selected = i;
    this.position = this.pages[i]!.position;
    this.grab = undefined;
    this.tap = undefined;
    this.wave = undefined;
    this.shots = [];
    this.start = this.target = i === 7 ? 0 : 1;
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
    else if (this.selected === 6 || (this.selected === 7 && !charged(p))) {
      this.shots = [...this.shots.slice(-7), { center: p, start: now }];
    } else if (
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
  const columns = f.width >= 720 ? 8 : f.width >= 354 ? 4 : 2;
  const rows = Math.ceil(s.pages.length / columns);
  for (const [i, p] of s.pages.entries()) {
    if (i % columns === 0) x = 16;
    const width = 19 + p.name.length * 7;
    await f.button(
      `scene-${i}`,
      rect(x, 16 + 39 * Math.floor(i / columns), width, 30),
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
    Math.min((f.width - 32) / 2, f.height - (152 + 39 * (rows - 1))),
  );
  const stage = rect(16, 55 + 39 * (rows - 1), height * 2, height);
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
  if (s.selected === 7 && !hovered) s.fade(false, f.elapsed);
  const now = f.elapsed,
    wave = s.wave && now - s.wave.start < 1 ? s.wave : undefined;
  s.shots = s.shots.filter(
    (sh) => now - sh.start < (s.selected === 6 ? 1.4 : 0.42),
  );
  const knobs: [number, number, number] = [
    s.value(0),
    s.value(1),
    page.controls.length > 2 ? s.value(2) : 0,
  ];
  draw(
    f.canvas,
    stage,
    page,
    s.selected,
    s.position,
    knobs,
    s.level(now),
    s.selected === 4 ? now * knobs[1] + s.fireTimeOffset : now,
    wave,
    s.shots,
    s.scenes[s.selected]!,
  );
  if (
    (s.start !== s.target && now - s.changed < 0.35) ||
    (s.selected === 4 && knobs[1] > 0) ||
    s.selected >= 6 ||
    wave
  )
    f.requestFrameAfter(1 / 30);
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
        if (s.selected === 4 && i === 1)
          s.fireTimeOffset += f.ui.elapsed() * (v - s.value(i));
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
    ui = new Ui(s, render, { width: 760, height: 510, title: "Effects lab" });
  ui.onMouseMove = (e) => {
    const b = ui.boundsOf("stage");
    if (!b) return;
    let x = ((e.position.x - b.x) * 288) / b.width,
      y = ((e.position.y - b.y) * 144) / b.height;
    if (s.selected === 7) {
      s.fade(charged([x, y]), ui.elapsed());
      ui.notify();
    }
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
