"""Native vector effects in local logical units. The caller owns the transform and clip."""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Callable, Literal, Optional, Sequence, Tuple
from .effects import Box, Surface, Wave
from vivid_sdk.overlay import (
    Brush,
    Canvas,
    GradientStop,
    Path,
    Point,
    Rect,
    Shadow,
    StrokeStyle,
)

XY = Tuple[float, float]


def _color(rgb: int, opacity: float) -> int:
    return (rgb << 8) | round(255 * min(1.0, max(0.0, opacity)))


def pulse_ring(
    canvas: Canvas, center: XY, radius: float, width: float, rgb: int, opacity: float
) -> None:
    """Draw a luminous ring with a soft outer stroke and a fine core."""
    if radius <= 0 or width <= 0 or opacity <= 0:
        return
    x, y = center
    path = Path.ellipse(Rect(x - radius, y - radius, 2 * radius, 2 * radius))
    for spread, power in ((3.0, 0.10), (1.6, 0.24), (1.0, 0.88)):
        canvas.stroke(path, Brush.solid(_color(rgb, opacity * power)), width * spread)


def electric_arc(
    canvas: Canvas,
    start: XY,
    end: XY,
    *,
    time: float,
    amplitude: float,
    glow: float,
    rgb: int = 0x7DB5FF,
    seed: int = 0,
    forks: bool = True,
) -> None:
    """Draw an animated, forked discharge between two pinned endpoints in local coordinates."""
    x, y = start
    dx, dy = end[0] - x, end[1] - y
    length = math.hypot(dx, dy)
    if length < 0.01 or glow <= 0:
        return
    count = max(5, min(48, math.ceil(length / 2.6)))
    path, branches = Path().move_to(x, y), Path()
    phase = (time % 4096) * 12 + seed * 2.7
    for i in range(1, count + 1):
        t = i / count
        envelope = math.sin(math.pi * t)
        noise = (
            math.sin(i * 2.31 + phase) * 0.44
            + math.sin(i * 0.79 - phase * 0.63) * 0.56
            + math.sin(i * 4.3 - phase * 1.4) * 0.24
        )
        offset = amplitude * envelope * noise
        px, py = x + dx * t - dy / length * offset, y + dy * t + dx / length * offset
        path.line_to(px, py)
        if forks and i % 8 == 4 and i < count - 2:
            side = (-1 if i % 16 == 4 else 1) * amplitude
            branches.move_to(px, py).line_to(
                px + dx / count * 1.5 - dy / length * side,
                py + dy / count * 1.5 + dx / length * side,
            ).line_to(
                px + dx / count * 3 - dy / length * side * 1.7,
                py + dy / count * 3 + dx / length * side * 1.7,
            )
    for width, color, strength in (
        (5.4, rgb, 0.07),
        (2.8, rgb, 0.18),
        (1.3, rgb, 0.72),
        (0.43, 0xECF9FF, 1.0),
    ):
        canvas.stroke_styled(
            path,
            Brush.solid(_color(color, glow * strength)),
            StrokeStyle(width=width, cap="round", join="round"),
        )
    if forks and count >= 7:
        canvas.stroke(branches, Brush.solid(_color(rgb, glow * 0.68)), 0.38)


Paint = Callable[[Canvas], None]


@dataclass(frozen=True)
class Scene:
    """Extent of the effect and silhouettes that cast shadows or disperse light."""

    bounds: Rect
    surfaces: Sequence[Surface] = ()


@dataclass(frozen=True)
class RoundedBox:
    rect: Box
    radius: float


@dataclass(frozen=True)
class Glass:
    lens: RoundedBox
    depth: float
    dispersion: float
    frost: float
    wave: Optional[Wave] = None


def _rounded(box: Box, radius: float) -> Path:
    return Path.rounded_rectangle(Rect(*box), radius)


