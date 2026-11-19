//! Lucide icons as geometry (https://lucide.dev, ISC license, see
//! `crates/purifex-ui/assets/LICENSE-lucide.txt`): the SVG of each icon is read into moves, lines and
//! cubic curves (arcs and circles are converted), which any graphics library can
//! stroke. Plain code, tested without a window.

use std::collections::HashMap;
use std::sync::OnceLock;

mod data;
mod svg_path;

pub use svg_path::{path, Seg};

use data::ICONS;

/// The outline of an icon by name, parsed once.
pub fn icon(name: &str) -> Option<&'static [Seg]> {
    static PARSED: OnceLock<HashMap<&'static str, Vec<Seg>>> = OnceLock::new();
    PARSED
        .get_or_init(|| {
            ICONS
                .iter()
                .filter_map(|(name, svg)| shapes(svg).ok().map(|segs| (*name, segs)))
                .collect()
        })
        .get(name)
        .map(Vec::as_slice)
}

/// The names of all the icons.
pub fn names() -> impl Iterator<Item = &'static str> {
    ICONS.iter().map(|(name, _)| *name)
}

/// Reads the shapes inside an `<svg>`: `path`, `rect`, `circle`, `line` and
/// `polyline`, which are all the library uses.
#[expect(clippy::too_many_lines, reason = "legacy: split")]
pub fn shapes(svg_inner: &str) -> Result<Vec<Seg>, String> {
    let mut segs = Vec::new();
    for element in svg_inner.split('<').skip(1) {
        let element = element.trim_end_matches(['>', '/', ' ']);
        let (tag, attrs) = element.split_once(' ').unwrap_or((element, ""));
        let attr = |name: &str| attribute(attrs, name);
        let number = |name: &str| -> Result<f32, String> {
            attr(name)
                .ok_or_else(|| format!("<{tag}> sin «{name}»"))?
                .trim()
                .parse()
                .map_err(|_| format!("<{tag}>: «{name}» no es un número"))
        };
        let or_zero = |name: &str| attr(name).and_then(|v| v.trim().parse::<f32>().ok());
        let d = match tag {
            "path" => attr("d").ok_or("<path> sin «d»")?.to_string(),
            "circle" => {
                let (cx, cy, r) = (number("cx")?, number("cy")?, number("r")?);
                format!(
                    "M{} {}A{r} {r} 0 1 1 {} {}A{r} {r} 0 1 1 {} {}Z",
                    cx - r,
                    cy,
                    cx + r,
                    cy,
                    cx - r,
                    cy
                )
            }
            "line" => format!(
                "M{} {}L{} {}",
                number("x1")?,
                number("y1")?,
                number("x2")?,
                number("y2")?
            ),
            "polyline" => {
                let points = attr("points").ok_or("<polyline> sin «points»")?;
                format!("M{points}")
            }
            "rect" => {
                let (x, y) = (or_zero("x").unwrap_or(0.0), or_zero("y").unwrap_or(0.0));
                let (w, h) = (number("width")?, number("height")?);
                let rx = or_zero("rx").or(or_zero("ry")).unwrap_or(0.0).min(w / 2.0);
                let ry = or_zero("ry").or(or_zero("rx")).unwrap_or(0.0).min(h / 2.0);
                format!(
                    "M{} {}H{}A{rx} {ry} 0 0 1 {} {}V{}A{rx} {ry} 0 0 1 {} {}H{}A{rx} {ry} 0 0 1 {} {}V{}A{rx} {ry} 0 0 1 {} {}Z",
                    x + rx, y,
                    x + w - rx,
                    x + w, y + ry,
                    y + h - ry,
                    x + w - rx, y + h,
                    x + rx,
                    x, y + h - ry,
                    y + ry,
                    x + rx, y
                )
            }
            "" | "svg" => continue,
            other => return Err(format!("elemento no soportado: <{other}>")),
        };
        segs.extend(path(&d)?);
    }
    Ok(segs)
}

