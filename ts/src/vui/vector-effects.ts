/** Native vector effects in local logical units. The caller owns the transform and clip. */
import { overlay } from "@vivido/vivid-sdk";
import type { XY, Surface, Wave } from "./effects.js";
const { Brush, Path } = overlay;

function tint(rgb: number, opacity: number = 1): number {
  return rgb * 256 + Math.round(255 * Math.max(0, Math.min(1, opacity)));
}

export function pulseRing(
  canvas: overlay.Canvas,
  [x, y]: XY,
  radius: number,
  width: number,
  rgb: number,
  opacity: number,
): void {
  if (radius <= 0 || width <= 0 || opacity <= 0) return;
  const path = Path.ellipse({
    x: x - radius,
    y: y - radius,
    width: 2 * radius,
    height: 2 * radius,
  });
  for (const [spread, power] of [
    [3, 0.1],
    [1.6, 0.24],
    [1, 0.88],
  ] as const)
    canvas.stroke(
      path,
      Brush.solid(tint(rgb, opacity * power)),
      width * spread,
    );
}

export interface Electricity {
  time: number;
  amplitude: number;
  glow: number;
  rgb?: number;
  seed?: number;
  forks?: boolean;
}

/** A forked discharge with pinned endpoints. Geometry uses the canvas's current transform. */
export function electricArc(
  canvas: overlay.Canvas,
  [x, y]: XY,
  end: XY,
  options: Electricity,
): void {
  const {
    time,
    amplitude,
    glow,
    rgb = 0x7db5ff,
    seed = 0,
    forks = true,
  } = options;
  const dx = end[0] - x,
    dy = end[1] - y,
    length = Math.hypot(dx, dy);
  if (length < 0.01 || glow <= 0) return;
  const count = Math.max(5, Math.min(48, Math.ceil(length / 2.6)));
  const path = new Path().moveTo(x, y),
    branches = new Path();
  const phase = (time % 4096) * 12 + seed * 2.7;
  for (let i = 1; i <= count; i++) {
    const t = i / count,
      envelope = Math.sin(Math.PI * t);
    const noise =
      Math.sin(i * 2.31 + phase) * 0.44 +
      Math.sin(i * 0.79 - phase * 0.63) * 0.56 +
      Math.sin(i * 4.3 - phase * 1.4) * 0.24;
    const offset = amplitude * envelope * noise;
    const px = x + dx * t - (dy / length) * offset,
      py = y + dy * t + (dx / length) * offset;
    path.lineTo(px, py);
    if (forks && i % 8 === 4 && i < count - 2) {
      const side = (i % 16 === 4 ? -1 : 1) * amplitude;
      branches
        .moveTo(px, py)
        .lineTo(
          px + (dx / count) * 1.5 - (dy / length) * side,
          py + (dy / count) * 1.5 + (dx / length) * side,
        )
        .lineTo(
          px + (dx / count) * 3 - (dy / length) * side * 1.7,
          py + (dy / count) * 3 + (dx / length) * side * 1.7,
        );
    }
  }
  for (const [width, hue, power] of [
    [5.4, rgb, 0.07],
    [2.8, rgb, 0.18],
    [1.3, rgb, 0.72],
    [0.43, 0xecf9ff, 1],
  ] as const)
    canvas.strokeStyled(path, Brush.solid(tint(hue, glow * power)), {
      width,
      cap: "round",
      join: "round",
    });
  if (forks && count >= 7)
    canvas.stroke(branches, Brush.solid(tint(rgb, glow * 0.68)), 0.38);
}

type Canvas = overlay.Canvas;
type Box = readonly [number, number, number, number];
type Paint = (canvas: Canvas) => void;
type Stops = [number, number][];
/** Effect extent and silhouettes that cast shadows or disperse light. */
export interface Scene {
  bounds: overlay.Rect;
  surfaces: readonly Surface[];
}
export interface RoundedBox {
  rect: Box;
  radius: number;
}
export interface Glass {
  lens: RoundedBox;
  depth: number;
  dispersion: number;
  frost: number;
  wave?: Wave;
}
const rect = (x: number, y: number, width: number, height: number) => ({
  x,
  y,
  width,
  height,
});
const rounded = (b: Box, r: number) => Path.roundedRectangle(rect(...b), r);
const stops = (s: Stops) => s.map(([offset, color]) => ({ offset, color }));
const vertical = (y: number, h: number, ...s: Stops) =>
  Brush.linear({ x: 0, y }, { x: 0, y: y + h }, stops(s));
