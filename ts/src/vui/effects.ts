// Software effects over a producer-owned straight-alpha RGBA canvas.
export type XY = readonly [number, number];
export type RGB = readonly [number, number, number];
export interface Surface {
  rect: readonly [number, number, number, number];
  radius: number;
  color: number;
  elevation?: number;
  emitter?: boolean;
}
export interface Light {
  position: XY;
  height: number;
  radius: number;
  color: RGB;
  intensity: number;
  ambient: number;
}
export interface Wave {
  center: XY;
  age: number;
}
export type Effect =
  | { kind: "lamp"; light: Light }
  | { kind: "flashlight"; light: Light; aim: XY; beam: number }
  | { kind: "godRays"; light: Light; softness: number }
  | { kind: "glow"; color: RGB; radius: number; intensity: number }
  | {
      kind: "fire";
      height: number;
      speed: number;
      embers: number;
      time: number;
      seed: number;
    }
  | {
      kind: "liquidGlass";
      center: XY;
      size: XY;
      depth: number;
      dispersion: number;
      frost: number;
      wave?: Wave;
    };

function check(ok: boolean): void {
  if (!ok)
    throw new RangeError(
      "invalid effect canvas geometry, artwork or parameters",
    );
}
const within = (v: number, low: number, high: number) => v >= low && v <= high;
const point = (p: XY) =>
  p.length === 2 && p.every((v) => within(v, -32768, 32768));
const rgb = (c: RGB) =>
  c.length === 3 && c.every((v) => Number.isInteger(v) && within(v, 0, 255));
const clamp = (v: number, lo = 0, hi = 1) => Math.min(hi, Math.max(lo, v));
const fract = (v: number) => v - Math.floor(v);

function over(out: Uint8Array, k: number, rgb: RGB, opacity: number): void {
  const a = clamp(opacity);
  if (!a) return;
  const tail = (out[k + 3]! / 255) * (1 - a);
  for (let c = 0; c < 3; c++)
    out[k + c] = Math.round((rgb[c]! * a + out[k + c]! * tail) / (a + tail));
  out[k + 3] = Math.round(255 * (a + tail));
}

/** Prepared artwork, height field and emission distances. Logical size is independent of raster size. */
export class EffectScene {
  readonly width: number;
  readonly height: number;
  private readonly sx: number;
  private readonly sy: number;
  private readonly art: Uint8Array;
  private readonly heights: Uint8Array;
  private readonly heightsSum: Uint32Array;
  private readonly emission: Uint8Array;
  private readonly rise: Float64Array;

  constructor(
    logical: XY,
    raster: XY,
    background: number,
    surfaces: readonly Surface[] = [],
    backdrop?: Uint8Array,
    mask?: Uint8Array,
  ) {
    const [w, h] = raster;
    const n = w * h;
    check(
      raster.every((v) => Number.isInteger(v) && v > 0) &&
        n <= (256 * 1024 - 16) / 4 &&
        logical.length === 2 &&
        logical.every((v) => within(v, 0.001, 16384)) &&
        surfaces.length <= 256 &&
        Number.isInteger(background) &&
        within(background, 0, 0xffffffff) &&
        (!backdrop || backdrop.length === 4 * n) &&
        (!mask || mask.length === n),
    );
    this.width = w;
    this.height = h;
    this.sx = logical[0] / w;
    this.sy = logical[1] / h;
    this.art = new Uint8Array(n * 4);
    if (backdrop) this.art.set(backdrop);
    else {
      const pixels = new DataView(this.art.buffer);
      for (let i = 0; i < n; i++) pixels.setUint32(i * 4, background);
    }
    this.heights = new Uint8Array(n);
    this.heightsSum = new Uint32Array((w + 1) * (h + 1));
    this.emission = mask ? Uint8Array.from(mask) : new Uint8Array(n);
    this.rise = new Float64Array(n);
    for (const s of surfaces) this.stamp(s);
    this.prepare();
  }