def _vertical(y: float, h: float, *stops: Tuple[float, int]) -> Brush:
    return Brush.linear(Point(0, y), Point(0, y + h), [GradientStop(*s) for s in stops])


def _radial(p: XY, radius: float, *stops: Tuple[float, int]) -> Brush:
    return Brush.radial(Point(*p), radius, [GradientStop(*s) for s in stops])


def _shadow(c: Canvas, b: Box, r: float, offset: XY, blur: float, color: int) -> None:
    c.shadow(Shadow(Rect(*b), (r,) * 4, color, Point(*offset), blur))


def _shadows(
    c: Canvas, scene: Scene, pos: XY, height: float, radius: Optional[float]
) -> None:
    x, y = pos
    for surface in scene.surfaces:
        shape, r, z = surface.rect, surface.radius, surface.elevation
        if z:
            shift = 0.48 * z / max(height - z, 8)
            offset = (
                (shape[0] + shape[2] * 0.5 - x) * shift,
                (shape[1] + shape[3] * 0.5 - y) * shift,
            )
            _shadow(
                c,
                shape,
                r,
                offset,
                1 + radius * 0.65 + 0.12 * math.hypot(*offset)
                if radius is not None
                else 3,
                _color(0x040814, 0.7),
            )


def lamp(
    c: Canvas,
    scene: Scene,
    position: XY,
    *,
    height: float,
    radius: float,
    intensity: float,
    paint: Paint,
) -> None:
    """Cast warm light and shadows. Paint foreground artwork in the callback."""
    _shadows(c, scene, position, height, radius)
    paint(c)
    full = Path.rectangle(scene.bounds)
    c.fill(
        full,
        _radial(
            position,
            height * 2 + radius * 3,
            (0, _color(0xFFCD8A, 0.34 * intensity)),
            (0.5, _color(0xFFBB78, 0.12 * intensity)),
            (1, _color(0x080A19, 0.44)),
        ),
    )
    c.fill(
        full,
        _radial(
            position,
            radius * 2.5,
            (0, _color(0xFFFEE6, 1)),
            (0.27, _color(0xFFE7AD, 0.98)),
            (0.4, _color(0xFFCB7C, 0.7)),
            (1, _color(0xFFBE71, 0)),
        ),
    )


def flashlight(
    c: Canvas,
    scene: Scene,
    position: XY,
    *,
    radius: float,
    ambient: float,
    level: float,
    paint: Paint,
) -> None:
    """Illuminate a circular beam. Ambient and level range from zero to one."""
    _shadows(c, scene, position, 56, None)
    paint(c)
    full = Path.rectangle(scene.bounds)
    c.fill(
        full,
        _radial(
            position,
            radius,
            (0, _color(0xFFE1AA, 0.32 * level)),
            (0.45, _color(0xFFDFAA, 0.15 * level)),
            (0.82, _color(0x080C1A, (1 - ambient) * 0.82)),
            (1, _color(0x080C1A, 1 - ambient)),
        ),
    )
    if level < 1:
        c.fill(full, Brush.solid(_color(0x080C1A, (1 - level) * (1 - ambient))))


