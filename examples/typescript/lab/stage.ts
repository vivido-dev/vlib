/** Native vector artwork. All positions are in the stage's 288 by 144 design space. */
import {
  overlay,
  vectorEffects as fx,
  rect,
  type Rect,
} from "@vivido/vlib/vui";
const { Path, Brush } = overlay;
export type XY = [number, number];
type Box = [number, number, number, number];
export interface Page {
  name: string;
  hint: string;
  background: string;
  horizon: string;
  position: XY;
  controls: { label: string; values: number[]; initial: number }[];
  surfaces: [Box, number, string, number][];
  labels: [number, number, number, string, string, number][];
}
export interface Shot {
  center: XY;
  start: number;
}
type Canvas = overlay.Canvas;
type Stops = [number, number][];
const stage = rect(0, 0, 288, 144);
const tint = (rgb: number, a = 1) =>
  rgb * 256 + Math.round(255 * Math.max(0, Math.min(1, a)));
const rounded = (b: Readonly<Box>, r: number) =>
  Path.roundedRectangle(rect(...b), r);
const stops = (s: Stops) => s.map(([offset, color]) => ({ offset, color }));
const vertical = (y: number, h: number, ...s: Stops) =>
  Brush.linear({ x: 0, y }, { x: 0, y: y + h }, stops(s));
