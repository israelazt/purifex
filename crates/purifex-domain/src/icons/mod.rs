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
pub fn shapes(svg_inner: &str) -> Result<Vec<Seg>, String> {
    let mut segs = Vec::new();
    for raw in svg_inner.split('<').skip(1) {
        if let Some(d) = Element::parse(raw).path_data()? {
            segs.extend(path(&d)?);
        }
    }
    Ok(segs)
}

/// One element of the SVG: its tag and its attributes, as text.
struct Element<'a> {
    tag: &'a str,
    attrs: &'a str,
}

impl<'a> Element<'a> {
    fn parse(raw: &'a str) -> Self {
        let raw = raw.trim_end_matches(['>', '/', ' ']);
        let (tag, attrs) = raw.split_once(' ').unwrap_or((raw, ""));
        Element { tag, attrs }
    }

    fn attr(&self, name: &str) -> Option<&'a str> {
        attribute(self.attrs, name)
    }

    /// A number that must be there.
    fn number(&self, name: &str) -> Result<f32, String> {
        self.attr(name)
            .ok_or_else(|| format!("<{}> sin «{name}»", self.tag))?
            .trim()
            .parse()
            .map_err(|_| format!("<{}>: «{name}» no es un número", self.tag))
    }

    /// A number that may be missing (or unreadable, which counts as missing).
    fn optional_number(&self, name: &str) -> Option<f32> {
        self.attr(name).and_then(|v| v.trim().parse().ok())
    }

    /// The element as the data of an SVG path; `None` for the ones that draw nothing.
    fn path_data(&self) -> Result<Option<String>, String> {
        let data = match self.tag {
            "path" => self.attr("d").ok_or("<path> sin «d»")?.to_string(),
            "circle" => self.circle()?,
            "line" => self.line()?,
            "polyline" => format!("M{}", self.attr("points").ok_or("<polyline> sin «points»")?),
            "rect" => self.rect()?,
            "" | "svg" => return Ok(None),
            other => return Err(format!("elemento no soportado: <{other}>")),
        };
        Ok(Some(data))
    }

    fn circle(&self) -> Result<String, String> {
        let (cx, cy, r) = (self.number("cx")?, self.number("cy")?, self.number("r")?);
        Ok(format!(
            "M{} {}A{r} {r} 0 1 1 {} {}A{r} {r} 0 1 1 {} {}Z",
            cx - r,
            cy,
            cx + r,
            cy,
            cx - r,
            cy
        ))
    }

    fn line(&self) -> Result<String, String> {
        Ok(format!(
            "M{} {}L{} {}",
            self.number("x1")?,
            self.number("y1")?,
            self.number("x2")?,
            self.number("y2")?
        ))
    }

    /// A rectangle with rounded corners (`rx`, `ry`, each at most half the side).
    fn rect(&self) -> Result<String, String> {
        let x = self.optional_number("x").unwrap_or(0.0);
        let y = self.optional_number("y").unwrap_or(0.0);
        let (w, h) = (self.number("width")?, self.number("height")?);
        let (rx_attr, ry_attr) = (self.optional_number("rx"), self.optional_number("ry"));
        let rx = rx_attr.or(ry_attr).unwrap_or(0.0).min(w / 2.0);
        let ry = ry_attr.or(rx_attr).unwrap_or(0.0).min(h / 2.0);
        Ok(format!(
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
        ))
    }
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