const radial = ([x, y]: XY, r: number, ...s: Stops) =>
  Brush.radial({ x, y }, r, stops(s));
function shadow(
  c: Canvas,
  b: Box,
  r: number,
  [x, y]: XY,
  blur: number,
  color: number,
): void {
  c.shadow({
    rect: rect(...b),
    radii: [r, r, r, r],
    offset: { x, y },
    blur,
    color,
  });
}
function shadows(
  c: Canvas,
  scene: Scene,
  pos: XY,
  height: number,
  radius?: number,
): void {
  const [x, y] = pos;
  for (const { rect: shape, radius: r, elevation: z } of scene.surfaces)
    if (z) {
      const shift = (0.48 * z) / Math.max(height - z, 8);
      shadow(
        c,
        shape,
        r,
        [
          (shape[0] + shape[2] * 0.5 - x) * shift,
          (shape[1] + shape[3] * 0.5 - y) * shift,
        ],
        radius !== undefined
          ? 1 +
              radius * 0.65 +
              0.12 *
                Math.hypot(
                  (shape[0] + shape[2] * 0.5 - x) * shift,
                  (shape[1] + shape[3] * 0.5 - y) * shift,
                )
          : 3,
        tint(0x040814, 0.7),
      );
    }
}
/** Cast warm light and shadows, painting the foreground in the callback. */
export function lamp(
  c: Canvas,
  scene: Scene,
  pos: XY,
  options: { height: number; radius: number; intensity: number },
  paint: Paint,
): void {
  const { height, radius, intensity } = options;
  shadows(c, scene, pos, height, radius);
  paint(c);
  const full = Path.rectangle(scene.bounds);
  c.fill(
    full,
    radial(
      pos,
      height * 2 + radius * 3,
      [0, tint(0xffcd8a, 0.34 * intensity)],
      [0.5, tint(0xffbb78, 0.12 * intensity)],
      [1, tint(0x080a19, 0.44)],
    ),
  );
  c.fill(
    full,
    radial(
      pos,
      radius * 2.5,
      [0, tint(0xfffee6)],
      [0.27, tint(0xffe7ad, 0.98)],
      [0.4, tint(0xffcb7c, 0.7)],
      [1, tint(0xffbe71, 0)],
    ),
  );
}
/** Illuminate a circular beam. Ambient and level range from zero to one. */
export function flashlight(
  c: Canvas,
  scene: Scene,
  pos: XY,
  options: { radius: number; ambient: number; level: number },
  paint: Paint,
): void {
  const { radius, ambient, level } = options;
  shadows(c, scene, pos, 56);
  paint(c);
  const full = Path.rectangle(scene.bounds);
  c.fill(
    full,
    radial(
      pos,
      radius,
      [0, tint(0xffe1aa, 0.32 * level)],
      [0.45, tint(0xffdfaa, 0.15 * level)],
      [0.82, tint(0x080c1a, (1 - ambient) * 0.82)],
      [1, tint(0x080c1a, 1 - ambient)],
    ),
  );
  if (level < 1)
    c.fill(full, Brush.solid(tint(0x080c1a, (1 - level) * (1 - ambient))));
}

