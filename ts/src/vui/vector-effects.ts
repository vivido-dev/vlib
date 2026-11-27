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

/** Paint animated flame contours and embers above a horizontal fuel bed. */
export function fire(
  c: Canvas,
  bounds: overlay.Rect,
  [left, base]: XY,
  options: { width: number; height: number; embers: number; time: number },
): void {
  const { width, height: flameHeight, embers, time } = options;
  const full = Path.rectangle(bounds);
  c.fill(
    full,
    radial(
      [left + width / 2, base - 3],
      width * (114 / 220),
      [0, tint(0xf55b24, 0.38)],
      [1, tint(0xa03728, 0)],
    ),
  );
  // Nested heat contours curl sideways with altitude; their roots stay attached.
  const plume: [number, number, number][] = [];
  for (let j = 0; j <= 128; j++) {
    const u = j / 128,
      xx = left + width * u,
      wind = Math.sin(xx * 0.092 - time * 1.8);
    const swirl = xx * 0.36 + 1.5 * wind + time * 1.9;
    const noise =
      0.52 +
      0.3 * Math.sin(swirl) +
      0.16 * Math.sin(swirl * 1.94 - time * 3.3) +
      0.1 * Math.sin(xx * 0.91 + time * 4.2);
    const envelope = Math.max(0, Math.sin(Math.PI * u)) ** 0.24;
    plume.push([
      xx,
      flameHeight *
        (0.16 + Math.min(1, Math.max(0, noise)) ** 1.4 * 0.85) *
        envelope,
      wind,
    ]);
  }
  for (let band = 0; band < 24; band++) {
    const heat = band / 23,
      altitude = (1 - heat * 0.94) ** 1.45;
    const path = new Path().moveTo(left, base);
    for (const [xx, height, wind] of plume) {
      const rise = height * altitude;
      const sway =
        rise *
        (0.13 * Math.sin(rise * 0.15 - time * 2.1 + xx * 0.09) + 0.08 * wind);
      path.lineTo(xx + sway, base - rise);
    }
    const red = Math.floor(174 + 81 * Math.min(1, heat / 0.42));
    const green = Math.floor(35 + 220 * heat ** 0.9),
      blue = Math.floor(25 + 195 * heat ** 3);
    const color = (red << 16) | (green << 8) | blue;
    c.fill(
      path
        .lineTo(left + width, base + 2)
        .lineTo(left, base + 2)
        .close(),
      Brush.solid(tint(color, 0.1 + 0.69 * heat)),
    );
  }
  for (let j = 0; j < 22; j++) {
    const t = (time * (0.32 + Math.sin(j * 0.76) * 0.12) + j * 0.618) % 1,
      px = left + width * ((5 + j * 9.8) / 220) + Math.sin(t * 5 + j) * 3,
      py =
        base - 5 - t * flameHeight * (0.8 + 0.6 * Math.abs(Math.sin(j * 2.3))),
      r = 0.35 + 0.42 * (1 - t);
    c.fill(
      rounded([px, py, r * 1.4, r * 2.7], r * 0.7),
      Brush.solid(tint(0xffd38a, embers * (1 - t))),
    );
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