function background(c: Canvas, p: Page): void {
  c.fill(
    Path.rectangle(stage),
    vertical(
      0,
      144,
      [0, tint(parseInt(p.background, 16))],
      [1, tint(parseInt(p.horizon, 16))],
    ),
  );
}
function art(c: Canvas, p: Page, scene: fx.Scene): void {
  for (const { rect: b, radius: r, color, elevation: z } of scene.surfaces) {
    const path = rounded(b, r);
    c.fill(path, Brush.solid(color));
    if (z || (b[2] > 18 && b[3] > 10)) {
      c.fill(
        path,
        vertical(
          b[1],
          b[3],
          [0, tint(0xffffff, 0.07)],
          [0.52, tint(0xffffff, 0)],
          [1, tint(0xa1020, 0.12)],
        ),
      );
      c.stroke(path, Brush.solid(tint(0xf1efff, 0.18)), 0.35);
    }
  }
  for (const [x, y, size, label, color, weight] of p.labels)
    c.text(label, { x, y }, size, tint(parseInt(color, 16)), {
      family: "sans-serif",
      weight,
    });
}
function lettering(): overlay.Path {
  return new Path()
    .moveTo(175, 87)
    .cubicTo(181, 75, 184, 55, 196, 50)
    .cubicTo(208, 46, 202, 63, 191, 66)
    .moveTo(176, 72)
    .lineTo(198, 70)
    .moveTo(195, 84)
    .cubicTo(200, 66, 209, 49, 214, 51)
    .cubicTo(218, 56, 210, 71, 201, 76)
    .cubicTo(196, 87, 203, 89, 210, 77)
    .moveTo(215, 67)
    .cubicTo(206, 90, 214, 90, 225, 69)
    .cubicTo(215, 93, 224, 86, 230, 77)
    .moveTo(231, 67)
    .cubicTo(238, 63, 234, 85, 244, 84)
    .moveTo(247, 65)
    .lineTo(227, 87);
}
type Knobs = [number, number, number];
export function draw(
  c: Canvas,
  bounds: Rect,
  p: Page,
  index: number,
  pos: XY,
  knobs: Knobs,
  level: number,
  now: number,
  wave: Shot | undefined,
  shots: Shot[],
  scene: fx.Scene,
): void {
  const scale = bounds.width / 288,
    full = Path.rectangle(stage);
  c.save().transform(scale, 0, 0, scale, bounds.x, bounds.y).clip(full);
  background(c, p);

  const paint = (canvas: Canvas) => art(canvas, p, scene);
  switch (index) {
    case 0: {
      const [height, radius, intensity] = knobs;
      fx.lamp(c, scene, pos, { height, radius, intensity: intensity }, paint);
      break;
    }
    case 1: {
      const [beam, ambient] = knobs;
      fx.flashlight(
        c,
        scene,
        pos,
        { radius: 20 + 89 * beam, ambient, level },
        paint,
      );
      break;
    }
    case 2: {
      const [strength, softness] = knobs;
      fx.godRays(c, scene, pos, { strength, softness, level });
      paint(c);
      break;
    }
    case 3: {
      const [palette, radius, power] = knobs;
      const rgb = [0x45ded5, 0xe96da4, 0xffa950, 0x8596ff][palette]!,
        style = { radius, intensity: power * level };
      fx.glow(c, { rect: [24, 36, 127, 72], radius: 12 }, rgb, style, paint);
      fx.glowPath(c, lettering(), rgb, style);
      break;
    }
    case 4: {
      const [height, speed, embers] = knobs;
      fx.fire(c, scene.bounds, [34, 107], {
        width: 220,
        height,
        embers,
        time: now * speed,
      });
      paint(c);
      break;
    }
    case 5: {
      const [depth, dispersion, frost] = knobs;
      paint(c);
      fx.liquidGlass(
        c,
        scene,
        {
          lens: { rect: [pos[0] - 40, pos[1] - 21, 80, 42], radius: 21 },
          depth,
          dispersion,
          frost,
          ...(wave
            ? { wave: { center: wave.center, age: now - wave.start } }
            : {}),
        },
        (canvas) => {
          background(canvas, p);
          paint(canvas);
        },
      );
      break;
    }
    case 6: {
      const [reach, period, count] = knobs;
      fx.radar(c, [51, 35], {
        radius: 9,
        reach,
        phase: now / period,
        count,
        rgb: 0xff748a,
      });
      fx.pulse(
        c,
        { rect: [133, 24, 22, 22], radius: 11 },
        "breathe",
        now / 2.8,
        0x5ff4bc,
      );
      fx.pulse(
        c,
        { rect: [216, 28, 34, 14], radius: 7 },
        "heartbeat",
        now / 1.1,
        0xff426e,
      );
      paint(c);
      for (const shot of shots) {
        let at = shot.center,
          age = now - shot.start;
        if (Math.hypot(at[0] - 54, at[1] - 100) <= 12) {
          if (age < 0.45) {
            fx.ping(c, [66, 100], [155, 100], age / 0.45, 0x93c9ff);
            continue;
          }
          at = [155, 100];
          age -= 0.45;
        }
        const inbox: fx.RoundedBox | undefined =
          at[0] >= 154 && at[0] <= 248 && at[1] >= 79 && at[1] <= 124
            ? { rect: [154, 79, 94, 45], radius: 5 }
            : undefined;
        fx.ripple(c, at, (reach * 2 + 37) * 0.95, age / 0.95, 0x85beff, inbox);
      }
      break;
    }
    default: {
      const [amplitude, speed, glow] = knobs;
      const arc = { time: now * speed, amplitude, glow: glow };
      fx.charge(
        c,
        { rect: [165, 90, 84, 20], radius: 10 },
        level,
        arc,
        (canvas) => {
          paint(canvas);
          fx.electricArc(canvas, [31, 46], [112, 46], arc);
          fx.electricBorder(
            canvas,
            { rect: [151, 23, 103, 46], radius: 5 },
            arc,
          );
        },
      );
      shots.forEach(({ center, start }, i) =>
        fx.zap(
          c,
          [center[0] < 71.5 ? 31 : 112, 46],
          center,
          (now - start) / 0.42,
          { ...arc, seed: i + 9 },
        ),
      );
    }
  }
  c.restore();
}
export function sceneFor(page: Page): fx.Scene {
  return {
    bounds: stage,
    surfaces: page.surfaces.map(([rect, radius, color, elevation]) => ({
      rect,
      radius,
      color: tint(parseInt(color, 16)),
      elevation,
    })),
  };
}