/** Paint projected shafts behind the foreground artwork. */
export function godRays(
  c: Canvas,
  scene: Scene,
  pos: XY,
  options: { strength: number; softness: number; level: number },
): void {
  const [x, y] = pos;
  const { strength, softness } = options;
  const level = Math.min(1, Math.max(0, options.level));
  const extent = Math.max(scene.bounds.width, scene.bounds.height);
  const full = Path.rectangle(scene.bounds);
  c.fill(
    full,
    radial(
      pos,
      extent * (235 / 288),
      [0, tint(0xffebc2, strength * 0.45 * level)],
      [0.42, tint(0xf8b78c, strength * 0.18 * level)],
      [1, tint(0xdfa1b6, 0)],
    ),
  );
  const attenuate = (opacity: number) => 1 - (1 - opacity * level) ** (1 / 9);
  const shade = radial(
    pos,
    extent * (193 / 288),
    [0, tint(0x151524, attenuate(0.9))],
    [0.55, tint(0x151524, attenuate(0.68))],
    [1, tint(0x151524, attenuate(0.24))],
  );
  for (let i = 0; i < 9; i++) {
    const angle = i * 2.399963,
      reach = (4 + softness * 2.6) * Math.sqrt(i / 8);
    const lx = x + reach * Math.cos(angle),
      ly = y + reach * Math.sin(angle);
    for (const {
      rect: [l, t, w, h],
      elevation: z,
    } of scene.surfaces) {
      if (!z || (l <= lx && lx <= l + w && t <= ly && ly <= t + h)) continue;
      const axis = Math.atan2(t + h / 2 - ly, l + w / 2 - lx),
        corners: XY[] = [
          [l, t],
          [l + w, t],
          [l + w, t + h],
          [l, t + h],
        ];
      const delta = (q: XY) =>
        (Math.atan2(q[1] - ly, q[0] - lx) - axis + 3 * Math.PI) % (2 * Math.PI);
      corners.sort((u, v) => delta(u) - delta(v));
      const u = corners[0]!,
        v = corners[3]!;
      const path = new Path()
        .moveTo(...u)
        .lineTo(...v)
        .lineTo(lx + (v[0] - lx) * 35, ly + (v[1] - ly) * 35)
        .lineTo(lx + (u[0] - lx) * 35, ly + (u[1] - ly) * 35)
        .close();
      c.fill(path, shade);
    }
  }
  c.fill(
    full,
    radial(
      pos,
      extent * (135 / 288),
      [0, tint(0xffdfac, 0.28 * level)],
      [0.18, tint(0xffc38a, 0.08 * level)],
      [0.54, tint(0x191b29, 0.48)],
      [1, tint(0x191b29, 0.9)],
    ),
  );
}
/** Surround a surface and its foreground artwork with a halo and a fine edge. */
export function glow(
  c: Canvas,
  panel: RoundedBox,
  rgb: number,
  options: { radius: number; intensity: number },
  paint: Paint,
): void {
  const { radius, intensity } = options;
  shadow(
    c,
    panel.rect,
    panel.radius,
    [0, 0],
    radius * 2.4,
    tint(rgb, 0.86 * intensity),
  );
  paint(c);
  c.stroke(
    rounded(panel.rect, panel.radius),
    Brush.solid(tint(rgb, intensity)),
    0.8,
  );
}
/** Stroke an arbitrary path as a luminous tube. */
export function glowPath(
  c: Canvas,
  tube: overlay.Path,
  rgb: number,
  options: { radius: number; intensity: number },
): void {
  const { radius, intensity } = options;
  for (const [width, opacity, hue] of [
    [radius * 1.4, 0.06 * intensity, rgb],
    [radius * 0.65, 0.16 * intensity, rgb],
    [2.6, 0.8 * intensity, rgb],
    [1.05, intensity + 0.14, 0xebfffb],
  ] as const) {
    if (width > 0) c.stroke(tube, Brush.solid(tint(hue, opacity)), width);
  }
}

