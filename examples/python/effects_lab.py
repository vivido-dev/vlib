"""An interactive gallery of eight resolution-independent vector effects."""

from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

from vlib.vui import Frame, PointerEvent, Rect, Ui, duration
from lab.stage import Shot, XY, draw, scene_for

COLOR_NAMES = ("Mint", "Rose", "Amber", "Blue")


def charged(p: XY) -> bool:
    return 165 <= p[0] <= 249 and 90 <= p[1] <= 110


class Lab:
    def __init__(self) -> None:
        path = Path(__file__).resolve().parents[1] / "assets" / "effects_lab.json"
        self.pages: List[Dict[str, Any]] = json.loads(path.read_text())
        self.scenes = [scene_for(page) for page in self.pages]
        self.shots: List[Shot] = []
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
        self.shots.clear()
        self.start = self.target = 0.0 if index == 7 else 1.0
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
        elif self.selected == 6 or (self.selected == 7 and not charged(p)):
            self.shots = self.shots[-7:] + [(p, now)]
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

        columns = 8 if f.width >= 720 else (4 if f.width >= 354 else 2)
        rows = math.ceil(len(s.pages) / columns)
        for i, p in enumerate(s.pages):
            if i % columns == 0:
                x = 16.0
            width = 19 + len(p["name"]) * 7.0

            def choose(_: Any, index: int = i) -> None:
                select(index)

            f.button(
                f"scene-{i}",
                Rect(x, 16 + 39 * (i // columns), width, 30),
                p["name"],
                base=0x31565CFF if i == s.selected else 0x28313BFF,
                on_click=choose,
            )
            x += width + 5
        height = max(1.0, min((f.width - 32) / 2, f.height - (152 + 39 * (rows - 1))))
        stage = Rect(16, 55 + 39 * (rows - 1), height * 2, height)

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
        if s.selected == 7 and not hovered:
            s.fade(False, f.elapsed)
        now = f.elapsed
        wave = s.wave if s.wave and now - s.wave[1] < 1 else None
        s.shots = [
            sh for sh in s.shots if now - sh[1] < (1.4 if s.selected == 6 else 0.42)
        ]
        knobs = [s.value(i) for i in range(len(page["controls"]))]
        if len(knobs) == 2:
            knobs.append(0.0)
        draw(
            f.canvas,
            stage,
            page,
            s.selected,
            s.position,
            knobs,
            s.level(now),
            now,
            wave,
            s.shots,
            s.scenes[s.selected],
        )
        if (
            (s.start != s.target and now - s.changed < 0.35)
            or (s.selected == 4 and knobs[1] > 0)
            or s.selected >= 6
            or wave
        ):
            f.request_frame_after(1 / 30)
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

    ui = Ui(lab, render, width=760, height=510, title="Effects lab")

    def motion(e: PointerEvent) -> None:
        b = ui.bounds_of("stage")
        if b is None:
            return
        x, y = (
            (e.position.x - b.x) * 288 / b.width,
            (e.position.y - b.y) * 144 / b.height,
        )
        if lab.selected == 7:
            lab.fade(charged((x, y)), ui.elapsed())
            ui.notify()
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