/// The value of `name="..."` in the attributes of an element.
fn attribute<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let mut from = 0;
    while let Some(found) = attrs[from..].find(&key) {
        let start = from + found;
        // A whole name: not the end of a longer one (`x` in `rx`).
        if start == 0 || attrs.as_bytes()[start - 1] == b' ' {
            let value = &attrs[start + key.len()..];
            return value.split('"').next();
        }
        from = start + key.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end_of(segs: &[Seg]) -> (f32, f32) {
        match segs.last().unwrap() {
            Seg::Move(x, y) | Seg::Line(x, y) | Seg::Cubic(_, _, _, _, x, y) => (*x, *y),
            Seg::Close => panic!("ends closed"),
        }
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn lines_and_relative_moves_are_followed() {
        let s = path("M1 2 L4 6 h2 v-3 z").unwrap();
        assert_eq!(
            s,
            [
                Seg::Move(1.0, 2.0),
                Seg::Line(4.0, 6.0),
                Seg::Line(6.0, 6.0),
                Seg::Line(6.0, 3.0),
                Seg::Close
            ]
        );
        assert_eq!(
            path("m1 1 2 2").unwrap(),
            [Seg::Move(1.0, 1.0), Seg::Line(3.0, 3.0)],
            "pairs after m are relative lines"
        );
    }

    #[test]
    fn numbers_may_touch_as_the_minified_icons_write_them() {
        assert_eq!(
            path("M1.5.5L-.5-1").unwrap(),
            [Seg::Move(1.5, 0.5), Seg::Line(-0.5, -1.0)]
        );
        assert_eq!(path("M2e1 1").unwrap(), [Seg::Move(20.0, 1.0)]);
    }

    #[test]
    fn arc_flags_may_touch_the_next_number() {
        // `a1 1 0 011 1`: radii 1 1, rotation 0, flags 0 and 1, end (1, 1).
        let s = path("M0 0a1 1 0 011 1").unwrap();
        assert_eq!(end_of(&s), (1.0, 1.0));
        assert!(s.len() >= 2);
    }

    #[test]
    fn an_arc_ends_exactly_where_it_was_told_and_is_a_quarter_turn_per_curve() {
        let half = path("M0 0A5 5 0 0 1 10 0").unwrap();
        assert_eq!(end_of(&half), (10.0, 0.0));
        assert_eq!(half.len(), 1 + 2, "a half circle is two quarter curves");
        let full_circle = shapes(r#"<circle cx="12" cy="12" r="4" />"#).unwrap();
        assert_eq!(
            full_circle
                .iter()
                .filter(|s| matches!(s, Seg::Cubic(..)))
                .count(),
            4
        );
        // The circle stays within its radius.
        for seg in &full_circle {
            if let Seg::Cubic(_, _, _, _, x, y) = seg {
                assert!(((x - 12.0).powi(2) + (y - 12.0).powi(2)).sqrt() - 4.0 < 1e-3);
            }
        }
    }

    #[test]
    fn a_radius_too_small_is_grown_and_a_zero_one_is_a_line() {
        let grown = path("M0 0A1 1 0 0 1 10 0").unwrap();
        assert_eq!(end_of(&grown), (10.0, 0.0));
        assert_eq!(
            path("M0 0A0 5 0 0 1 3 4").unwrap().last(),
            Some(&Seg::Line(3.0, 4.0))
        );
    }

    #[test]
    fn quadratic_curves_become_cubic_ones() {
        let s = path("M0 0Q3 6 6 0").unwrap();
        assert_eq!(s[1], Seg::Cubic(2.0, 4.0, 4.0, 4.0, 6.0, 0.0));
        let t = path("M0 0Q3 6 6 0T12 0").unwrap();
        assert_eq!(end_of(&t), (12.0, 0.0));
    }

    #[test]
    fn what_is_not_a_path_is_an_error_not_a_guess() {
        assert!(path("L1 1 5").is_err() || path("5 5").is_err());
        assert!(path("M1").is_err());
        assert!(path("M0 0Z 3").is_err());
        assert!(path("M0 0X1 1").is_err());
        assert!(path("M0 0a1 1 0 21 1 1").is_err(), "a flag is 0 or 1");
    }

    #[test]
    fn elements_become_outlines() {
        let rect = shapes(r#"<rect width="8" height="4" x="2" y="3" rx="1" />"#).unwrap();
        assert!(matches!(rect[0], Seg::Move(x, y) if near(x, 3.0) && near(y, 3.0)));
        assert_eq!(rect.last(), Some(&Seg::Close));
        let line = shapes(r#"<line x1="1" x2="5" y1="2" y2="2" />"#).unwrap();
        assert_eq!(line, [Seg::Move(1.0, 2.0), Seg::Line(5.0, 2.0)]);
        let poly = shapes(r#"<polyline points="1 1 4 4 7 1" />"#).unwrap();
        assert_eq!(
            poly,
            [
                Seg::Move(1.0, 1.0),
                Seg::Line(4.0, 4.0),
                Seg::Line(7.0, 1.0)
            ]
        );
        assert!(shapes(r#"<star points="1" />"#).is_err());
    }

    #[test]
    fn every_icon_of_the_library_reads_and_stays_on_its_grid() {
        let mut count = 0;
        for name in names() {
            let segs = icon(name).unwrap_or_else(|| panic!("{name} does not parse"));
            assert!(!segs.is_empty(), "{name}");
            for seg in segs {
                let points: Vec<(f32, f32)> = match *seg {
                    Seg::Move(x, y) | Seg::Line(x, y) => vec![(x, y)],
                    Seg::Cubic(a, b, c, d, x, y) => vec![(a, b), (c, d), (x, y)],
                    Seg::Close => vec![],
                };
                for (x, y) in points {
                    assert!(
                        (-1.0..=25.0).contains(&x) && (-1.0..=25.0).contains(&y),
                        "{name}: ({x}, {y})"
                    );
                }
            }
            count += 1;
        }
        assert!(count >= 50, "{count}");
        assert!(icon("not-an-icon").is_none());
    }
}
