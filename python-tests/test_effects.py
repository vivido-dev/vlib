"""Reference pixels shared with Rust and TypeScript, plus window-local image ownership."""

from __future__ import annotations
import json
from pathlib import Path
from typing import Any, Dict, List
from unittest.mock import Mock
import pytest
from vlib.vui import Frame, Rect, Ui
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
    Surface,
    Wave,
)


def artwork() -> EffectScene:
    rgba = bytes(
        c
        for i in range(48 * 32)
        for c in ((25 if (i % 48 // 4) % 2 == 0 else 80), 50, 70, 255)
    )
    return EffectScene(
        (48, 32),
        (48, 32),
        0,
        [Surface((18, 12, 10, 12), 2, 0x708878FF, 14, True)],
        backdrop=rgba,
    )


def test_reference_pixels() -> None:
    light = Light((7, 8), 18, 3, (255, 204, 155), 1.6, 0.35)
    effects: List[Effect] = [
        Lamp(light),
        Flashlight(light, (26, 20), 0.45),
        GodRays(light, 2),
        Glow((64, 213, 249), 5, 0.9),
        Fire(14, 1.4, 0.8, 0.6, 17),
        LiquidGlass((25, 17), (22, 14), 8, 0.35, 0.15, Wave((5, 15), 0.3)),
    ]
    probes = json.loads(
        (
            Path(__file__).resolve().parents[1] / "tests/fixtures/effect_probes.json"
        ).read_text()
    )
    s = artwork()
    assert len(probes) == len(effects)
    for e, case in zip(effects, probes):
        pixels = s.render(e)
        for x, y, expected in case:
            k = (y * 48 + x) * 4
            assert all(abs(a - b) <= 1 for a, b in zip(pixels[k : k + 4], expected))


class Window:
    def __init__(self) -> None:
        self.held: Dict[int, bytes] = {}
        self.last = 0

    def upload_rgba(self, width: int, height: int, pixels: bytes) -> int:
        assert len(pixels) == width * height * 4
        self.last += 1
        self.held[self.last] = pixels
        return self.last

    def draw_image(self, canvas: Any, image: int, bounds: Rect, opacity: float) -> None:
        assert image in self.held

    def release_image(self, image: int) -> None:
        del self.held[image]

    def present(self, canvas: Any) -> None:
        pass


def test_window_owns_only_mounted_effects() -> None:
    s = artwork()

    def render(level: List[int], frame: Frame) -> None:
        if level[0]:
            frame.effect_canvas(
                "shared", Rect(0, 0, 48, 32), s, Glow((1, 2, 3), 4, level[0])
            )

    windows = [Window(), Window()]
    levels = [[1], [1]]
    uis = [Ui(level, render, width=48, height=32) for level in levels]
    for ui, w in zip(uis, windows):
        ui.attach(Mock(), w)
        ui.paint()
        assert list(w.held) == [1]
    levels[0][0] = 2
    uis[0].paint()
    assert list(windows[0].held) == [2]
    levels[0][0] = 0
    uis[0].paint()
    assert not windows[0].held and list(windows[1].held) == [1]
    with pytest.raises(ValueError):
        s.render(Glow((1, 2, 3), float("nan"), 1))


def test_vector_glass_replays_custom_artwork_and_releases_its_clip() -> None:
    from vivid_sdk.overlay import Brush, Canvas, Path as VectorPath
    from vlib.vui import vector_effects as fx

    scene = fx.Scene(Rect(480, 160, 60, 40))
    glass = fx.Glass(
        fx.RoundedBox((510, 180, 5, 3), 0), 8, 0.2, 0.3, Wave((496, 171), 0.4)
    )
    triangle = (
        VectorPath().move_to(480, 160).line_to(530, 188).line_to(500, 192).close()
    )
    replays = 0

    def paint(c: Canvas) -> None:
        nonlocal replays
        replays += 1
        c.fill(triangle, Brush.solid(0x4B99DFFF))

    canvas = Canvas()
    fx.liquid_glass(canvas, scene, glass, paint)
    canvas.validate()
    assert replays == 4

    def reject(_: Canvas) -> None:
        raise RuntimeError("artwork failed")

    with pytest.raises(RuntimeError, match="artwork failed"):
        fx.liquid_glass(canvas, scene, glass, reject)
    canvas.validate()