  private stamp(s: Surface): void {
    const [left, top, width, height] = s.rect;
    const elevation = s.elevation ?? 0;
    check(
      s.rect.every((v) => within(v, -32768, 32768)) &&
        width >= 0 &&
        height >= 0 &&
        within(s.radius, 0, 32768) &&
        Number.isInteger(elevation) &&
        within(elevation, 0, 255) &&
        Number.isInteger(s.color) &&
        within(s.color, 0, 0xffffffff),
    );
    if (width === 0 || height === 0) return;
    const r = Math.min(s.radius, width * 0.5, height * 0.5);
    const color: RGB = [
      s.color >>> 24,
      (s.color >>> 16) & 255,
      (s.color >>> 8) & 255,
    ];
    const alpha = s.color & 255;
    for (
      let y = Math.max(0, Math.floor(top / this.sy));
      y < Math.min(this.height, Math.ceil((top + height) / this.sy));
      y++
    ) {
      const dy = Math.max(
        0,
        Math.abs((y + 0.5) * this.sy - top - height * 0.5) - height * 0.5 + r,
      );
      for (
        let x = Math.max(0, Math.floor(left / this.sx));
        x < Math.min(this.width, Math.ceil((left + width) / this.sx));
        x++
      ) {
        const dx = Math.max(
          0,
          Math.abs((x + 0.5) * this.sx - left - width * 0.5) - width * 0.5 + r,
        );
        if (dx * dx + dy * dy > r * r) continue;
        const i = y * this.width + x;
        over(this.art, 4 * i, color, alpha / 255);
        if (alpha) this.heights[i] = Math.max(this.heights[i]!, elevation);
        if (s.emitter) this.emission[i] = Math.max(this.emission[i]!, alpha);
      }
    }
  }

  private prepare(): void {
    const { width: w, height: h, sy, emission: m, rise } = this;
    for (let y = 0; y < h; y++) {
      let row = 0;
      for (let x = 0; x < w; x++) {
        row += this.heights[y * w + x]!;
        this.heightsSum[(y + 1) * (w + 1) + x + 1] =
          this.heightsSum[y * (w + 1) + x + 1]! + row;
      }
    }
    for (let i = w * h - 1; i >= 0; i--)
      rise[i] = m[i] ? 0 : i + w < w * h ? rise[i + w]! + sy : Infinity;
  }

  private bloom(radius: number): Float64Array {
    const { width: w, height: h } = this;
    const f = Float64Array.from(this.emission, (a) => a / 255);
    for (const [lines, length, stride, spacing, pitch] of [
      [h, w, 1, this.sx, w],
      [w, h, w, this.sy, 1],
    ] as const) {
      const decay = Math.exp(-spacing / radius);
      for (let line = 0; line < lines; line++) {
        let v = 0;
        const start = line * pitch;
        for (let j = 0; j < length; j++) {
          const i = start + j * stride;
          v = f[i]! + decay * (v - f[i]!);
          f[i] = v;
        }
        v /= 1 + decay;
        for (let j = length - 1; j >= 0; j--) {
          const i = start + j * stride;
          v = f[i]! + decay * (v - f[i]!);
          f[i] = v;
        }
      }
    }
    return f;
  }

  private index(x: number, y: number): number {
    return (
      clamp(Math.floor(y / this.sy), 0, this.height - 1) * this.width +
      clamp(Math.floor(x / this.sx), 0, this.width - 1)
    );
  }

