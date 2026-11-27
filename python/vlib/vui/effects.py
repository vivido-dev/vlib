"""Bounded software effects over producer-supplied, straight-alpha RGBA artwork."""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import List, Optional, Sequence, Tuple, Union

XY = Tuple[float, float]
RGB = Tuple[int, int, int]
Box = Tuple[float, float, float, float]
MAX_PIXELS = (256 * 1024 - 16) // 4


@dataclass(frozen=True)
class Surface:
    rect: Box
    radius: float
    color: int
    elevation: int = 0
    emitter: bool = False


@dataclass(frozen=True)
class Light:
    position: XY
    height: float
    radius: float
    color: RGB
    intensity: float
    ambient: float


@dataclass(frozen=True)
class Lamp:
    light: Light


@dataclass(frozen=True)
class Flashlight:
    light: Light
    aim: XY
    beam: float


@dataclass(frozen=True)
class GodRays:
    light: Light
    softness: float


@dataclass(frozen=True)
class Glow:
    color: RGB
    radius: float
    intensity: float


@dataclass(frozen=True)
class Fire:
    height: float
    speed: float
    embers: float
    time: float
    seed: int


@dataclass(frozen=True)
class Wave:
    center: XY
    age: float


@dataclass(frozen=True)
class LiquidGlass:
    center: XY
    size: XY
    depth: float
    dispersion: float
    frost: float
    wave: Optional[Wave] = None


Effect = Union[Lamp, Flashlight, GodRays, Glow, Fire, LiquidGlass]


def _check(ok: bool) -> None:
    if not ok:
        raise ValueError("invalid effect canvas geometry, artwork or parameters")


def _point(p: XY) -> bool:
    return len(p) == 2 and all(abs(v) <= 32768 for v in p)


def _rgb(c: RGB) -> bool:
    return len(c) == 3 and all(isinstance(v, int) and 0 <= v <= 255 for v in c)


def _over(out: bytearray, k: int, rgb: RGB, a: float) -> None:
    if a <= 0:
        return
    a = min(1.0, a)
    tail = out[k + 3] / 255 * (1 - a)
    alpha = a + tail
    for c in range(3):
        out[k + c] = int((rgb[c] * a + out[k + c] * tail) / alpha + 0.5)
    out[k + 3] = int(255 * alpha + 0.5)


