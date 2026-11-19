//! Draws a hand-made picture (shapes, text in both typefaces, icons, a clip) to a
//! PNG, to check the renderer on its own. Read-only: `cargo run --example draw_probe -- out.png`.

#[cfg(windows)]
mod probe {
    use std::time::Duration;

    use purifex_domain::draw::{Color, DrawList, Font, Op, Rect, TextStyle};
    use purifex_domain::ports::TextMetrics;
    use purifex_domain::ui_event::{Reaction, UiEvent, UiHandler};

    const MINT: Color = Color::rgb(0, 242, 170);
    const WHITE: Color = Color::rgb(242, 242, 242);

    struct Probe;

    impl UiHandler for Probe {
        fn frame(&mut self, _w: f32, _h: f32, text: &dyn TextMetrics, _now: Duration) -> DrawList {
            let mut list = DrawList::new(Color::rgb(7, 8, 11));
            card(&mut list);
            texts(&mut list, text);
            icons(&mut list);
            button_and_clip(&mut list);
            list
        }
        fn animating(&self) -> bool {
            false
        }
        fn set_reduced_motion(&mut self, _: bool) {}
        fn event(&mut self, _e: UiEvent) -> Reaction {
            Reaction::default()
        }
        fn tick(&mut self) -> Reaction {
            Reaction::default()
        }
        fn hand_cursor(&self) -> bool {
            false
        }
    }

    /// A filled and outlined rounded rectangle.
    fn card(list: &mut DrawList) {
        let rect = Rect::new(20.0, 20.0, 360.0, 180.0);
        list.push(Op::Fill {
            rect,
            radius: 8.0,
            color: Color::rgb(13, 15, 21),
        });
        list.push(Op::Stroke {
            rect,
            radius: 8.0,
            width: 1.0,
            color: Color::rgb(30, 32, 38),
        });
    }

    fn text_op(x: f32, y: f32, text: String, style: TextStyle, max_width: Option<f32>) -> Op {
        Op::Text {
            x,
            y,
            text,
            style,
            max_width,
        }
    }

    /// Text in both typefaces: a title, its measured size, a tracked label and a long
    /// line cut to a width.
    fn texts(list: &mut DrawList, text: &dyn TextMetrics) {
        let title = TextStyle::new(Font::Sans, 28.0, WHITE).bold();
        let mono = TextStyle::new(Font::Mono, 13.0, Color::rgb(154, 163, 178));
        let label = TextStyle::new(Font::Mono, 10.5, Color::rgb(94, 104, 118)).tracked(1.4);
        let size = text.measure("Estado de tu PC", &title);
        list.push(text_op(40.0, 40.0, "Estado de tu PC".into(), title, None));
        let measured = format!("medido: {:.0} x {:.0}", size.w, size.h);
        list.push(text_op(40.0, 40.0 + size.h + 6.0, measured, mono, None));
        list.push(text_op(
            40.0,
            120.0,
            "ETIQUETA CON TRACKING".into(),
            label,
            None,
        ));
        let long = "Un texto muy largo que no cabe en su columna y debe terminar en puntos";
        list.push(text_op(40.0, 145.0, long.into(), mono, Some(300.0)));
    }

    /// A row of icons, alternating colours.
    fn icons(list: &mut DrawList) {
        let names = [
            "zap",
            "gauge",
            "layers",
            "gamepad-2",
            "search",
            "bluetooth",
            "hard-drive",
            "monitor",
        ];
        for (i, name) in names.iter().enumerate() {
            list.push(Op::Icon {
                name,
                x: 40.0 + i as f32 * 40.0,
                y: 230.0,
                size: 24.0,
                color: if i % 2 == 0 { MINT } else { WHITE },
            });
        }
    }

    /// A button with an icon, a red bar cut by a clip, and a line.
    fn button_and_clip(list: &mut DrawList) {
        list.push(Op::Fill {
            rect: Rect::new(420.0, 20.0, 160.0, 44.0),
            radius: 8.0,
            color: MINT,
        });
        list.push(Op::Icon {
            name: "refresh-cw",
            x: 436.0,
            y: 34.0,
            size: 16.0,
            color: Color::rgb(0, 17, 11),
        });
        list.push(Op::Clip {
            rect: Rect::new(420.0, 80.0, 100.0, 40.0),
        });
        list.push(Op::Fill {
            rect: Rect::new(400.0, 70.0, 300.0, 80.0),
            radius: 0.0,
            color: Color::rgb(244, 63, 94),
        });
        list.push(Op::Unclip);
        list.push(Op::Line {
            x1: 420.0,
            y1: 160.0,
            x2: 580.0,
            y2: 160.0,
            width: 1.0,
            color: MINT,
        });
    }

    pub fn main() {
        let out = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "draw_probe.png".into());
        match purifex_platform::render_png(
            std::path::Path::new(&out),
            620.0,
            280.0,
            2.0,
            &mut Probe,
        ) {
            Ok(()) => println!("{out}"),
            Err(error) => {
                println!("error: {error}");
                std::process::exit(1);
            }
        }
    }
}

#[cfg(windows)]
fn main() {
    probe::main();
}

/// The renderer only exists on Windows.
#[cfg(not(windows))]
fn main() {}