  /** Render one deterministic straight-alpha sRGB frame. */
  render(e: Effect): Uint8Array {
    const out = this.art.slice();
    if (e.kind === "lamp" || e.kind === "godRays" || e.kind === "flashlight") {
      this.light(out, e);
    } else if (e.kind === "glow") {
      check(
        rgb(e.color) &&
          within(e.radius, 0.001, 32768) &&
          within(e.intensity, 0, 16),
      );
      const halo = this.bloom(e.radius),
        core = this.bloom(e.radius * 0.2);
      for (let i = 0; i < this.emission.length; i++) {
        const source = this.emission[i]! / 255,
          edge = Math.abs(source - core[i]!);
        over(
          out,
          4 * i,
          e.color,
          (edge * 1.9 + Math.max(0, halo[i]! - 0.75 * source) * 1.8) *
            e.intensity,
        );
        over(out, 4 * i, [250, 250, 255], edge * 0.95 * e.intensity);
      }
    } else if (e.kind === "fire") {
      check(
        within(e.height, 0.001, 32768) &&
          within(e.speed, 0, 16) &&
          within(e.embers, 0, 1) &&
          Number.isFinite(e.time) &&
          e.time >= 0 &&
          Number.isInteger(e.seed) &&
          within(e.seed, 0, 0xffffffff),
      );
      const t = (e.time % 65536) * e.speed,
        phase = (e.seed % 65521) * 0.063;
      for (let i = 0; i < this.rise.length; i++) {
        const rise = this.rise[i]!;
        if (rise <= 0 || rise > e.height * 1.8) continue;
        const u = (((i % this.width) + 0.5) * this.sx) / e.height;
        const n =
          Math.sin(u * 13 + t * 3 + phase + Math.sin(u * 7 - t * 2)) * 0.5 +
          Math.sin(u * 28 - t * 4 + phase + (rise / e.height) * 2) * 0.3 +
          Math.sin(u * 51 + t * 5) * 0.2;
        const heat = clamp(1 - rise / (e.height * (0.62 + n * 0.4)));
        over(
          out,
          4 * i,
          [255, Math.round(40 + 206 * heat), Math.round(155 * heat ** 3)],
          heat * 3,
        );
        const ember = Math.sin(Math.floor(u * 5) * 73.7 + phase) * 19.1;
        const altitude = (fract(t * 0.52 + ember) * 1.65 + 0.1) * e.height;
        const lateral =
          ((fract(u * 5) - 0.5 - 0.28 * Math.sin(t * 2 + ember)) * e.height) /
          5;
        const spark = Math.max(
          0,
          1 -
            Math.hypot(lateral, (rise - altitude) * 0.7) /
              (1 + e.height * 0.025),
        );
        over(out, 4 * i, [255, 199, 92], e.embers * spark);
      }
    } else this.glass(out, e);
    return out;
  }

  private light(out: Uint8Array, e: Extract<Effect, { light: Light }>): void {
    const l = e.light,
      [lx, ly] = l.position;
    check(
      point(l.position) &&
        rgb(l.color) &&
        within(l.height, 0.001, 32768) &&
        within(l.radius, 0.001, 32768) &&
        within(l.ambient, 0, 1) &&
        within(l.intensity, 0, 16),
    );
    let ax = 0,
      ay = 0,
      axis = 1,
      edge = 0;
    if (e.kind === "flashlight") {
      check(point(e.aim) && within(e.beam, 0.01, 1.56));
      ax = e.aim[0] - lx;
      ay = e.aim[1] - ly;
      axis = Math.hypot(ax, ay, l.height);
      edge = Math.cos(e.beam);
    }
    if (e.kind === "godRays") check(within(e.softness, 0.1, 8));
    const reach2 = (2 * l.height + l.radius * 3) ** 2;
    for (let i = 0; i < this.heights.length; i++) {
      const x = ((i % this.width) + 0.5) * this.sx,
        y = (Math.floor(i / this.width) + 0.5) * this.sy;
      const dx = lx - x,
        dy = ly - y,
        z = this.heights[i]!,
        rr = dx * dx + dy * dy;
      const fall = 1 / (1 + rr / reach2);
      let beam = 1,
        shadow = 0,
        open = 0;
      if (e.kind === "flashlight") {
        const dz = z - l.height;
        const cosine =
          (-dx * ax - dy * ay - dz * l.height) /
          Math.max(1e-6, Math.sqrt(rr + dz * dz) * axis);
        beam = clamp(((cosine - edge) / (1 - edge)) * 1.5);
        beam = beam * beam * (3 - 2 * beam);
      }
      for (let j = 0; j < 12; j++) {
        const t = (j + 0.5) / 12,
          px = x + dx * t,
          py = y + dy * t;
        if (
          px < 0 ||
          py < 0 ||
          px >= this.width * this.sx ||
          py >= this.height * this.sy
        ) {
          open++;
          continue;
        }
        const r =
          e.kind === "godRays"
            ? e.softness * (0.6 + t * 1.8)
            : 0.4 + t * l.radius * 0.65;
        const x0 = clamp(Math.floor((px - r) / this.sx), 0, this.width - 1),
          x1 = clamp(Math.ceil((px + r) / this.sx), x0 + 1, this.width);
        const y0 = clamp(Math.floor((py - r) / this.sy), 0, this.height - 1),
          y1 = clamp(Math.ceil((py + r) / this.sy), y0 + 1, this.height);
        const a = this.heightsSum,
          stride = this.width + 1;
        const h =
          (a[y1 * stride + x1]! +
            a[y0 * stride + x0]! -
            a[y0 * stride + x1]! -
            a[y1 * stride + x0]!) /
          ((x1 - x0) * (y1 - y0));
        if (e.kind === "godRays") open += 1 / (1 + h / e.softness);
        else
          shadow = Math.max(
            shadow,
            clamp((h - (z + (l.height - z) * t)) / (1 + l.radius * t)),
          );
      }
      const power =
        fall *
        l.intensity *
        (e.kind === "godRays"
          ? (open / 12) ** 2 * (z > 0 ? 0.12 : 1)
          : beam * (1 - 0.88 * shadow));
      for (let c = 0; c < 3; c++) {
        const k = 4 * i + c,
          base = out[k]!;
        out[k] = Math.round(
          clamp(
            base * l.ambient +
              (((base * 0.86 + 22) * l.color[c]!) / 255) * power,
            0,
            255,
          ),
        );
      }
      if (e.kind === "lamp")
        over(
          out,
          4 * i,
          l.color,
          Math.exp(-rr / (l.radius * l.radius * 0.35)) * l.intensity,
        );
    }
  }