const thermalRamp = [
  [107, 30, 16],
  [198, 60, 17],
  [250, 117, 24],
  [255, 181, 56],
  [255, 226, 134],
  [255, 248, 219],
] as const;
function thermalColor(heat: number): number {
  const value = Math.max(0, Math.min(1, heat)) * 5;
  const i = Math.min(Math.floor(value), 4),
    f = value - i;
  const lo = thermalRamp[i]!,
    hi = thermalRamp[i + 1]!;
  const channel = (k: 0 | 1 | 2) => Math.round(lo[k] + (hi[k] - lo[k]) * f);
  return (channel(0) << 16) | (channel(1) << 8) | channel(2);
}
function sway(z: number, phase: number, time: number): number {
  // Older parcels have travelled farther through the drifting, curling velocity field.
  const age = time - z * 1.15;
  return (
    z *
    (0.15 * Math.sin(phase + 2.1 * age) +
      0.085 * Math.sin(1.73 * phase - 3.7 * age + 5.3 * z))
  );
}
/** Draw turbulent, cooling plumes above a fuel bed. Time is in seconds. */
export function fire(
  c: Canvas,
  bounds: overlay.Rect,
  [left, base]: XY,
  options: { width: number; height: number; embers: number; time: number },
): void {
  const { width, height, embers, time } = options;
  if (width <= 0 || height <= 0) return;
  c.fill(
    Path.rectangle(bounds),
    radial(
      [left + width / 2, base],
      width * 0.64,
      [0, tint(0xff7d29, 0.21)],
      [0.56, tint(0xe34b1d, 0.06)],
      [1, tint(0xc33e16, 0)],
    ),
  );
  const unit = Path.rectangle(rect(-1, -1, 2, 2));
  // Broad smoke, orange plumes, then incandescent cores. Each soft parcel is
  // sheared along its local velocity to follow the stream without hard edges.
  const layers = [
    {
      count: 12,
      slices: 5,
      altitude: 1.32,
      spread: 1.3,
      gain: 0,
      opacity: 0.064,
    },
    {
      count: 25,
      slices: 8,
      altitude: 1.02,
      spread: 1.12,
      gain: 0.82,
      opacity: 0.68,
    },
    {
      count: 32,
      slices: 7,
      altitude: 0.78,
      spread: 1.12 * 0.78,
      gain: 1.02,
      opacity: 0.83,
    },
  ];
  c.save();
  try {
    c.clip(
      Path.rectangle(
        rect(left - width * 0.25, base - height * 2, width * 1.5, height * 2),
      ),
    );
    for (const [
      layer,
      { count, slices, altitude, spread, gain, opacity },
    ] of layers.entries()) {
      for (let i = 0; i < count; i++) {
        const seed = i * 2.399963 + layer * 1.71;
        const u = (i + 0.5 + 0.3 * Math.sin(seed * 3.7)) / count;
        const edge = Math.min(1, u * 12, (1 - u) * 12);
        const phase = u * 32 + 0.65 * Math.sin(seed) + layer * 0.34;
        for (let k = slices - 1; k >= 0; k--) {
          const z = k / slices,
            age = time - z * 0.86;
          const fuel =
            0.66 +
            0.21 * Math.sin(u * 25 + 1.45 * Math.sin(u * 9 + age) - 2.1 * age) +
            0.15 * Math.sin(u * 53 + age * 3.4) +
            0.08 * Math.cos(u * 11 - age);
          const flicker =
            0.94 +
            0.15 * Math.sin(seed + age * 3.9) +
            0.08 * Math.sin(seed * 2.7 - age * 6.7);
          const rise =
            height * fuel * flicker * (0.35 + 0.65 * edge) * altitude;
          const x = left + width * u + rise * sway(z, phase, time),
            y = base - z * rise;
          const pulse =
            0.5 +
            0.5 *
              Math.sin(
                seed + time * 4.5 - z * 10 + 0.6 * Math.sin(phase + z * 6),
              );
          const half =
            (width / count) *
            spread *
            (1 - 0.88 * z) *
            (0.86 + 0.2 * Math.sin(phase + z * 8 - age * 4)) *
            (1 - z * 0.58 * (1 - pulse));
          const length = rise * (0.24 - 0.06 * z);
          const tilt =
            (sway(z + 0.02, phase, time) - sway(z - 0.02, phase, time)) / 0.04;
          const power =
            opacity * (1 - z) ** 0.75 * (1 - z * 0.75 * (1 - pulse));
          const rgb =
            layer === 0
              ? 0x806958
              : thermalColor(
                  (1 - z) ** 1.35 * gain +
                    0.085 * Math.sin(seed + z * 12 - age * 3),
                );
          const brush = radial(
            [0, 0],
            1,
            [0, tint(rgb, power)],
            [0.35, tint(rgb, power * 0.87)],
            [0.7, tint(rgb, power * 0.34)],
            [1, tint(rgb, 0)],
          );
          c.save();
          try {
            c.transform(half, 0, length * tilt, -length, x, y).fill(
              unit,
              brush,
            );
          } finally {
            c.restore();
          }
        }
      }
    }
  } finally {
    c.restore();
  }
  if (embers <= 0) return;
  for (let i = 0; i < 22; i++) {
    const seed = i * 2.399963,
      cycles = time * (0.36 + 0.12 * Math.sin(seed)) + i * 0.618034;
    const age = cycles - Math.floor(cycles);
    const drift = Math.sin(age * 5 + seed) - Math.sin(seed);
    const x = left + (width * (i + 0.5)) / 22 + height * 0.09 * age * drift;
    const y = base - height * age * (0.8 + 0.44 * Math.abs(Math.sin(seed)));
    const alpha =
      embers *
      Math.min(1, age * 12, (1 - age) * 4) *
      (0.65 + 0.35 * Math.sin(seed) ** 2);
    const r = Math.min(width, height) * (0.004 + 0.006 * (1 - age));
    c.fill(
      Path.rectangle(rect(x - 3 * r, y - 3 * r, 6 * r, 6 * r)),
      radial(
        [x, y],
        3 * r,
        [0, tint(0xff9440, alpha * 0.35)],
        [1, tint(0xff7125, 0)],
      ),
    );
    c.fill(
      Path.ellipse(rect(x - r, y - 2 * r, 2 * r, 4 * r)),
      vertical(
        y - 2 * r,
        4 * r,
        [0, tint(0xffeb96, alpha)],
        [1, tint(0xff6027, alpha * 0.15)],
      ),
    );
  }
}
interface RimPoint {
  x: number;
  y: number;
  nx: number;
  ny: number;
}
function rimSample(shape: RoundedBox, guide: number, u: number): RimPoint {
  const [x, y, w, h] = shape.rect,
    r = Math.min(Math.max(0, shape.radius), w / 2, h / 2);
  const radius = r + guide,
    arc = (radius * Math.PI) / 2;
  let distance = u * (2 * (w + h - 4 * r) + 4 * arc);
  // Each tangent ends in a quarter circle with a continuous outward normal.
  const tangents = [
    [1, 0],
    [0, 1],
    [-1, 0],
    [0, -1],
  ] as const;
  for (const [side, [tx, ty]] of tangents.entries()) {
    const length = (side % 2 === 0 ? w : h) - 2 * r;
    const cx = side < 2 ? x + w - r : x + r,
      cy = side === 0 || side === 3 ? y + r : y + h - r;
    if (distance <= length)
      return {
        x: cx + radius * ty - (length - distance) * tx,
        y: cy - radius * tx - (length - distance) * ty,
        nx: ty,
        ny: -tx,
      };
    distance -= length;
    if (distance <= arc || side === 3) {
      const angle = distance / radius,
        nx = ty * Math.cos(angle) + tx * Math.sin(angle),
        ny = -tx * Math.cos(angle) + ty * Math.sin(angle);
      return { x: cx + radius * nx, y: cy + radius * ny, nx, ny };
    }
    distance -= arc;
  }
  throw new Error("a full lap ends on the fourth arc");
}

