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

#[test]
fn flames_and_plasma_share_a_bounded_replayable_vector_frame() {
    use vlib::vui::effects::vector::{self, Canvas, Rect, RoundedBox};
    let frame = |time, embers, radius| {
        let mut c = Canvas::new();
        vector::fire(
            &mut c,
            Rect::new(400., 20., 400., 140.).unwrap(),
            [417., 119.],
            143.,
            75.,
            embers,
            time,
        )
        .unwrap();
        vector::plasma(
            &mut c,
            RoundedBox::new([590., 89., 88., 25.], radius),
            3.1,
            1.,
            time,
        )
        .unwrap();
        c.encode().unwrap()
    };
    let active = frame(0.6, 1., 12.5);
    assert!(
        active.len() + 12 < 256 * 1024,
        "must fit one native overlay record"
    );
    assert_eq!(active, frame(0.6, 1., 12.5));
    assert_ne!(active, frame(0.75, 1., 12.5));
    assert_ne!(active, frame(0.6, 0., 12.5));
    assert_ne!(active, frame(0.6, 1., 0.));
    let mut c = Canvas::new();
    vector::plasma(&mut c, RoundedBox::new([0., 0., 30., 30.], 15.), 2., 0., 0.).unwrap();
    assert!(c.commands().is_empty());
}