class EffectScene:
    """Static artwork and lighting fields. All positions and lengths use logical units."""

    def __init__(
        self,
        logical: XY,
        raster: Tuple[int, int],
        background: int,
        surfaces: Sequence[Surface] = (),
        *,
        backdrop: Optional[bytes] = None,
        mask: Optional[bytes] = None,
    ) -> None:
        w, h = raster
        n = w * h
        _check(
            all(isinstance(v, int) and v > 0 for v in raster)
            and n <= MAX_PIXELS
            and len(logical) == 2
            and all(0.001 <= v <= 16384 for v in logical)
            and 0 <= background <= 0xFFFFFFFF
            and len(surfaces) <= 256
            and (backdrop is None or len(backdrop) == n * 4)
            and (mask is None or len(mask) == n)
        )
        self._raster = raster
        self._logical = logical
        self._sx, self._sy = logical[0] / w, logical[1] / h
        self._xs = [(x + 0.5) * self._sx for x in range(w)]
        self._ys = [(y + 0.5) * self._sy for y in range(h)]
        self._art = bytearray(
            backdrop if backdrop is not None else background.to_bytes(4, "big") * n
        )
        self._heights = bytearray(n)
        self._emission = bytearray(mask) if mask is not None else bytearray(n)
        for s in surfaces:
            left, top, width, height = s.rect
            _check(
                all(abs(v) <= 32768 for v in s.rect)
                and min(width, height) >= 0
                and 0 <= s.radius <= 32768
                and 0 <= s.elevation <= 255
                and 0 <= s.color <= 0xFFFFFFFF
            )
            if width == 0 or height == 0:
                continue
            radius = min(s.radius, width * 0.5, height * 0.5)
            color = s.color.to_bytes(4, "big")
            for y in range(
                max(0, math.floor(top / self._sy)),
                min(h, math.ceil((top + height) / self._sy)),
            ):
                dy = max(
                    0.0, abs(self._ys[y] - top - height * 0.5) - height * 0.5 + radius
                )
                for x in range(
                    max(0, math.floor(left / self._sx)),
                    min(w, math.ceil((left + width) / self._sx)),
                ):
                    dx = max(
                        0.0,
                        abs(self._xs[x] - left - width * 0.5) - width * 0.5 + radius,
                    )
                    if dx * dx + dy * dy > radius * radius:
                        continue
                    i = y * w + x
                    _over(
                        self._art, 4 * i, (color[0], color[1], color[2]), color[3] / 255
                    )
                    if color[3]:
                        self._heights[i] = max(self._heights[i], s.elevation)
                    if s.emitter:
                        self._emission[i] = max(self._emission[i], color[3])
        self._rise = [math.inf] * n
        self._prepare()

    @property
    def raster(self) -> Tuple[int, int]:
        return self._raster

    def _prepare(self) -> None:
        w, h = self._raster
        sx, sy = self._sx, self._sy
        self._heights_sum = [0] * ((w + 1) * (h + 1))
        for y in range(h):
            row = 0
            for x in range(w):
                row += self._heights[y * w + x]
                self._heights_sum[(y + 1) * (w + 1) + x + 1] = (
                    self._heights_sum[y * (w + 1) + x + 1] + row
                )
        for i in reversed(range(w * h)):
            if self._emission[i]:
                self._rise[i] = 0.0
            elif i + w < w * h:
                self._rise[i] = self._rise[i + w] + sy

    def _bloom(self, radius: float) -> List[float]:
        w, h = self._raster
        f = [a / 255 for a in self._emission]
        for lines, length, stride, spacing, pitch in (
            (h, w, 1, self._sx, w),
            (w, h, w, self._sy, 1),
        ):
            decay = math.exp(-spacing / radius)
            for line in range(lines):
                start = line * pitch
                indices = range(start, start + length * stride, stride)
                v = 0.0
                for i in indices:
                    v = f[i] + decay * (v - f[i])
                    f[i] = v
                v /= 1 + decay
                for i in reversed(indices):
                    v = f[i] + decay * (v - f[i])
                    f[i] = v
        return f

    def render(self, effect: Effect) -> bytes:
        """Render one deterministic frame and return straight-alpha sRGB RGBA8."""
        out = bytearray(self._art)
        if isinstance(effect, (Lamp, Flashlight, GodRays)):
            self._light(out, effect)
        elif isinstance(effect, Glow):
            _check(
                _rgb(effect.color)
                and 0.001 <= effect.radius <= 32768
                and 0 <= effect.intensity <= 16
            )
            halo = self._bloom(effect.radius)
            core = self._bloom(effect.radius * 0.20)
            for i, a in enumerate(self._emission):
                source = a / 255
                edge = abs(source - core[i])
                _over(
                    out,
                    4 * i,
                    effect.color,
                    (edge * 1.9 + max(0.0, halo[i] - 0.75 * source) * 1.8)
                    * effect.intensity,
                )
                _over(out, 4 * i, (250, 250, 255), edge * 0.95 * effect.intensity)
        elif isinstance(effect, Fire):
            self._fire(out, effect)
        else:
            self._glass(out, effect)
        return bytes(out)

    def _light(self, out: bytearray, e: Union[Lamp, Flashlight, GodRays]) -> None:
        l = e.light
        _check(
            _point(l.position)
            and _rgb(l.color)
            and 0.001 <= l.height <= 32768
            and 0.001 <= l.radius <= 32768
            and 0 <= l.intensity <= 16
            and 0 <= l.ambient <= 1
        )
        lx, ly = l.position
        w, h = self._raster
        rays = isinstance(e, GodRays)
        spot = isinstance(e, Flashlight)
        ax = ay = axis = edge = 0.0
        softness = 1.0
        if isinstance(e, Flashlight):
            _check(_point(e.aim) and 0.01 <= e.beam <= 1.56)
            ax, ay = e.aim[0] - lx, e.aim[1] - ly
            axis = math.sqrt(ax * ax + ay * ay + l.height * l.height)
            edge = math.cos(e.beam)
        if isinstance(e, GodRays):
            softness = e.softness
            _check(0.1 <= softness <= 8)
        reach2 = (2 * l.height + l.radius * 3) ** 2
        heights = self._heights
        fractions = [(s + 0.5) / 12 for s in range(12)]
        # The ray fractions separate by axis. Compute coordinate maps once per row/column.
        radii = [
            softness * (0.6 + t * 1.8) if rays else 0.4 + t * l.radius * 0.65
            for t in fractions
        ]

        def ranges(
            coords: Sequence[float], source: float, spacing: float, count: int
        ) -> List[List[Tuple[int, int]]]:
            result = []
            for v in coords:
                pairs = []
                for t, r in zip(fractions, radii):
                    p = v + (source - v) * t
                    lo = max(0, min(count - 1, math.floor((p - r) / spacing)))
                    hi = max(lo + 1, min(count, math.ceil((p + r) / spacing)))
                    pairs.append((lo, hi) if 0 <= p < count * spacing else (0, 0))
                result.append(pairs)
            return result

        cols = ranges(self._xs, lx, self._sx, w)
        rows = ranges(self._ys, ly, self._sy, h)
        table = self._heights_sum
        stride = w + 1
        tint = [v / 255 for v in l.color]
        for row, y in enumerate(self._ys):
            dy = ly - y
            for col, x in enumerate(self._xs):
                dx = lx - x
                i = row * w + col
                z = heights[i]
                rr = dx * dx + dy * dy
                fall = 1 / (1 + rr / reach2)
                beam = 1.0
                if spot:
                    dz = z - l.height
                    cosine = (-dx * ax - dy * ay - dz * l.height) / max(
                        1e-6, math.sqrt(rr + dz * dz) * axis
                    )
                    beam = min(1.0, max(0.0, (cosine - edge) / (1 - edge) * 1.5))
                    beam = beam * beam * (3 - 2 * beam)
                shadow = opened = 0.0
                for (x0, x1), (y0, y1), t in zip(cols[col], rows[row], fractions):
                    if x0 == x1 or y0 == y1:
                        opened += 1
                        continue
                    total = (
                        table[y1 * stride + x1]
                        + table[y0 * stride + x0]
                        - table[y0 * stride + x1]
                        - table[y1 * stride + x0]
                    )
                    height = total / ((x1 - x0) * (y1 - y0))
                    if rays:
                        opened += 1 / (1 + height / softness)
                    else:
                        clearance = height - (z + (l.height - z) * t)
                        if clearance > 0:
                            shadow = max(
                                shadow, min(1.0, clearance / (1 + l.radius * t))
                            )
                power = (
                    fall
                    * l.intensity
                    * (
                        (opened / 12) ** 2 * (0.12 if z > 0 else 1)
                        if rays
                        else beam * (1 - 0.88 * shadow)
                    )
                )
                for c in range(3):
                    k = i * 4 + c
                    out[k] = min(
                        255,
                        int(
                            out[k] * l.ambient
                            + (out[k] * 0.86 + 22) * tint[c] * power
                            + 0.5
                        ),
                    )
                if not rays and not spot:
                    _over(
                        out,
                        4 * i,
                        l.color,
                        math.exp(-rr / (l.radius * l.radius * 0.35)) * l.intensity,
                    )

    def _fire(self, out: bytearray, e: Fire) -> None:
        _check(
            0.001 <= e.height <= 32768
            and 0 <= e.speed <= 16
            and 0 <= e.embers <= 1
            and math.isfinite(e.time)
            and e.time >= 0
            and 0 <= e.seed <= 0xFFFFFFFF
        )
        t = (e.time % 65536) * e.speed
        phase = (e.seed % 65521) * 0.063
        for i, rise in enumerate(self._rise):
            if not 0 < rise <= e.height * 1.8:
                continue
            u = self._xs[i % self._raster[0]] / e.height
            n = (
                math.sin(u * 13 + t * 3 + phase + math.sin(u * 7 - t * 2)) * 0.5
                + math.sin(u * 28 - t * 4 + phase + rise / e.height * 2) * 0.3
                + math.sin(u * 51 + t * 5) * 0.2
            )
            heat = max(0.0, 1 - rise / (e.height * (0.62 + n * 0.4)))
            _over(
                out,
                i * 4,
                (255, int(40 + 206 * heat + 0.5), int(155 * heat**3 + 0.5)),
                min(1.0, heat * 3),
            )
            ember = math.sin(math.floor(u * 5) * 73.7 + phase) * 19.1
            altitude = (((t * 0.52 + ember) % 1) * 1.65 + 0.1) * e.height
            lateral = (
                ((u * 5) % 1 - 0.5 - 0.28 * math.sin(t * 2 + ember)) * e.height / 5
            )
            spark = max(
                0.0,
                1
                - math.hypot(lateral, (rise - altitude) * 0.7) / (1 + e.height * 0.025),
            )
            _over(out, i * 4, (255, 199, 92), e.embers * spark)

    def _glass(self, out: bytearray, e: LiquidGlass) -> None:
        _check(
            _point(e.center)
            and all(0.001 <= v <= 32768 for v in e.size)
            and 0 <= e.depth <= 256
            and 0 <= e.dispersion <= 1
            and 0 <= e.frost <= 1
            and (
                e.wave is None
                or (
                    _point(e.wave.center)
                    and math.isfinite(e.wave.age)
                    and e.wave.age >= 0
                )
            )
        )
        w, h = self._raster
        radius = min(e.size) * 0.5
        bx, by = e.size[0] * 0.5 - radius, e.size[1] * 0.5 - radius
        art = self._art

        def index(x: float, y: float) -> int:
            return 4 * (
                max(0, min(h - 1, int(y / self._sy))) * w
                + max(0, min(w - 1, int(x / self._sx)))
            )

        for row, y in enumerate(self._ys):
            for col, x in enumerate(self._xs):
                dx, dy = x - e.center[0], y - e.center[1]
                nx, ny = dx - max(-bx, min(bx, dx)), dy - max(-by, min(by, dy))
                norm = math.hypot(nx, ny)
                inside = norm <= radius
                px, py = x, y
                if e.wave is not None and e.wave.age < 1:
                    vx, vy = x - e.wave.center[0], y - e.wave.center[1]
                    d = max(0.001, math.hypot(vx, vy))
                    band = (d - e.wave.age * self._logical[0] * 0.85) / 5
                    push = band * math.exp(-band * band * 0.5) * 7 * (1 - e.wave.age)
                    px += vx / d * push
                    py += vy / d * push
                if not inside and px == x and py == y:
                    continue
                rim = (norm / radius) ** 4 if inside else 0.0
                bend = e.depth * rim / max(0.001, norm)
                px -= nx * bend
                py -= ny * bend
                k = 4 * (row * w + col)
                mid = index(px, py)
                out[k : k + 4] = art[mid : mid + 4]
                if not inside:
                    continue
                for c in range(3):
                    split = (c - 1) * e.dispersion * bend
                    u, v = px + nx * split, py + ny * split
                    sample = index(u, v)
                    base = art[mid + c]
                    coverage = min(1.0, art[sample + 3] / max(1, art[mid + 3]))
                    value = base + (art[sample + c] - base) * coverage
                    if e.frost:
                        r = 1 + e.frost * 4
                        total = weight = 0.0
                        for ox, oy in ((-r, 0.0), (r, 0.0), (0.0, -r), (0.0, r)):
                            j = index(u + ox, v + oy)
                            total += art[j + c] * art[j + 3]
                            weight += art[j + 3]
                        if weight:
                            value += (total / weight - value) * e.frost
                    out[k + c] = int(value + 0.5)
                highlight = min(
                    1.0, max(0.0, (-nx - ny) / max(0.001, norm) * 0.4 + 0.5)
                )
                _over(
                    out,
                    k,
                    (225, 245, 255),
                    rim**3 * (0.18 + highlight * 0.65) + e.frost * 0.08,
                )