def god_rays(
    c: Canvas,
    scene: Scene,
    position: XY,
    *,
    strength: float,
    softness: float,
    level: float,
) -> None:
    """Paint projected shafts behind the foreground artwork."""
    x, y = position
    extent = max(scene.bounds.width, scene.bounds.height)
    full = Path.rectangle(scene.bounds)
    level = min(1.0, max(0.0, level))
    c.fill(
        full,
        _radial(
            position,
            extent * (235 / 288),
            (0, _color(0xFFEBC2, strength * 0.45 * level)),
            (0.42, _color(0xF8B78C, strength * 0.18 * level)),
            (1, _color(0xDFA1B6, 0)),
        ),
    )

    def attenuate(opacity: float) -> float:
        return 1 - math.pow(1 - opacity * level, 1 / 9)

    shade = _radial(
        position,
        extent * (193 / 288),
        (0, _color(0x151524, attenuate(0.9))),
        (0.55, _color(0x151524, attenuate(0.68))),
        (1, _color(0x151524, attenuate(0.24))),
    )
    for i in range(9):
        angle = i * 2.399963
        reach = (4 + softness * 2.6) * math.sqrt(i / 8)
        lx, ly = x + reach * math.cos(angle), y + reach * math.sin(angle)
        for surface in scene.surfaces:
            l, t, w, h = surface.rect
            z = surface.elevation
            if not z or (l <= lx <= l + w and t <= ly <= t + h):
                continue
            axis = math.atan2(t + h / 2 - ly, l + w / 2 - lx)
            corners = sorted(
                [(l, t), (l + w, t), (l + w, t + h), (l, t + h)],
                key=lambda q: (
                    (math.atan2(q[1] - ly, q[0] - lx) - axis + math.pi) % math.tau
                ),
            )
            u0, v0 = corners[0], corners[-1]
            path = (
                Path()
                .move_to(*u0)
                .line_to(*v0)
                .line_to(lx + (v0[0] - lx) * 35, ly + (v0[1] - ly) * 35)
                .line_to(lx + (u0[0] - lx) * 35, ly + (u0[1] - ly) * 35)
                .close()
            )
            c.fill(path, shade)
    c.fill(
        full,
        _radial(
            position,
            extent * (135 / 288),
            (0, _color(0xFFDFAC, 0.28 * level)),
            (0.18, _color(0xFFC38A, 0.08 * level)),
            (0.54, _color(0x191B29, 0.48)),
            (1, _color(0x191B29, 0.90)),
        ),
    )


def glow(
    c: Canvas,
    panel: RoundedBox,
    rgb: int,
    *,
    radius: float,
    intensity: float,
    paint: Paint,
) -> None:
    """Paint a halo and edge around a surface, sandwiching its foreground artwork."""
    _shadow(
        c, panel.rect, panel.radius, (0, 0), radius * 2.4, _color(rgb, 0.86 * intensity)
    )
    paint(c)
    c.stroke(
        _rounded(panel.rect, panel.radius), Brush.solid(_color(rgb, intensity)), 0.8
    )


def glow_path(
    c: Canvas, tube: Path, rgb: int, *, radius: float, intensity: float
) -> None:
    """Stroke an arbitrary path with a halo and a bright core."""
    for width, opacity, color in (
        (radius * 1.4, 0.06 * intensity, rgb),
        (radius * 0.65, 0.16 * intensity, rgb),
        (2.6, 0.8 * intensity, rgb),
        (1.05, intensity + 0.14, 0xEBFFFB),
    ):
        if width > 0:
            c.stroke(tube, Brush.solid(_color(color, opacity)), width)


def _thermal_color(t: float) -> int:
    # sRGB emission ramp from dim soot through orange to a yellow-white core.
    ramp = (
        (107, 30, 16),
        (198, 60, 17),
        (250, 117, 24),
        (255, 181, 56),
        (255, 226, 134),
        (255, 248, 219),
    )
    value = max(0.0, min(1.0, t)) * (len(ramp) - 1)
    i = min(int(value), len(ramp) - 2)
    f = value - i
    r, g, b = (round(a + (b - a) * f) for a, b in zip(ramp[i], ramp[i + 1]))
    return r << 16 | g << 8 | b


def _sway(z: float, phase: float, time: float) -> float:
    # Backtrace a rising parcel through the large drift and smaller curling eddies.
    age = time - z * 1.15
    return z * (
        0.15 * math.sin(phase + 2.1 * age)
        + 0.085 * math.sin(1.73 * phase - 3.7 * age + 5.3 * z)
    )