/** Draw a flowing violet and cyan plasma border. Width is in logical units. */
export function plasma(
  c: Canvas,
  shape: RoundedBox,
  options: { width: number; intensity: number; time: number },
): void {
  const { width, intensity, time } = options;
  const [x, y, w, h] = shape.rect;
  if (Math.min(w, h, width, intensity) <= 0) return;
  const r = Math.min(Math.max(0, shape.radius), w / 2, h / 2),
    guide = width * 0.58;
  const rim = rounded(
    [x - guide, y - guide, w + 2 * guide, h + 2 * guide],
    r + guide,
  );
  for (const [spread, opacity] of [
    [3.2, 0.035],
    [2.5, 0.055],
    [1.9, 0.08],
    [1.4, 0.11],
    [0.95, 0.14],
  ] as const) {
    c.stroke(
      rim,
      Brush.solid(tint(0x7563ec, opacity * intensity)),
      width * spread,
    );
  }
  const count = 128;
  const samples = Array.from({ length: count }, (_, k) =>
    rimSample(shape, guide, k / count),
  );
  for (let j = 0; j < 6; j++) {
    const phase = j * 2.399963;
    const points = samples.map(({ x, y, nx, ny }, k): XY => {
      const a = (2 * Math.PI * k) / count;
      const offset =
        width *
        (0.31 *
          Math.sin(3 * a + phase + time * 1.5 + 0.8 * Math.sin(2 * a - time)) +
          0.19 *
            Math.sin(
              7 * a - 2.4 * time + phase * 2 + 0.8 * Math.cos(3 * a + time),
            ));
      return [x + nx * offset, y + ny * offset];
    });
    const last = points[count - 1]!,
      first = points[0]!;
    const path = new Path().moveTo(
      (last[0] + first[0]) / 2,
      (last[1] + first[1]) / 2,
    );
    points.forEach((p, k) => {
      const q = points[(k + 1) % count]!;
      path.quadTo(...p, (p[0] + q[0]) / 2, (p[1] + q[1]) / 2);
    });
    path.close();
    c.stroke(path, Brush.solid(tint(0x8473ff, intensity * 0.19)), width * 0.41);
    c.stroke(
      path,
      Brush.solid(tint(j % 2 ? 0x9fc4ff : 0x9c8bff, intensity * 0.48)),
      width * 0.12,
    );
    for (let crest = 0; crest < 2; crest++) {
      const cycles = time * (0.067 + 0.006 * j) + j * 0.381966 + crest * 0.5;
      const start = cycles - Math.floor(cycles);
      const upper: XY[] = [],
        lower: XY[] = [];
      for (let step = 0; step <= 16; step++) {
        const pos = (start + step * 0.007) * count;
        const k = Math.floor(pos) % count,
          next = (k + 1) % count,
          f = pos - Math.floor(pos);
        const mix = (a: number, b: number) => a + (b - a) * f;
        const p = points[k]!,
          q = points[next]!;
        const px = mix(p[0], q[0]),
          py = mix(p[1], q[1]);
        const half = width * 0.125 * Math.sin((Math.PI * step) / 16) ** 2;
        const a = samples[k]!,
          b = samples[next]!,
          dx = mix(a.nx, b.nx) * half,
          dy = mix(a.ny, b.ny) * half;
        upper.push([px + dx, py + dy]);
        lower.push([px - dx, py - dy]);
      }
      const streak = new Path().moveTo(...upper[0]!);
      for (const point of upper.slice(1)) streak.lineTo(...point);
      for (let k = 15; k >= 1; k--) streak.lineTo(...lower[k]!);
      c.fill(streak.close(), Brush.solid(tint(0xcbf1ff, intensity * 0.88)));
    }
  }
}