  private glass(
    out: Uint8Array,
    e: Extract<Effect, { kind: "liquidGlass" }>,
  ): void {
    check(
      point(e.center) &&
        e.size.every((v) => within(v, 0.001, 32768)) &&
        within(e.depth, 0, 256) &&
        within(e.dispersion, 0, 1) &&
        within(e.frost, 0, 1) &&
        (!e.wave ||
          (point(e.wave.center) &&
            Number.isFinite(e.wave.age) &&
            e.wave.age >= 0)),
    );
    const r = Math.min(...e.size) * 0.5,
      bx = e.size[0] * 0.5 - r,
      by = e.size[1] * 0.5 - r;
    for (let i = 0; i < this.heights.length; i++) {
      const x = ((i % this.width) + 0.5) * this.sx,
        y = (Math.floor(i / this.width) + 0.5) * this.sy;
      const dx = x - e.center[0],
        dy = y - e.center[1];
      const nx = dx - clamp(dx, -bx, bx),
        ny = dy - clamp(dy, -by, by),
        norm = Math.hypot(nx, ny);
      const inside = norm <= r;
      let px = x,
        py = y;
      if (e.wave && e.wave.age < 1) {
        const vx = x - e.wave.center[0],
          vy = y - e.wave.center[1],
          d = Math.max(0.001, Math.hypot(vx, vy));
        const band = (d - e.wave.age * this.width * this.sx * 0.85) / 5;
        const push = band * Math.exp(-band * band * 0.5) * 7 * (1 - e.wave.age);
        px += (vx / d) * push;
        py += (vy / d) * push;
      }
      if (!inside && px === x && py === y) continue;
      const rim = inside ? (norm / r) ** 4 : 0,
        bend = (e.depth * rim) / Math.max(0.001, norm);
      px -= nx * bend;
      py -= ny * bend;
      const mid = 4 * this.index(px, py);
      out.set(this.art.subarray(mid, mid + 4), 4 * i);
      if (!inside) continue;
      for (let c = 0; c < 3; c++) {
        const split = (c - 1) * e.dispersion * bend,
          u = px + nx * split,
          v = py + ny * split;
        const sample = 4 * this.index(u, v);
        const base = this.art[mid + c]!;
        const coverage = Math.min(
          1,
          this.art[sample + 3]! / Math.max(1, this.art[mid + 3]!),
        );
        let value = base + (this.art[sample + c]! - base) * coverage;
        if (e.frost) {
          const span = 1 + e.frost * 4;
          let total = 0,
            weight = 0;
          for (const j of [
            this.index(u - span, v),
            this.index(u + span, v),
            this.index(u, v - span),
            this.index(u, v + span),
          ]) {
            const a = this.art[4 * j + 3]!;
            total += this.art[4 * j + c]! * a;
            weight += a;
          }
          if (weight > 0) value += (total / weight - value) * e.frost;
        }
        out[4 * i + c] = Math.round(value);
      }
      const highlight = clamp(((-nx - ny) / Math.max(0.001, norm)) * 0.4 + 0.5);
      over(
        out,
        4 * i,
        [225, 245, 255],
        rim ** 3 * (0.18 + highlight * 0.65) + e.frost * 0.08,
      );
    }
  }
}