def fire(
    c: Canvas,
    bounds: Rect,
    origin: XY,
    *,
    width: float,
    height: float,
    embers: float,
    time: float,
) -> None:
    """Draw turbulent, cooling plumes above a fuel bed. Time is in seconds."""
    if width <= 0 or height <= 0:
        return
    left, base = origin
    c.fill(
        Path.rectangle(bounds),
        _radial(
            (left + width / 2, base),
            width * 0.64,
            (0, _color(0xFF7D29, 0.21)),
            (0.56, _color(0xE34B1D, 0.06)),
            (1, _color(0xC33E16, 0)),
        ),
    )
    unit = Path.rectangle(Rect(-1, -1, 2, 2))
    # Each cross section has a continuous radial falloff. Shearing its long axis
    # along the velocity field joins adjacent sections into curling streams.
    c.save()
    try:
        c.clip(
            Path.rectangle(
                Rect(left - width * 0.25, base - height * 2, width * 1.5, height * 2)
            )
        )
        layers = (
            (12, 5, 1.32, 1.30, 0.0, 0.064),
            (25, 8, 1.02, 1.12, 0.82, 0.68),
            (32, 7, 0.78, 1.12 * 0.78, 1.02, 0.83),
        )
        for layer, (count, slices, altitude, spread, gain, opacity) in enumerate(
            layers
        ):
            for i in range(count):
                seed = i * 2.399963 + layer * 1.71
                u = (i + 0.5 + 0.30 * math.sin(seed * 3.7)) / count
                edge = min(1.0, u * 12, (1 - u) * 12)
                phase = u * 32 + 0.65 * math.sin(seed) + layer * 0.34
                for k in range(slices - 1, -1, -1):
                    z = k / slices
                    age = time - z * 0.86
                    fuel = (
                        0.66
                        + 0.21
                        * math.sin(u * 25 + 1.45 * math.sin(u * 9 + age) - 2.1 * age)
                        + 0.15 * math.sin(u * 53 + age * 3.4)
                        + 0.08 * math.cos(u * 11 - age)
                    )
                    flicker = (
                        0.94
                        + 0.15 * math.sin(seed + age * 3.9)
                        + 0.08 * math.sin(seed * 2.7 - age * 6.7)
                    )
                    reach = height * fuel * flicker * (0.35 + 0.65 * edge)
                    rise = reach * altitude
                    x = left + width * u + rise * _sway(z, phase, time)
                    y = base - z * rise
                    half = (
                        width
                        / count
                        * spread
                        * (1 - 0.88 * z)
                        * (0.86 + 0.20 * math.sin(phase + z * 8 - age * 4))
                    )
                    pulse = 0.5 + 0.5 * math.sin(
                        seed + time * 4.5 - z * 10 + 0.6 * math.sin(phase + z * 6)
                    )
                    half *= 1.0 - z * 0.58 * (1 - pulse)
                    length = rise * (0.24 - 0.06 * z)
                    tilt = (
                        _sway(z + 0.02, phase, time) - _sway(z - 0.02, phase, time)
                    ) / 0.04
                    fade = (1 - z) ** 0.75 * (1 - z * 0.75 * (1 - pulse))
                    if layer == 0:
                        rgb = 0x806958
                    else:
                        heat = (1 - z) ** 1.35 * gain + 0.085 * math.sin(
                            seed + z * 12 - age * 3
                        )
                        rgb = _thermal_color(heat)
                    power = opacity * fade
                    brush = _radial(
                        (0, 0),
                        1,
                        (0, _color(rgb, power)),
                        (0.35, _color(rgb, power * 0.87)),
                        (0.70, _color(rgb, power * 0.34)),
                        (1, _color(rgb, 0)),
                    )
                    c.save()
                    try:
                        c.transform(half, 0, length * tilt, -length, x, y)
                        c.fill(unit, brush)
                    finally:
                        c.restore()
    finally:
        c.restore()
    if embers <= 0:
        return
    for i in range(22):
        seed = i * 2.399963
        age = (time * (0.36 + 0.12 * math.sin(seed)) + i * 0.618034) % 1
        u = (i + 0.5) / 22
        drift = math.sin(age * 5 + seed) - math.sin(seed)
        x = left + width * u + height * 0.09 * age * drift
        y = base - height * age * (0.8 + 0.44 * abs(math.sin(seed)))
        alpha = (
            embers
            * min(1.0, age * 12, (1 - age) * 4)
            * (0.65 + 0.35 * math.sin(seed) ** 2)
        )
        radius = min(width, height) * (0.004 + 0.006 * (1 - age))
        c.fill(
            Path.rectangle(
                Rect(x - 3 * radius, y - 3 * radius, 6 * radius, 6 * radius)
            ),
            _radial(
                (x, y),
                3 * radius,
                (0, _color(0xFF9440, alpha * 0.35)),
                (1, _color(0xFF7125, 0)),
            ),
        )
        spark = Rect(x - radius, y - 2 * radius, 2 * radius, 4 * radius)
        c.fill(
            Path.ellipse(spark),
            _vertical(
                y - 2 * radius,
                4 * radius,
                (0, _color(0xFFEB96, alpha)),
                (1, _color(0xFF6027, alpha * 0.15)),
            ),
        )