function magnify(
  c: Canvas,
  paint: Paint,
  clip: overlay.Path,
  [x, y]: XY,
  scale: number,
): void {
  c.save();
  try {
    c.clip(clip).transform(
      scale,
      0,
      0,
      scale,
      x * (1 - scale),
      y * (1 - scale),
    );
    paint(c);
  } finally {
    c.restore();
  }
}
/** Repaint visible silhouettes through displaced color edges and a Gaussian frost. */
function glassDetail(
  c: Canvas,
  scene: Scene,
  shape: RoundedBox,
  [x, y]: XY,
  depth: number,
  dispersion: number,
  frost: number,
): void {
  const lens = shape.rect;
  const w = lens[2],
    h = lens[3];
  if (!dispersion && !frost) return;
  const scale = 1 + depth * 0.008;
  c.save();
  try {
    c.clip(rounded(lens, shape.radius));
    if (frost)
      c.fill(
        rounded(lens, shape.radius),
        Brush.solid(tint(0xe4e6df, 0.65 * frost)),
      );
    c.transform(scale, 0, 0, scale, x * (1 - scale), y * (1 - scale));
    for (const { rect: b, radius: r, color } of scene.surfaces) {
      const alpha = (color & 255) / 255;
      if (
        alpha === 0 ||
        b[0] > x + w / 2 + 5 ||
        b[0] + b[2] < x - w / 2 - 5 ||
        b[1] > y + h / 2 + 5 ||
        b[1] + b[3] < y - h / 2 - 5
      )
        continue;
      if (frost)
        shadow(
          c,
          b,
          r,
          [0, 0],
          1 + 4 * frost,
          tint(color >>> 8, 0.84 * frost * alpha),
        );
      if (dispersion)
        for (const [direction, hue] of [
          [-1, 0xe95791],
          [1, 0x4cd9ec],
        ] as const) {
          const dx =
            Math.min(w / 2, Math.max(-w / 2, b[0] + b[2] * 0.5 - x)) *
            dispersion *
            0.06 *
            direction;
          const dy =
            Math.min(h / 2, Math.max(-h / 2, b[1] + b[3] * 0.5 - y)) *
            dispersion *
            0.05 *
            direction;
          c.stroke(
            rounded([b[0] + dx, b[1] + dy, b[2], b[3]], r),
            Brush.solid(tint(hue, dispersion * 0.72 * alpha)),
            0.55 + dispersion * 0.5,
          );
        }
    }
  } finally {
    c.restore();
  }
}
/** Refract existing artwork. Replay its background and foreground in paint. */
export function liquidGlass(
  c: Canvas,
  scene: Scene,
  glass: Glass,
  paint: Paint,
): void {
  const { lens: shape, depth, dispersion, frost, wave } = glass;
  const lens = shape.rect;
  const [l, top, w, h] = lens;
  if (!(w > 0 && h > 0))
    throw new RangeError("lens dimensions must be positive");
  const radius = Math.min(shape.radius, w / 2, h / 2);
  const pos: XY = [l + w / 2, top + h / 2];
  if (wave && wave.age > 0.001 && wave.age < 1) {
    const t = wave.age,
      r = t * Math.max(scene.bounds.width, scene.bounds.height) * (225 / 288),
      [u, v] = wave.center;
    magnify(
      c,
      paint,
      rounded([u - r, v - r, 2 * r, 2 * r], r),
      wave.center,
      1 + 0.035 * (1 - t),
    );
    pulseRing(c, wave.center, r, 0.65, 0xffffff, 0.62 * (1 - t));
  }

  shadow(c, lens, radius, [0, 2.8], 5, tint(0x17243d, 0.36));
  for (const [i, factor] of [
    [0, 0.02],
    [0.85, 0.013],
    [2.4, 0.008],
  ] as const) {
    const inset = (i * Math.min(w, h, 42)) / 42;
    magnify(
      c,
      paint,
      rounded(
        [l + inset, top + inset, w - 2 * inset, h - 2 * inset],
        Math.max(0, radius - inset),
      ),
      pos,
      1 + depth * factor,
    );
  }
  glassDetail(c, scene, { rect: lens, radius }, pos, depth, dispersion, frost);
  c.fill(
    rounded(lens, radius),
    vertical(
      top,
      h,
      [0, tint(0xf4ffff, 0.33 + frost * 0.4)],
      [0.3, tint(0xfaffff, frost * 0.48)],
      [0.76, tint(0xedfaff, frost * 0.48)],
      [1, tint(0xa0d5ed, 0.42 + frost * 0.18)],
    ),
  );
  c.stroke(
    rounded(lens, radius),
    vertical(
      top,
      h,
      [0, tint(0xffffff, 0.96)],
      [0.45, tint(0xb5fbff, 0.26 + dispersion * 0.65)],
      [1, tint(0x8178d0, 0.46 + dispersion * 0.6)],
    ),
    1,
  );
}

