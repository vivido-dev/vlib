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
    """Paint animated flame contours and drifting embers above a horizontal fuel bed."""
    left, base = origin
    full = Path.rectangle(bounds)
    c.fill(
        full,
        _radial(
            (left + 0.5 * width, base - 3),
            width * (114 / 220),
            (0, _color(0xF55B24, 0.38)),
            (1, _color(0xA03728, 0)),
        ),
    )
    # Nested heat contours curl sideways with altitude; their roots stay attached.
    plume = []
    for j in range(129):
        u = j / 128
        xx = left + width * u
        wind = math.sin(xx * 0.092 - time * 1.8)
        swirl = xx * 0.36 + 1.5 * wind + time * 1.9
        noise = (
            0.52
            + 0.30 * math.sin(swirl)
            + 0.16 * math.sin(swirl * 1.94 - time * 3.3)
            + 0.10 * math.sin(xx * 0.91 + time * 4.2)
        )
        envelope = max(0.0, math.sin(math.pi * u)) ** 0.24
        plume.append(
            (
                xx,
                height * (0.16 + min(1.0, max(0.0, noise)) ** 1.4 * 0.85) * envelope,
                wind,
            )
        )
    for band in range(24):
        heat = band / 23
        altitude = (1 - heat * 0.94) ** 1.45
        path = Path().move_to(left, base)
        for xx, extent, wind in plume:
            rise = extent * altitude
            sway = rise * (
                0.13 * math.sin(rise * 0.15 - time * 2.1 + xx * 0.09) + 0.08 * wind
            )
            path.line_to(xx + sway, base - rise)
        red = int(174 + 81 * min(1.0, heat / 0.42))
        green = int(35 + 220 * heat**0.90)
        blue = int(25 + 195 * heat**3)
        color = red << 16 | green << 8 | blue
        c.fill(
            path.line_to(left + width, base + 2).line_to(left, base + 2).close(),
            Brush.solid(_color(color, 0.10 + 0.69 * heat)),
        )
    for j in range(22):
        t = (time * (0.32 + math.sin(j * 0.76) * 0.12) + j * 0.618) % 1
        px = left + width * ((5 + j * 9.8) / 220) + math.sin(t * 5 + j) * 3
        py = base - 5 - t * height * (0.8 + 0.6 * abs(math.sin(j * 2.3)))
        r = 0.35 + 0.42 * (1 - t)
        c.fill(
            _rounded((px, py, r * 1.4, r * 2.7), r * 0.7),
            Brush.solid(_color(0xFFD38A, embers * (1 - t))),
        )


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