def _rim_sample(
    shape: RoundedBox, guide: float, u: float
) -> Tuple[float, float, float, float]:
    x, y, w, h = shape.rect
    r = min(max(0.0, shape.radius), w / 2, h / 2)
    radius = r + guide
    arc = radius * math.pi / 2
    distance = u * (2 * (w + h - 4 * r) + 4 * arc)
    # Each tangent ends at a quarter circle. Its outward normal is continuous
    # across both joins, including the seam between the last and first side.
    for side, (tx, ty) in enumerate(((1, 0), (0, 1), (-1, 0), (0, -1))):
        length = (w if side % 2 == 0 else h) - 2 * r
        cx = x + w - r if side in (0, 1) else x + r
        cy = y + r if side in (0, 3) else y + h - r
        if distance <= length:
            return (
                cx + radius * ty - (length - distance) * tx,
                cy - radius * tx - (length - distance) * ty,
                ty,
                -tx,
            )
        distance -= length
        if distance <= arc or side == 3:
            a = distance / radius
            nx = ty * math.cos(a) + tx * math.sin(a)
            ny = -tx * math.cos(a) + ty * math.sin(a)
            return cx + radius * nx, cy + radius * ny, nx, ny
        distance -= arc
    raise AssertionError("unreachable")


def plasma(
    c: Canvas, shape: RoundedBox, *, width: float, intensity: float, time: float
) -> None:
    """Draw a flowing violet and cyan plasma border. Width is in logical units."""
    x, y, w, h = shape.rect
    if min(w, h, width, intensity) <= 0:
        return
    r = min(max(shape.radius, 0.0), w / 2, h / 2)
    guide = width * 0.58
    rim = _rounded((x - guide, y - guide, w + 2 * guide, h + 2 * guide), r + guide)
    for spread, opacity in (
        (3.2, 0.035),
        (2.5, 0.055),
        (1.9, 0.08),
        (1.4, 0.11),
        (0.95, 0.14),
    ):
        c.stroke(
            rim, Brush.solid(_color(0x7563EC, opacity * intensity)), width * spread
        )
    count = 128
    samples = [_rim_sample(shape, guide, k / count) for k in range(count)]
    for j in range(6):
        phase = j * 2.399963
        points = []
        for k, (px, py, nx, ny) in enumerate(samples):
            a = math.tau * k / count
            offset = width * (
                0.31
                * math.sin(3 * a + phase + time * 1.5 + 0.8 * math.sin(2 * a - time))
                + 0.19
                * math.sin(
                    7 * a - 2.4 * time + phase * 2 + 0.8 * math.cos(3 * a + time)
                )
            )
            points.append((px + nx * offset, py + ny * offset))
        p, q = points[-1], points[0]
        path = Path().move_to((p[0] + q[0]) / 2, (p[1] + q[1]) / 2)
        for k, p in enumerate(points):
            q = points[(k + 1) % count]
            path.quad_to(*p, (p[0] + q[0]) / 2, (p[1] + q[1]) / 2)
        path.close()
        c.stroke(path, Brush.solid(_color(0x8473FF, intensity * 0.19)), width * 0.41)
        c.stroke(
            path,
            Brush.solid(_color(0x9FC4FF if j % 2 else 0x9C8BFF, intensity * 0.48)),
            width * 0.12,
        )
        # Bright crests travel continuously along the same filament. Their
        # tapered ends merge back into the dimmer, unbroken strand.
        for crest in range(2):
            start = (time * (0.067 + 0.006 * j) + j * 0.381966 + crest * 0.5) % 1
            upper, lower = [], []
            for step in range(17):
                pos = (start + step * 0.007) * count
                k = math.floor(pos)
                f = pos - k
                p, q = points[k % count], points[(k + 1) % count]
                px, py = p[0] + (q[0] - p[0]) * f, p[1] + (q[1] - p[1]) * f
                # Interpolate the guide normals as well, so a crest can cross a corner.
                a0, a1 = samples[k % count], samples[(k + 1) % count]
                nx = a0[2] + (a1[2] - a0[2]) * f
                ny = a0[3] + (a1[3] - a0[3]) * f
                half = width * 0.125 * math.sin(math.pi * step / 16) ** 2
                upper.append((px + nx * half, py + ny * half))
                lower.append((px - nx * half, py - ny * half))
            crest_path = Path().move_to(*upper[0])
            for vertex in upper[1:] + lower[-2:0:-1]:
                crest_path.line_to(*vertex)
            c.fill(crest_path.close(), Brush.solid(_color(0xCBF1FF, 0.88 * intensity)))


