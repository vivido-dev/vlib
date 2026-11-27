//! Cross-language reference pixels and behavioral checks for the software canvas.

// Rust guideline compliant 2026-10-07
use vlib::vui::{
    Color,
    effects::{Effect, EffectScene, Light, Surface, Wave},
};

const W: usize = 48;
const H: usize = 32;
fn scene(blocker: bool) -> EffectScene {
    let backdrop: Vec<_> = (0..W * H)
        .flat_map(|i| {
            [
                if (i % W / 4).is_multiple_of(2) {
                    25
                } else {
                    80
                },
                50,
                70,
                255,
            ]
        })
        .collect();
    let surface = Surface {
        rect: [18., 12., 10., 12.],
        radius: 2.,
        color: Color(0x708878ff),
        elevation: if blocker { 14 } else { 0 },
        emitter: true,
    };
    EffectScene::new(
        [48., 32.],
        [W, H],
        Color(0),
        Some(&backdrop),
        &[surface],
        None,
    )
    .unwrap()
}
fn light() -> Light {
    Light {
        position: [7., 8.],
        height: 18.,
        radius: 3.,
        color: [255, 204, 155],
        intensity: 1.6,
        ambient: 0.35,
    }
}
fn effects() -> [Effect; 6] {
    [
        Effect::Lamp(light()),
        Effect::Flashlight {
            light: light(),
            aim: [26., 20.],
            beam: 0.45,
        },
        Effect::GodRays {
            light: light(),
            softness: 2.,
        },
        Effect::Glow {
            color: [64, 213, 249],
            radius: 5.,
            intensity: 0.9,
        },
        Effect::Fire {
            height: 14.,
            speed: 1.4,
            embers: 0.8,
            time: 0.6,
            seed: 17,
        },
        Effect::LiquidGlass {
            center: [25., 17.],
            size: [22., 14.],
            depth: 8.,
            dispersion: 0.35,
            frost: 0.15,
            wave: Some(Wave {
                center: [5., 15.],
                age: 0.3,
            }),
        },
    ]
}
fn pixel(frame: &[u8], x: usize, y: usize) -> &[u8] {
    let i = (y * W + x) * 4;
    &frame[i..i + 4]
}

#[test]
fn all_ports_share_the_same_reference_pixels() {
    let s = scene(true);
    let probes: Vec<Vec<(usize, usize, [u8; 4])>> =
        serde_json::from_str(include_str!("fixtures/effect_probes.json")).unwrap();
    assert_eq!(probes.len(), 6);
    for (effect, case) in effects().into_iter().zip(probes) {
        let frame = s.render(effect).unwrap();
        for (x, y, expected) in case {
            let actual = pixel(&frame, x, y);
            assert!(
                actual.iter().zip(expected).all(|(v, e)| v.abs_diff(e) <= 1),
                "{effect:?} at {x},{y}: {actual:?} vs {expected:?}"
            );
        }
    }
}

#[test]
fn blockers_cast_shadows_and_interrupt_shafts() {
    let raised = scene(true);
    let flat = scene(false);
    for (source, near, far) in [(4., 13, 32), (43., 32, 13)] {
        let light = Light {
            position: [source, 18.],
            ..light()
        };
        for effect in [
            Effect::Lamp(light),
            Effect::GodRays {
                light,
                softness: 1.,
            },
        ] {
            let a = raised.render(effect).unwrap();
            let b = flat.render(effect).unwrap();
            assert!(pixel(&a, far, 18)[0] + 8 < pixel(&b, far, 18)[0]);
            assert_eq!(pixel(&a, near, 18), pixel(&b, near, 18));
        }
    }
}

#[test]
fn moving_fire_and_glass_sample_the_scene() {
    let scene = scene(true);
    let fire = |time| Effect::Fire {
        height: 14.,
        speed: 1.4,
        embers: 0.8,
        time,
        seed: 17,
    };
    assert_eq!(
        scene.render(fire(0.6)).unwrap(),
        scene.render(fire(0.6)).unwrap()
    );
    assert_ne!(
        scene.render(fire(0.6)).unwrap(),
        scene.render(fire(0.9)).unwrap()
    );
    let lens = |x| Effect::LiquidGlass {
        center: [x, 17.],
        size: [22., 14.],
        depth: 8.,
        dispersion: 0.35,
        frost: 0.,
        wave: None,
    };
    let a = scene.render(lens(25.)).unwrap();
    let b = scene.render(lens(8.)).unwrap();
    assert_ne!(pixel(&a, 33, 16), pixel(&b, 33, 16));
    assert_eq!(pixel(&a, 45, 4), pixel(&b, 45, 4));
}

