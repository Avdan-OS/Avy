use std::{mem, sync::mpsc::RecvTimeoutError, thread::spawn, time::Duration};

use avy_render::{
    graphics::vulkan::Vulkan,
    util::Size,
    wayland::surface::layer::{AvyLayer, AvyLayerParams},
    AvyClient,
};

use skia_safe::{Color4f, Paint};
use smithay_client_toolkit::{
    reexports::{
        calloop::EventLoop,
        calloop_wayland_source::WaylandSource,
        client::{globals::registry_queue_init, Connection},
    },
    shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer},
};
use vulkano::Version;

const INIT_WIDTH: u32 = 1920;
const INIT_HEIGHT: u32 = 60;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut event_queue) = registry_queue_init::<AvyClient>(&conn).unwrap();
    let qh = event_queue.handle();

    let mut app = AvyClient::new(&globals, &qh, (INIT_WIDTH, INIT_HEIGHT), conn.display())?;
    let vulkan = Vulkan::new("Demo", Version::major_minor(0, 1))?;

    event_queue.roundtrip(&mut app).unwrap();

    let size = app
        .output_state
        .outputs()
        .next()
        .and_then(|wl_output| {
            app.output_state
                .info(&wl_output)
                .and_then(|info| info.logical_size.map(|(w, h)| (w as u32, h as u32)))
        })
        .unwrap_or((INIT_WIDTH, INIT_HEIGHT));

    let surface = AvyLayer::build(
        &mut app,
        &mut event_queue,
        AvyLayerParams {
            layer: Layer::Top,
            namespace: Some("demo"),
            output: None,
            anchor: Anchor::BOTTOM,
            size: Size::new((size.0, INIT_HEIGHT)),
            margin: None,
            keyboard_interactivity: KeyboardInteractivity::OnDemand,
        },
    )
    .make_backend(&vulkan)?;

    let mut event_loop = EventLoop::<AvyClient>::try_new()?;
    WaylandSource::new(conn, event_queue).insert(event_loop.handle())?;

    let fonts = skia_safe::FontMgr::new();
    let inter = fonts
        .match_family_style("Inter", skia_safe::FontStyle::bold())
        .expect("Inter bold");

    let inter_50pt = skia_safe::Font::from_typeface(inter.clone(), Some(50.0));

    let (tx, rx) = std::sync::mpsc::channel::<()>();

    spawn(move || {
        // From https://x.com/notargs/status/1250468645030858753 -- Thank you!
        let shader = skia_safe::RuntimeEffect::make_for_shader(
            r#"
uniform float iTime;
uniform float2 iResolution;
float f(vec3 p) {
    p.z -= iTime * 10.;
    float a = p.z * .1;
    p.xy *= mat2(cos(a), sin(a), -sin(a), cos(a));
    return .1 - length(cos(p.xy) + sin(p.yz));
}

half4 main(vec2 fragcoord) { 
    vec3 d = .5 - fragcoord.xy1 / iResolution.y;
    vec3 p=vec3(0);
    for (int i = 0; i < 32; i++) {
      p += f(p) * d;
    }
    return ((sin(p) + vec3(2, 5, 12)) / length(p)).xyz1;
}
"#,
            None,
        );

        let runtime_effect = match shader {
            Ok(shader) => shader,
            Err(err) => panic!("{err}"),
        };

        #[allow(non_snake_case, unused)]
        #[repr(packed)]
        struct _Uniforms {
            iTime: f32,
            iResolution: [f32; 2],
        }

        impl _Uniforms {
            fn make_shader(
                &self,
                runtime_effect: &skia_safe::RuntimeEffect,
            ) -> Option<skia_safe::Shader> {
                const SIZE: usize = mem::size_of::<_Uniforms>();

                let data = unsafe {
                    let bytes = core::slice::from_raw_parts(self as *const _ as *const u8, SIZE);
                    skia_safe::Data::new_bytes(bytes)
                };

                runtime_effect.make_shader(data, &[], None)
            }
        }

        let mut uniforms = _Uniforms {
            iTime: 0.0,
            iResolution: [size.0 as f32, size.1 as f32],
        };

        let time = std::time::Instant::now();
        let mut frames = 0;

        let black = Paint::new(Color4f::new(0.1, 0.1, 0.1, 1.0), None);

        let width_of = |s: &str| {
            let mut bounds = vec![Default::default(); s.len()];
            inter_50pt.get_widths(&inter_50pt.str_to_glyphs_vec(s), &mut bounds);
            bounds.iter().sum::<f32>() as i32
        };

        loop {
            let time = time.elapsed();
            if time > Duration::from_secs(20) {
                break;
            }

            uniforms.iTime = time.as_secs_f32() / 15.0;

            // let std::ops::CoroutineState::Yielded(color) = rainbow.as_mut().resume(()) else {
            //     panic!("Why is it finished?");
            // };

            let shader = uniforms.make_shader(&runtime_effect).unwrap();
            let mut shader_paint = Paint::new(Color4f::new(0.0, 0.0, 0.0, 1.0), None);
            shader_paint.set_shader(shader);

            // let color: Rgb = color.into_color();
            // let (r, g, b) = color.into_format::<u8>().into_components();

            // let mut color = Paint::default();
            // color.set_color(skia_safe::Color::from_rgb(r, g, b));
            // color.set_anti_alias(true);

            surface
                .render(|canvas| {
                    // canvas.draw_text_align(
                    //     "Welcome to AvdanOS",
                    //     (1700, 50),
                    //     &inter_50pt,
                    //     &shader_paint,
                    //     skia_bindings::SkTextUtils_Align::Right,
                    // );

                    canvas.draw_text_align(
                        format!("{:.2}", time.as_secs_f64()),
                        (0, 50),
                        &inter_50pt,
                        &black,
                        skia_bindings::SkTextUtils_Align::Left,
                    );

                    let left = 150;
                    canvas.draw_text_align(
                        "It's",
                        (left, 50),
                        &inter_50pt,
                        &black,
                        skia_bindings::SkTextUtils_Align::Left,
                    );

                    canvas.draw_text_align(
                        "shader",
                        (left + width_of("It's "), 50),
                        &inter_50pt,
                        &shader_paint,
                        skia_bindings::SkTextUtils_Align::Left,
                    );

                    canvas.draw_text_align(
                        "time at ",
                        (left + width_of("It's ") + width_of("shader "), 50),
                        &inter_50pt,
                        &black,
                        skia_bindings::SkTextUtils_Align::Left,
                    );

                    canvas.draw_text_align(
                        "Avy",
                        (left + width_of("It's shader time at "), 50),
                        &inter_50pt,
                        &shader_paint,
                        skia_bindings::SkTextUtils_Align::Left,
                    );

                    canvas.draw_text_align(
                        ".",
                        (left + width_of("It's shader time at Avy"), 50),
                        &inter_50pt,
                        &black,
                        skia_bindings::SkTextUtils_Align::Left,
                    );
                })
                .expect("Bad render");

            frames += 1;
        }

        println!(
            "Average FPS: {:.2}",
            frames as f64 / time.elapsed().as_secs_f64()
        );

        tx.send(()).unwrap();
    });

    loop {
        match rx.recv_timeout(Duration::from_millis(1)) {
            Ok(()) => break,
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => (),
        }
        event_loop.dispatch(Duration::from_millis(5), &mut app)?;
    }

    drop(vulkan);
    drop(app);

    Ok(())
}