def _magnify(c: Canvas, paint: Paint, clip: Path, p: XY, scale: float) -> None:
    c.save()
    try:
        c.clip(clip).transform(
            scale, 0, 0, scale, p[0] * (1 - scale), p[1] * (1 - scale)
        )
        paint(c)
    finally:
        c.restore()


def _glass_detail(
    c: Canvas,
    scene: Scene,
    shape: RoundedBox,
    center: XY,
    depth: float,
    dispersion: float,
    frost: float,
) -> None:
    """Repaint visible silhouettes through displaced color edges and a Gaussian frost."""
    if not (dispersion or frost):
        return
    lens = shape.rect
    w, h = lens[2:]
    x, y = center
    scale = 1 + depth * 0.008
    c.save()
    try:
        c.clip(_rounded(lens, shape.radius))
        if frost:
            c.fill(
                _rounded(lens, shape.radius),
                Brush.solid(_color(0xE4E6DF, 0.65 * frost)),
            )
        c.transform(scale, 0, 0, scale, x * (1 - scale), y * (1 - scale))
        for surface in scene.surfaces:
            b, r, color = surface.rect, surface.radius, surface.color
            alpha = (color & 255) / 255
            if (
                alpha == 0
                or b[0] > x + w / 2 + 5
                or b[0] + b[2] < x - w / 2 - 5
                or b[1] > y + h / 2 + 5
                or b[1] + b[3] < y - h / 2 - 5
            ):
                continue
            if frost:
                _shadow(
                    c,
                    b,
                    r,
                    (0, 0),
                    1 + 4 * frost,
                    _color(color >> 8, 0.84 * frost * alpha),
                )
            if dispersion:
                for direction, hue in ((-1, 0xE95791), (1, 0x4CD9EC)):
                    dx = (
                        min(w / 2, max(-w / 2, b[0] + b[2] * 0.5 - x))
                        * dispersion
                        * 0.06
                        * direction
                    )
                    dy = (
                        min(h / 2, max(-h / 2, b[1] + b[3] * 0.5 - y))
                        * dispersion
                        * 0.05
                        * direction
                    )
                    c.stroke(
                        _rounded((b[0] + dx, b[1] + dy, b[2], b[3]), r),
                        Brush.solid(_color(hue, dispersion * 0.72 * alpha)),
                        0.55 + dispersion * 0.5,
                    )
    finally:
        c.restore()