/// A second application can replay arbitrary vector content through a lens at any origin.
#[test]
fn vector_glass_replays_custom_artwork_and_releases_its_clip() {
    use vivid_protocol::vector::Command;
    use vlib::vui::effects::vector::{
        self, Brush, Canvas, Glass, InvalidScene, Path, Rect, RoundedBox, Scene,
    };

    let bounds = Rect::new(480., 160., 60., 40.).unwrap();
    let scene = Scene {
        bounds,
        surfaces: &[],
    };
    let lens = RoundedBox::new([510., 180., 5., 3.], 0.);
    let glass = Glass {
        lens,
        depth: 8.,
        dispersion: 0.2,
        frost: 0.3,
        wave: Some(Wave {
            center: [496., 171.],
            age: 0.4,
        }),
    };
    let artwork = Path::builder()
        .move_to(480., 160.)
        .line_to(530., 188.)
        .line_to(500., 192.)
        .close()
        .build()
        .unwrap();
    let mut canvas = Canvas::new();
    vector::liquid_glass(&mut canvas, scene, glass, |c| {
        c.fill(artwork.clone(), Brush::Solid(Color(0x4b99dfff)))?;
        Ok(())
    })
    .unwrap();
    canvas.validate().unwrap();
    assert_eq!(
        canvas
            .commands()
            .iter()
            .filter(|cmd| matches!(cmd,Command::Fill(path,_) if path==&artwork))
            .count(),
        4
    );
    let expected = Rect::new(510., 180., 5., 3.).unwrap();
    assert!(
        canvas
            .commands()
            .iter()
            .any(|cmd| matches!(cmd,Command::Shadow(s) if s.rect==expected))
    );

    let transparent = Surface {
        rect: [510., 180., 5., 3.],
        radius: 1.,
        color: Color(0xff000000),
        elevation: 0,
        emitter: false,
    };
    let mut covered = Canvas::new();
    vector::liquid_glass(
        &mut covered,
        Scene {
            surfaces: &[transparent],
            ..scene
        },
        glass,
        |c| {
            c.fill(artwork.clone(), Brush::Solid(Color(0x4b99dfff)))?;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(
        canvas, covered,
        "transparent surfaces must not tint the lens"
    );

    let mut failed = Canvas::new();
    let err = InvalidScene::new("artwork failed");
    assert_eq!(
        vector::liquid_glass(&mut failed, scene, glass, |_| Err(err)),
        Err(err)
    );
    failed.validate().unwrap();
}

#[cfg(feature = "testing")]
mod ui {
    use super::lab::Lab;
    use std::time::Duration;
    use vivid_protocol::vector::Command;
    use vlib::vui::testing::TestUi;

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
    fn send_reaches_the_inbox_and_charging_the_button_does_not_zap_it() {
        let mut ui = TestUi::start(Lab::new().unwrap(), 460., 322.).unwrap();
        ui.focus_window().unwrap();
        ui.click("scene-6").unwrap();
        let b = ui.bounds("stage").unwrap();
        let local = |x: f64, y: f64| {
            (
                f64::from(b.origin.x.0) + x * f64::from(b.width().0) / 288.,
                f64::from(b.origin.y.0) + y * f64::from(b.height().0) / 144.,
            )
        };
        let click = |ui: &mut TestUi<Lab>, (x, y): (f64, f64)| {
            for down in [true, false] {
                ui.presenter()
                    .overlay_pointer(x, y, Some((0, down)), u32::from(down))
                    .unwrap();
            }
            ui.settle().unwrap();
        };
        let clips = |ui: &TestUi<Lab>| {
            ui.canvas()
                .commands()
                .iter()
                .filter(|c| matches!(c, Command::Clip(_)))
                .count()
        };
        let base = clips(&ui);
        click(&mut ui, local(54., 100.));
        ui.advance(Duration::from_millis(150)).unwrap();
        assert_eq!(clips(&ui), base);
        ui.advance(Duration::from_millis(400)).unwrap();
        assert_eq!(
            clips(&ui),
            base + 1,
            "the arriving ripple is clipped to the inbox"
        );
        ui.click("scene-7").unwrap();
        let charge = |ui: &TestUi<Lab>| {
            ui.canvas()
                .commands()
                .iter()
                .any(|c| matches!(c,Command::Shadow(s) if s.color.0>>8==0x5fceff))
        };
        let button = local(207., 100.);
        ui.move_pointer(button.0, button.1).unwrap();
        ui.advance(Duration::from_millis(400)).unwrap();
        assert!(charge(&ui));
        click(&mut ui, button);
        assert_eq!(ui.read(|s| s.shots.len()), 0);
        let outside = local(102., 82.);
        ui.move_pointer(outside.0, outside.1).unwrap();
        click(&mut ui, outside);
        assert_eq!(ui.read(|s| s.shots.len()), 1);
        ui.advance(Duration::from_millis(500)).unwrap();
        assert!(!charge(&ui));
        assert_eq!(ui.read(|s| s.shots.len()), 0);
    }

    #[test]
    fn thermal_speed_changes_preserve_phase_and_zero_sleeps() {
        let mut ui = TestUi::start(Lab::new().unwrap(), 760., 510.).unwrap();
        ui.click("scene-4").unwrap();
        ui.advance(Duration::from_millis(160)).unwrap();
        // Controls change their labels. The native transforms and strokes identify both effects.
        let geometry = |ui: &TestUi<Lab>| {
            ui.canvas()
                .commands()
                .iter()
                .filter(|cmd| matches!(cmd, Command::Transform(_) | Command::Stroke(..)))
                .cloned()
                .collect::<Vec<_>>()
        };
        let moving = geometry(&ui);
        for _ in 0..3 {
            ui.click("knob-1").unwrap();
            assert_eq!(moving, geometry(&ui));
        }
        ui.advance(Duration::from_secs(3)).unwrap();
        assert_eq!(moving, geometry(&ui));
        assert!(ui.wake_in().is_none());
        ui.click("knob-1").unwrap();
        assert_eq!(moving, geometry(&ui));
        ui.advance(Duration::from_millis(100)).unwrap();
        assert_ne!(moving, geometry(&ui));
    }

    #[test]
    fn gallery_draws_scalable_geometry_and_animates_only_active_scenes() {
        let mut ui = TestUi::start(Lab::new().unwrap(), 760., 510.).unwrap();
        let initial = ui.canvas();
        let scale = ui.bounds("stage").unwrap().width().0 as f64 / 288.;
        assert!(
            initial
                .commands()
                .iter()
                .any(|c| matches!(c,Command::Transform(t) if (t.0[0].get()-scale).abs()<1e-6))
        );
        ui.advance(Duration::from_secs(1)).unwrap();
        assert_eq!(initial, ui.canvas());
        assert!(ui.wake_in().is_none());
        for i in 0..8 {
            ui.click(&format!("scene-{i}")).unwrap();
            assert_eq!(ui.read(|s| s.selected), i);
            ui.click("knob-0").unwrap();
            assert_eq!(ui.retained_images(), 0);
            let before = ui.canvas();
            assert!(
                !before
                    .commands()
                    .iter()
                    .any(|c| matches!(c, Command::Image { .. }))
            );
            if matches!(i, 4 | 6 | 7) {
                assert!(ui.wake_in().is_some());
                ui.advance(Duration::from_millis(125)).unwrap();
                assert_ne!(before, ui.canvas());
            }
            if i == 5 {
                assert!(ui.wake_in().is_none());
                ui.click("stage").unwrap();
                assert!(ui.wake_in().is_some());
                ui.advance(Duration::from_secs(2)).unwrap();
                assert!(ui.wake_in().is_none());
            }
            if i >= 6 {
                for _ in 0..10 {
                    ui.click("stage").unwrap();
                }
                assert_eq!(ui.read(|s| s.shots.len()), 8);
                ui.advance(Duration::from_secs(2)).unwrap();
                assert_eq!(ui.read(|s| s.shots.len()), 0);
            }
        }
    }
}
#[cfg(feature = "testing")]
#[path = "../examples/views/effects_lab.rs"]
mod lab;