#[test]
fn alpha_masks_preserve_strength_and_invalid_inputs_are_recoverable() {
    let glow = Effect::Glow {
        radius: 3.,
        intensity: 1.,
        color: [120, 220, 180],
    };
    let halo = |a| {
        let mut mask = [0; 24];
        mask[11] = a;
        let s = EffectScene::new([24., 1.], [24, 1], Color(0), None, &[], Some(&mask)).unwrap();
        s.render(glow).unwrap()[10 * 4 + 3]
    };
    assert_eq!(halo(0), 0);
    assert!(halo(200) > halo(20));
    assert!(EffectScene::new([8., 8.], [usize::MAX, 2], Color(0), None, &[], None).is_err());
    assert!(EffectScene::new([8., 8.], [8, 8], Color(0), Some(&[]), &[], None).is_err());
    assert!(
        scene(true)
            .render(Effect::Lamp(Light {
                radius: f64::NAN,
                ..light()
            }))
            .is_err()
    );
}

#[test]
fn dispersion_does_not_reveal_rgb_from_transparent_artwork() {
    let lens = Effect::LiquidGlass {
        center: [12., 8.],
        size: [20., 14.],
        depth: 5.,
        dispersion: 1.,
        frost: 0.3,
        wave: None,
    };
    let render = |hidden| {
        let rgba: Vec<_> = (0usize..24 * 16)
            .flat_map(|i| {
                if (i % 24).is_multiple_of(3) {
                    [hidden, 0, hidden, 0]
                } else {
                    [32, 64, 96, 255]
                }
            })
            .collect();
        EffectScene::new([24., 16.], [24, 16], Color(0), Some(&rgba), &[], None)
            .unwrap()
            .render(lens)
            .unwrap()
    };
    let a = render(0);
    let b = render(255);
    for (a, b) in a.as_chunks::<4>().0.iter().zip(b.as_chunks::<4>().0) {
        if a[3] > 0 {
            assert_eq!(a, b);
        }
    }
}

#[cfg(feature = "testing")]
mod ui {
    use super::lab::Lab;
    use std::time::Duration;
    use vivid_protocol::vector::Command;
    use vlib::vui::testing::TestUi;

    fn asset(ui: &TestUi<Lab>) -> u64 {
        ui.canvas()
            .commands()
            .iter()
            .find_map(|c| {
                if let Command::Image { asset, .. } = c {
                    Some(*asset)
                } else {
                    None
                }
            })
            .unwrap()
    }
    #[test]
    fn a_captured_lamp_drag_uses_stage_coordinates_at_both_sizes() {
        for (width, height) in [(624., 442.), (460., 322.)] {
            let mut ui = TestUi::start(Lab::new().unwrap(), width, height).unwrap();
            ui.focus_window().unwrap();
            let stage = ui.bounds("stage").unwrap();
            let local = |x, y| {
                (
                    f64::from(stage.origin.x.0) + x * f64::from(stage.width().0) / 288.,
                    f64::from(stage.origin.y.0) + y * f64::from(stage.height().0) / 144.,
                )
            };
            let start = local(232., 36.);
            ui.presenter()
                .overlay_pointer(start.0, start.1, Some((0, true)), 1)
                .unwrap();
            ui.settle().unwrap();
            let outside = local(300., 72.);
            ui.move_pointer(outside.0, outside.1).unwrap();
            assert_eq!(ui.read(|s| s.position[0]), 288.);
            ui.presenter()
                .overlay_pointer(outside.0, outside.1, Some((0, false)), 0)
                .unwrap();
            ui.settle().unwrap();
            let inside = local(50., 50.);
            ui.move_pointer(inside.0, inside.1).unwrap();
            assert_eq!(ui.read(|s| s.position[0]), 288.);
        }
    }

    #[test]
    fn gallery_switches_and_replaces_only_the_visible_canvas() {
        let mut ui = TestUi::start(Lab::new().unwrap(), 624., 442.).unwrap();
        let first = asset(&ui);
        ui.advance(Duration::from_secs(1)).unwrap();
        assert_eq!(first, asset(&ui));
        assert!(ui.wake_in().is_none());
        for i in 1..6 {
            ui.click(&format!("scene-{i}")).unwrap();
            assert_eq!(ui.read(|s| s.selected), i);
            ui.click("knob-0").unwrap();
            assert_eq!(ui.retained_images(), 1);
            if i == 4 {
                let mut previous = asset(&ui);
                for _ in 0..12 {
                    ui.advance(Duration::from_millis(80)).unwrap();
                    assert_ne!(asset(&ui), previous);
                    previous = asset(&ui);
                    assert_eq!(ui.retained_images(), 1);
                }
            }
        }
        assert!(ui.wake_in().is_none());
        ui.click("stage").unwrap();
        assert!(ui.wake_in().is_some());
        ui.advance(Duration::from_secs(2)).unwrap();
        assert!(ui.wake_in().is_none());
    }
}
#[cfg(feature = "testing")]
#[path = "../examples/views/effects_lab.rs"]
mod lab;