def liquid_glass(c: Canvas, scene: Scene, glass: Glass, paint: Paint) -> None:
    """Refract existing artwork. Replay its background and foreground in paint."""
    lens = glass.lens.rect
    left, top, w, h = lens
    if not w > 0 or not h > 0:
        raise ValueError("lens dimensions must be positive")
    radius = min(glass.lens.radius, w / 2, h / 2)
    pos = (left + w / 2, top + h / 2)
    depth, dispersion, frost = glass.depth, glass.dispersion, glass.frost
    wave = glass.wave
    if wave and 0.001 < wave.age < 1:
        t = wave.age
        wx, wy = wave.center
        r = t * max(scene.bounds.width, scene.bounds.height) * (225 / 288)
        _magnify(
            c,
            paint,
            _rounded((wx - r, wy - r, 2 * r, 2 * r), r),
            wave.center,
            1 + 0.035 * (1 - t),
        )
        pulse_ring(c, wave.center, r, 0.65, 0xFFFFFF, 0.62 * (1 - t))
    _shadow(c, lens, radius, (0, 2.8), 5, _color(0x17243D, 0.36))
    for inset, factor in ((0, 0.020), (0.85, 0.013), (2.4, 0.008)):
        inset *= min(w, h, 42) / 42
        _magnify(
            c,
            paint,
            _rounded(
                (lens[0] + inset, lens[1] + inset, w - 2 * inset, h - 2 * inset),
                max(0.0, radius - inset),
            ),
            pos,
            1 + depth * factor,
        )
    _glass_detail(c, scene, RoundedBox(lens, radius), pos, depth, dispersion, frost)
    c.fill(
        _rounded(lens, radius),
        _vertical(
            lens[1],
            h,
            (0, _color(0xF4FFFF, 0.33 + frost * 0.40)),
            (0.3, _color(0xFAFFFF, frost * 0.48)),
            (0.76, _color(0xEDFAFF, frost * 0.48)),
            (1, _color(0xA0D5ED, 0.42 + frost * 0.18)),
        ),
    )
    c.stroke(
        _rounded(lens, radius),
        _vertical(
            lens[1],
            h,
            (0, _color(0xFFFFFF, 0.96)),
            (0.45, _color(0xB5FBFF, 0.26 + dispersion * 0.65)),
            (1, _color(0x8178D0, 0.46 + dispersion * 0.6)),
        ),
        1,
    )


def radar(
    c: Canvas,
    center: XY,
    *,
    radius: float,
    reach: float,
    phase: float,
    count: int,
    rgb: int,
) -> None:
    """Animate up to 64 repeating rings. Phase is measured in cycles."""
    count = min(64, count)
    for i in range(count):
        t = (phase + i / count) % 1
        pulse_ring(c, center, radius + t * reach, 0.66, rgb, (1 - t) ** 2)


def pulse(
    c: Canvas,
    shape: RoundedBox,
    mode: Literal["breathe", "heartbeat"],
    phase: float,
    rgb: int,
) -> None:
    """Draw an animated halo behind a surface. Phase is measured in cycles."""
    if mode == "breathe":
        b = 0.5 - 0.5 * math.cos(phase * math.tau)
        blur, opacity = 3 + 9 * b, 0.35 + 0.6 * b
    elif mode == "heartbeat":
        beat = phase % 1
        h = math.exp(-(((beat - 0.14) / 0.10) ** 2)) + 0.55 * math.exp(
            -(((beat - 0.37) / 0.08) ** 2)
        )
        blur, opacity = 2 + 8 * h, 0.28 + 0.72 * h
    else:
        raise ValueError("unknown pulse mode")
    _shadow(c, shape.rect, shape.radius, (0, 0), blur, _color(rgb, opacity))