/** Animate up to 64 repeating rings. Phase is measured in cycles. */
export function radar(
  c: Canvas,
  center: XY,
  options: {
    radius: number;
    reach: number;
    phase: number;
    count: number;
    rgb: number;
  },
): void {
  const { radius, reach, phase, rgb } = options;
  const count = Math.min(64, Math.floor(options.count));
  for (let i = 0; i < count; i++) {
    const p = phase + i / count,
      t = p - Math.floor(p);
    pulseRing(c, center, radius + t * reach, 0.66, rgb, (1 - t) ** 2);
  }
}
/** Draw the animated halo behind a surface. Phase is measured in cycles. */
export function pulse(
  c: Canvas,
  shape: RoundedBox,
  mode: "breathe" | "heartbeat",
  phase: number,
  rgb: number,
): void {
  if (mode === "breathe") {
    const b = 0.5 - 0.5 * Math.cos(phase * 2 * Math.PI);
    shadow(
      c,
      shape.rect,
      shape.radius,
      [0, 0],
      3 + 9 * b,
      tint(rgb, 0.35 + 0.6 * b),
    );
  } else {
    const beat = phase - Math.floor(phase);
    const h =
      Math.exp(-(((beat - 0.14) / 0.1) ** 2)) +
      0.55 * Math.exp(-(((beat - 0.37) / 0.08) ** 2));
    shadow(
      c,
      shape.rect,
      shape.radius,
      [0, 0],
      2 + 8 * h,
      tint(rgb, 0.28 + 0.72 * h),
    );
  }
}
/** Draw a traveling ping with its fading trail. */
export function ping(
  c: Canvas,
  start: XY,
  end: XY,
  progress: number,
  rgb: number,
): void {
  const dx = end[0] - start[0],
    dy = end[1] - start[1],
    length = Math.hypot(dx, dy);
  if (!length || !(progress >= 0 && progress <= 1)) return;
  for (let j = 6; j >= 0; j--) {
    const r = 1.8 - j * 0.15,
      t = progress - (j * 1.7) / length;
    c.fill(
      rounded([start[0] + dx * t - r, start[1] + dy * t - r, 2 * r, 2 * r], r),
      Brush.solid(tint(rgb, 1 - j / 7)),
    );
  }
}
/** Draw an expanding ripple, optionally clipped to a rounded shape. */
export function ripple(
  c: Canvas,
  center: XY,
  reach: number,
  progress: number,
  rgb: number,
  clip?: RoundedBox,
): void {
  if (!(progress >= 0 && progress < 1)) return;
  const draw = () =>
    pulseRing(
      c,
      center,
      0.1 + reach * progress,
      1.15,
      rgb,
      (1 - progress) ** 2,
    );
  if (!clip) {
    draw();
    return;
  }
  c.save();
  try {
    c.clip(rounded(clip.rect, clip.radius));
    draw();
  } finally {
    c.restore();
  }
}
/** Trace current around a closed perimeter. */
export function electricBorder(
  c: Canvas,
  shape: RoundedBox,
  arc: Electricity,
): void {
  const [x, y, w, h] = shape.rect,
    r = Math.min(shape.radius, w / 2, h / 2);
  const corners: XY[] = [
    [x + r, y],
    [x + w - r, y],
    [x + w, y + r],
    [x + w, y + h - r],
    [x + w - r, y + h],
    [x + r, y + h],
    [x, y + h - r],
    [x, y + r],
  ];
  corners.forEach((corner, i) =>
    electricArc(c, corner, corners[(i + 1) % 8]!, {
      ...arc,
      amplitude: arc.amplitude * 0.45,
      glow: arc.glow * 0.72,
      seed: (arc.seed ?? 0) + i,
      forks: false,
    }),
  );
}
/** Surround a control with a halo and a discharge along its upper edge. */
export function charge(
  c: Canvas,
  shape: RoundedBox,
  level: number,
  arc: Electricity,
  paint: Paint,
): void {
  const [x, y, w, h] = shape.rect,
    r = Math.min(shape.radius, w / 2, h / 2);
  if (level > 0.001)
    shadow(c, shape.rect, r, [0, 0], 8, tint(0x5fceff, level * 0.9));
  paint(c);
  if (level > 0.001) {
    c.stroke(rounded(shape.rect, r), Brush.solid(tint(0xc2f5ff, level)), 0.7);
    electricArc(c, [x + r, y], [x + w - r, y], {
      ...arc,
      amplitude: arc.amplitude * 0.65,
      glow: arc.glow * level,
    });
  }
}
/** Draw a fading discharge between two endpoints. */
export function zap(
  c: Canvas,
  start: XY,
  end: XY,
  progress: number,
  arc: Electricity,
): void {
  if (progress >= 0 && progress < 1)
    electricArc(c, start, end, {
      ...arc,
      amplitude: arc.amplitude * 1.6,
      glow: arc.glow * (1 - progress),
    });
}
