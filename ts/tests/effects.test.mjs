// Reference pixels shared with Rust and Python.
import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import {
  EffectScene,
  Ui,
  rect,
  overlay,
  vectorEffects as fx,
} from "@vivido/vlib/vui";

function artwork() {
  const backdrop = new Uint8Array(48 * 32 * 4);
  for (let i = 0; i < 48 * 32; i++)
    backdrop.set(
      [Math.floor((i % 48) / 4) % 2 === 0 ? 25 : 80, 50, 70, 255],
      4 * i,
    );
  return new EffectScene(
    [48, 32],
    [48, 32],
    0,
    [
      {
        rect: [18, 12, 10, 12],
        radius: 2,
        color: 0x708878ff,
        elevation: 14,
        emitter: true,
      },
    ],
    backdrop,
  );
}
test("all six passes match the shared reference pixels", () => {
  const light = {
    position: [7, 8],
    height: 18,
    radius: 3,
    color: [255, 204, 155],
    intensity: 1.6,
    ambient: 0.35,
  };
  const effects = [
    { kind: "lamp", light },
    { kind: "flashlight", light, aim: [26, 20], beam: 0.45 },
    { kind: "godRays", light, softness: 2 },
    { kind: "glow", color: [64, 213, 249], radius: 5, intensity: 0.9 },
    { kind: "fire", height: 14, speed: 1.4, embers: 0.8, time: 0.6, seed: 17 },
    {
      kind: "liquidGlass",
      center: [25, 17],
      size: [22, 14],
      depth: 8,
      dispersion: 0.35,
      frost: 0.15,
      wave: { center: [5, 15], age: 0.3 },
    },
  ];
  const probes = JSON.parse(
    readFileSync(
      new URL("../../tests/fixtures/effect_probes.json", import.meta.url),
      "utf8",
    ),
  );
  const s = artwork();
  assert.equal(effects.length, probes.length);
  for (const [i, e] of effects.entries()) {
    const pixels = s.render(e);
    for (const [x, y, expected] of probes[i]) {
      const k = (y * 48 + x) * 4;
      assert.ok(
        expected.every((v, c) => Math.abs(v - pixels[k + c]) <= 1),
        e.kind,
      );
    }
  }
});
test("unmount and replacement release only this window's images", async () => {
  const s = artwork();
  const window = () => ({
    held: new Set(),
    next: 0,
    async uploadRgba() {
      this.held.add(++this.next);
      return this.next;
    },
    async releaseImage(id) {
      assert.ok(this.held.delete(id));
    },
    async drawImage(_, id) {
      assert.ok(this.held.has(id));
    },
    async present() {},
  });
  const ws = [window(), window()],
    states = [{ level: 1 }, { level: 1 }];
  const uis = states.map(
    (state) =>
      new Ui(
        state,
        async (state, f) => {
          if (state.level)
            await f.effectCanvas("shared", rect(0, 0, 48, 32), s, {
              kind: "glow",
              color: [1, 2, 3],
              radius: 4,
              intensity: state.level,
            });
        },
        { width: 48, height: 32 },
      ),
  );
  for (const [i, ui] of uis.entries()) {
    await ui.attach(null, ws[i]);
    await ui.paint();
    assert.deepEqual([...ws[i].held], [1]);
  }
  states[0].level = 2;
  await uis[0].paint();
  assert.deepEqual([...ws[0].held], [2]);
  states[0].level = 0;
  await uis[0].paint();
  assert.equal(ws[0].held.size, 0);
  assert.deepEqual([...ws[1].held], [1]);
});

test("vector glass replays custom artwork and releases its clip", () => {
  const scene = { bounds: rect(480, 160, 60, 40), surfaces: [] };
  const glass = {
    lens: { rect: [510, 180, 5, 3], radius: 0 },
    depth: 8,
    dispersion: 0.2,
    frost: 0.3,
    wave: { center: [496, 171], age: 0.4 },
  };
  const triangle = new overlay.Path()
    .moveTo(480, 160)
    .lineTo(530, 188)
    .lineTo(500, 192)
    .close();
  const canvas = new overlay.Canvas();
  let replays = 0;
  fx.liquidGlass(canvas, scene, glass, (c) => {
    replays++;
    c.fill(triangle, overlay.Brush.solid(0x4b99dfff));
  });
  canvas.validate();
  assert.equal(replays, 4);
  assert.throws(
    () =>
      fx.liquidGlass(canvas, scene, glass, () => {
        throw new Error("artwork failed");
      }),
    /artwork failed/,
  );
  canvas.validate();
});
