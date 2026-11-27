"""Draw the eight lab scenes in logical coordinates on the host vector canvas."""

from __future__ import annotations

import math
from typing import Any, Dict, Optional, Sequence, Tuple
from vivid_sdk.overlay import Brush, Canvas, GradientStop, Path, Point, Rect
from vlib.vui import vector_effects as fx
from vlib.vui.effects import Surface, Wave

XY = Tuple[float, float]
Box = Tuple[float, float, float, float]
Shot = Tuple[XY, float]
STAGE = Rect(0, 0, 288, 144)


def tint(rgb: int, alpha: float = 1) -> int:
    return rgb * 256 + round(255 * max(0.0, min(1.0, alpha)))


def rounded(box: Box, radius: float) -> Path:
    return Path.rounded_rectangle(Rect(*box), radius)


def vertical(y: float, h: float, *stops: Tuple[float, int]) -> Brush:
    return Brush.linear(Point(0, y), Point(0, y + h), [GradientStop(*s) for s in stops])


def background(c: Canvas, page: Dict[str, Any]) -> None:
    c.fill(
        Path.rectangle(STAGE),
        vertical(
            0,
            144,
            (0, tint(int(page["background"], 16))),
            (1, tint(int(page["horizon"], 16))),
        ),
    )


def art(c: Canvas, page: Dict[str, Any], scene: fx.Scene) -> None:
    for surface in scene.surfaces:
        b = surface.rect
        path = rounded(b, surface.radius)
        c.fill(path, Brush.solid(surface.color))
        if surface.elevation or (b[2] > 18 and b[3] > 10):
            c.fill(
                path,
                vertical(
                    b[1],
                    b[3],
                    (0, tint(0xFFFFFF, 0.07)),
                    (0.52, tint(0xFFFFFF, 0)),
                    (1, tint(0xA1020, 0.12)),
                ),
            )
            c.stroke(path, Brush.solid(tint(0xF1EFFF, 0.18)), 0.35)
    for x, y, size, label, ink, weight in page["labels"]:
        c.text(
            label,
            Point(x, y),
            size,
            tint(int(ink, 16)),
            family="sans-serif",
            weight=weight,
        )


def lettering() -> Path:
    return (
        Path()
        .move_to(175, 87)
        .cubic_to(181, 75, 184, 55, 196, 50)
        .cubic_to(208, 46, 202, 63, 191, 66)
        .move_to(176, 72)
        .line_to(198, 70)
        .move_to(195, 84)
        .cubic_to(200, 66, 209, 49, 214, 51)
        .cubic_to(218, 56, 210, 71, 201, 76)
        .cubic_to(196, 87, 203, 89, 210, 77)
        .move_to(215, 67)
        .cubic_to(206, 90, 214, 90, 225, 69)
        .cubic_to(215, 93, 224, 86, 230, 77)
        .move_to(231, 67)
        .cubic_to(238, 63, 234, 85, 244, 84)
        .move_to(247, 65)
        .line_to(227, 87)
    )