def ping(c: Canvas, start: XY, end: XY, progress: float, rgb: int) -> None:
    """Draw a traveling ping and its trail. Progress ranges from zero to one."""
    dx, dy = end[0] - start[0], end[1] - start[1]
    length = math.hypot(dx, dy)
    if not 0 <= progress <= 1 or not length:
        return
    for j in reversed(range(7)):
        r = 1.8 - j * 0.15
        t = progress - j * 1.7 / length
        c.fill(
            _rounded((start[0] + dx * t - r, start[1] + dy * t - r, 2 * r, 2 * r), r),
            Brush.solid(_color(rgb, 1 - j / 7)),
        )


def ripple(
    c: Canvas,
    center: XY,
    reach: float,
    progress: float,
    rgb: int,
    clip: Optional[RoundedBox] = None,
) -> None:
    """Draw an expanding ripple, optionally clipped to a rounded shape."""
    if not 0 <= progress < 1:
        return

    def draw() -> None:
        pulse_ring(c, center, 0.1 + reach * progress, 1.15, rgb, (1 - progress) ** 2)

    if clip is None:
        draw()
        return
    c.save()
    try:
        c.clip(_rounded(clip.rect, clip.radius))
        draw()
    finally:
        c.restore()


def electric_border(
    c: Canvas,
    shape: RoundedBox,
    *,
    time: float,
    amplitude: float,
    glow: float,
    rgb: int = 0x7DB5FF,
    seed: int = 0,
) -> None:
    """Trace current around a closed perimeter."""
    x, y, w, h = shape.rect
    r = min(shape.radius, w / 2, h / 2)
    corners = (
        (x + r, y),
        (x + w - r, y),
        (x + w, y + r),
        (x + w, y + h - r),
        (x + w - r, y + h),
        (x + r, y + h),
        (x, y + h - r),
        (x, y + r),
    )
    for i, corner in enumerate(corners):
        electric_arc(
            c,
            corner,
            corners[(i + 1) % 8],
            time=time,
            amplitude=amplitude * 0.45,
            glow=glow * 0.72,
            seed=seed + i,
            rgb=rgb,
            forks=False,
        )


def charge(
    c: Canvas,
    shape: RoundedBox,
    level: float,
    *,
    time: float,
    amplitude: float,
    glow: float,
    paint: Paint,
    rgb: int = 0x7DB5FF,
) -> None:
    """Surround a control with a halo and a discharge along its upper edge."""
    x, y, w, h = shape.rect
    r = min(shape.radius, w / 2, h / 2)
    if level > 0.001:
        _shadow(c, shape.rect, r, (0, 0), 8, _color(0x5FCEFF, level * 0.9))
    paint(c)
    if level > 0.001:
        c.stroke(_rounded(shape.rect, r), Brush.solid(_color(0xC2F5FF, level)), 0.7)
        electric_arc(
            c,
            (x + r, y),
            (x + w - r, y),
            time=time,
            amplitude=amplitude * 0.65,
            glow=glow * level,
            rgb=rgb,
        )


def zap(
    c: Canvas,
    start: XY,
    end: XY,
    progress: float,
    *,
    time: float,
    amplitude: float,
    glow: float,
    rgb: int = 0x7DB5FF,
    seed: int = 0,
) -> None:
    """Draw a fading discharge between two endpoints."""
    if 0 <= progress < 1:
        electric_arc(
            c,
            start,
            end,
            time=time,
            amplitude=amplitude * 1.6,
            glow=glow * (1 - progress),
            rgb=rgb,
            seed=seed,
        )
