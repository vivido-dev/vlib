"""An interactive gallery of six producer-rendered lighting and optical effects."""

from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

from vlib.vui import Frame, PointerEvent, Rect, Ui, duration
from vlib.vui.effects import (
    Effect,
    EffectScene,
    Fire,
    Flashlight,
    Glow,
    GodRays,
    Lamp,
    Light,
    LiquidGlass,
    RGB,
    Surface,
    Wave,
    XY,
)

COLORS: Tuple[RGB, ...] = (
    (69, 222, 213),
    (233, 109, 164),
    (255, 169, 80),
    (133, 150, 255),
)
COLOR_NAMES = ("Mint", "Rose", "Amber", "Blue")
SPACE = (288, 144)
# A smaller sampling grid keeps the interpreted software renderer interactive.
RASTER = (144, 72)


def scene(page: Dict[str, Any]) -> EffectScene:
    shapes = [
        Surface(tuple(rect), r, int(color + "ff", 16), z, emit)
        for rect, r, color, z, emit in page["surfaces"]
    ]
    mask = None
    if "mask" in page:
        m = page["mask"]
        ox, oy = m["origin"]
        scale = m["scale"]
        rows = m["rows"]
        w, h = RASTER
        alpha = bytearray(w * h)
        for i in range(w * h):
            x = int(((i % w + 0.5) * SPACE[0] / w - ox) // scale)
            y = int(((i // w + 0.5) * SPACE[1] / h - oy) // scale)
            if 0 <= y < len(rows) and 0 <= x < len(rows[y]) and rows[y][x] == "1":
                alpha[i] = 255
        mask = bytes(alpha)
    return EffectScene(
        SPACE, RASTER, int(page["background"] + "ff", 16), shapes, mask=mask
    )


class Lab:
    def __init__(self) -> None:
        path = Path(__file__).resolve().parents[1] / "assets" / "effects_lab.json"
        self.pages: List[Dict[str, Any]] = json.loads(path.read_text())
        self.art = [scene(p) for p in self.pages]
        self.indices: List[List[int]] = [
            [k["initial"] for k in p["controls"]] for p in self.pages
        ]
        self.selected = 0
        self.position: XY = (0, 0)
        self.grab: Optional[XY] = None
        self.wave: Optional[Tuple[XY, float]] = None
        self.select(0)

    def select(self, index: int) -> None:
        self.selected = index
        self.position = tuple(self.pages[index]["position"])
        self.grab = self.wave = None
        self.tap: Optional[XY] = None
        self.start = self.target = 1.0
        self.changed = 0.0
        self.hovered = False

    def value(self, i: int) -> float:
        return float(
            self.pages[self.selected]["controls"][i]["values"][
                self.indices[self.selected][i]
            ]
        )

    def level(self, now: float) -> float:
        t = max(0.0, min(1.0, (now - self.changed) / 0.35))
        return self.start + (self.target - self.start) * t * t * (3 - 2 * t)

    def fade(self, on: bool, now: float) -> None:
        if self.target != float(on):
            self.start, self.target, self.changed = self.level(now), float(on), now

    def params(self, now: float) -> Tuple[Effect, bool]:
        a, b = self.value(0), self.value(1)
        x, y = self.position
        warm: RGB = (255, 206, 152)
        level = self.level(now)
        moving = self.start != self.target and now - self.changed < 0.35
        if self.selected == 0:
            return Lamp(Light((x, y), a, b, warm, self.value(2), 0.48)), False
        if self.selected == 1:
            return Flashlight(
                Light((x + 33, y + 44), 56, 7, warm, 2.6 * level, b), (x, y), a
            ), moving
        if self.selected == 2:
            return GodRays(Light((x, y), 35, 25, warm, a * level, 0.33), b), moving
        if self.selected == 3:
            return Glow(COLORS[int(a)], b, self.value(2) * level), moving
        if self.selected == 4:
            return Fire(a, b, self.value(2), now if b else 0.0, 19), b > 0
        wave = (
            Wave(self.wave[0], now - self.wave[1])
            if self.wave and now - self.wave[1] < 1
            else None
        )
        return LiquidGlass(
            (x, y), (80, 42), a, b, self.value(2), wave
        ), wave is not None

    def press(self, p: XY) -> None:
        self.tap = p
        x, y = p
        dx, dy = x - self.position[0], y - self.position[1]
        if self.selected == 0 and math.hypot(dx, dy) <= max(self.value(1), 13):
            self.grab = (dx, dy)
        elif self.selected == 5:
            if math.hypot(max(0.0, abs(dx) - 19), dy) <= 21:
                self.grab = (dx, dy)

    def click(self, now: float) -> None:
        p, self.tap = self.tap, None
        if p is None:
            return
        if self.selected == 5:
            self.wave = (p, now)
        elif self.selected == 3 and 24 <= p[0] <= 151 and 36 <= p[1] <= 108:
            self.fade(not self.target, now)


def make_ui() -> Ui:
    lab = Lab()

    def render(s: Lab, f: Frame) -> None:
        f.box(Rect(0, 0, f.width, f.height), bg=0x18202AFF)
        page = s.pages[s.selected]
        x = 16.0

        def select(i: int) -> None:
            if s.grab is not None:
                f.ui.capture(False)
            s.select(i)
            f.notify()

        compact = f.width < 520
        for i, p in enumerate(s.pages):
            if compact and i == 3:
                x = 16.0
            width = 19 + len(p["name"]) * 7.0

            def choose(_: Any, index: int = i) -> None:
                select(index)

            f.button(
                f"scene-{i}",
                Rect(x, 55 if compact and i >= 3 else 16, width, 30),
                p["name"],
                base=0x31565CFF if i == s.selected else 0x28313BFF,
                on_click=choose,
            )
            x += width + 5
        height = max(1.0, min((f.width - 32) / 2, f.height - (191 if compact else 152)))
        stage = Rect(16, 94 if compact else 55, height * 2, height)

        def press(e: PointerEvent) -> None:
            s.press(
                (
                    (e.position.x - stage.x) * 288 / stage.width,
                    (e.position.y - stage.y) * 144 / stage.height,
                ),
            )
            if s.grab is not None:
                f.ui.capture()
            f.notify()

        def click(_: PointerEvent) -> None:
            s.click(f.ui.elapsed())
            f.notify()

        hovered = f.region("stage", stage, on_mouse_down=press, on_click=click).hovered
        if s.selected in (1, 2) and hovered != s.hovered:
            s.fade(hovered, f.elapsed)
        s.hovered = hovered
        effect, moving = s.params(f.elapsed)
        f.effect_canvas("stage", stage, s.art[s.selected], effect)
        if moving:
            f.request_frame_after(0.056)
        y = stage.y + stage.height + 9
        f.label(page["hint"], Rect(16, y, f.width - 32, 20), size=13, color=0xC6CDD3FF)
        x = 16.0
        for i, k in enumerate(page["controls"]):
            value = s.value(i)
            label = (
                COLOR_NAMES[int(value)] if s.selected == 3 and i == 0 else f"{value:g}"
            )
            label = f"{k['label']}  {label}  ›"
            width = 18 + len(label) * 7.0

            def cycle(_: Any, index: int = i) -> None:
                indices = s.indices[s.selected]
                indices[index] = (indices[index] + 1) % len(
                    page["controls"][index]["values"]
                )
                f.notify()

            f.button(
                f"knob-{i}",
                Rect(x, y + 29, width, 30),
                label,
                on_click=cycle,
                base=0x28313BFF,
            )
            x += width + 6

        grip = Rect(max(0.0, f.width - 20), max(0.0, f.height - 20), 18, 18)
        f.label("◢", grip, size=13, color=0x849499FF)
        f.resize_region("resize", grip, 12, "resize-down-right")

    ui = Ui(lab, render, width=624, height=442, title="Effects lab")

    def motion(e: PointerEvent) -> None:
        b = ui.bounds_of("stage")
        if b is None:
            return
        x, y = (
            (e.position.x - b.x) * 288 / b.width,
            (e.position.y - b.y) * 144 / b.height,
        )
        if lab.tap is not None and math.hypot(x - lab.tap[0], y - lab.tap[1]) > 2:
            lab.tap = None
        if lab.grab is not None:
            x -= lab.grab[0]
            y -= lab.grab[1]
        elif lab.selected not in (1, 2) or not (0 <= x < 288 and 0 <= y < 144):
            return
        lab.position = (max(0.0, min(288.0, x)), max(0.0, min(144.0, y)))
        ui.notify()

    def release(_: PointerEvent) -> None:
        if lab.grab is not None:
            lab.grab = None
            ui.capture(False)
            ui.notify()

    ui.on_mouse_move, ui.on_mouse_up = motion, release
    return ui


if __name__ == "__main__":
    make_ui().run(duration())