def draw(
    c: Canvas,
    stage: Rect,
    page: Dict[str, Any],
    index: int,
    pos: XY,
    knobs: Sequence[float],
    level: float,
    now: float,
    wave: Optional[Shot],
    shots: Sequence[Shot],
    scene: fx.Scene,
) -> None:
    scale = stage.width / 288
    c.save().transform(scale, 0, 0, scale, stage.x, stage.y).clip(Path.rectangle(STAGE))
    background(c, page)

    def paint(canvas: Canvas) -> None:
        art(canvas, page, scene)

    if index == 0:
        height, radius, intensity = knobs
        fx.lamp(
            c,
            scene,
            pos,
            height=height,
            radius=radius,
            intensity=intensity,
            paint=paint,
        )
    elif index == 1:
        beam, ambient, _ = knobs
        fx.flashlight(
            c,
            scene,
            pos,
            radius=20 + 89 * beam,
            ambient=ambient,
            level=level,
            paint=paint,
        )
    elif index == 2:
        strength, softness, _ = knobs
        fx.god_rays(c, scene, pos, strength=strength, softness=softness, level=level)
        paint(c)
    elif index == 3:
        palette, radius, power = knobs
        rgb = (0x45DED5, 0xE96DA4, 0xFFA950, 0x8596FF)[int(palette)]
        fx.glow(
            c,
            fx.RoundedBox((24, 36, 127, 72), 12),
            rgb,
            radius=radius,
            intensity=power * level,
            paint=paint,
        )
        fx.glow_path(c, lettering(), rgb, radius=radius, intensity=power * level)
    elif index == 4:
        height, _, embers = knobs
        fx.fire(
            c,
            scene.bounds,
            (17, 99),
            width=143,
            height=height,
            embers=embers,
            time=now,
        )
        fx.plasma(
            c,
            fx.RoundedBox((181, 69, 88, 25), 12.5),
            width=3.1,
            intensity=1.0,
            time=now,
        )
        paint(c)
    elif index == 5:
        depth, dispersion, frost = knobs
        paint(c)

        def replay(canvas: Canvas) -> None:
            background(canvas, page)
            paint(canvas)

        lens = fx.RoundedBox((pos[0] - 40, pos[1] - 21, 80, 42), 21)
        fx.liquid_glass(
            c,
            scene,
            fx.Glass(
                lens,
                depth,
                dispersion,
                frost,
                Wave(wave[0], now - wave[1]) if wave else None,
            ),
            replay,
        )
    elif index == 6:
        reach, period, count = knobs
        fx.radar(
            c,
            (51, 35),
            radius=9,
            reach=reach,
            phase=now / period,
            count=int(count),
            rgb=0xFF748A,
        )
        fx.pulse(
            c, fx.RoundedBox((133, 24, 22, 22), 11), "breathe", now / 2.8, 0x5FF4BC
        )
        fx.pulse(
            c, fx.RoundedBox((216, 28, 34, 14), 7), "heartbeat", now / 1.1, 0xFF426E
        )
        paint(c)
        for at, start in shots:
            age = now - start
            if math.hypot(at[0] - 54, at[1] - 100) <= 12:
                if age < 0.45:
                    fx.ping(c, (66, 100), (155, 100), age / 0.45, 0x93C9FF)
                    continue
                at, age = (155, 100), age - 0.45
            inbox = (
                fx.RoundedBox((154, 79, 94, 45), 5)
                if 154 <= at[0] <= 248 and 79 <= at[1] <= 124
                else None
            )
            fx.ripple(c, at, (reach * 2 + 37) * 0.95, age / 0.95, 0x85BEFF, inbox)
    else:
        amplitude, speed, glow = knobs

        def circuit(canvas: Canvas) -> None:
            paint(canvas)
            fx.electric_arc(
                canvas,
                (31, 46),
                (112, 46),
                time=now * speed,
                amplitude=amplitude,
                glow=glow,
            )
            fx.electric_border(
                canvas,
                fx.RoundedBox((151, 23, 103, 46), 5),
                time=now * speed,
                amplitude=amplitude,
                glow=glow,
            )

        fx.charge(
            c,
            fx.RoundedBox((165, 90, 84, 20), 10),
            level,
            time=now * speed,
            amplitude=amplitude,
            glow=glow,
            paint=circuit,
        )
        for i, (at, start) in enumerate(shots):
            terminal = (31 if at[0] < 71.5 else 112, 46)
            fx.zap(
                c,
                terminal,
                at,
                (now - start) / 0.42,
                time=now * speed,
                amplitude=amplitude,
                glow=glow,
                seed=i + 9,
            )
    c.restore()


def scene_for(page: Dict[str, Any]) -> fx.Scene:
    return fx.Scene(
        STAGE,
        [
            Surface(tuple(b), r, tint(int(color, 16)), z)
            for b, r, color, z in page["surfaces"]
        ],
    )
