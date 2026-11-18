//! Lucide icons as geometry: the SVG of each icon is read into moves, lines and
//! cubic curves (arcs and circles are converted), which any graphics library can
//! stroke. Plain code, tested without a window.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::icon_data::ICONS;

/// One step of an outline, on the 24x24 grid of the icons.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    Move(f32, f32),
    Line(f32, f32),
    /// Two control points and the end.
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

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

struct Scanner<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Scanner<'_> {
    fn skip_separators(&mut self) {
        while self.at < self.bytes.len()
            && (self.bytes[self.at].is_ascii_whitespace() || self.bytes[self.at] == b',')
        {
            self.at += 1;
        }
    }

    fn at_end(&mut self) -> bool {
        self.skip_separators();
        self.at >= self.bytes.len()
    }

    fn letter(&mut self) -> Option<u8> {
        self.skip_separators();
        let c = *self.bytes.get(self.at)?;
        c.is_ascii_alphabetic().then(|| {
            self.at += 1;
            c
        })
    }

    fn has_number(&mut self) -> bool {
        self.skip_separators();
        self.bytes
            .get(self.at)
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'.' | b'-' | b'+'))
    }

    /// A number: `12`, `-.5`, `1.5`, `2e-3`. Two numbers may touch (`1.5.5` is
    /// 1.5 and .5; `1-2` is 1 and -2).
    fn number(&mut self) -> Result<f32, String> {
        self.skip_separators();
        let start = self.at;
        let b = self.bytes;
        let mut i = self.at;
        if matches!(b.get(i), Some(b'-' | b'+')) {
            i += 1;
        }
        let mut digits = false;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
            digits = true;
        }
        if b.get(i) == Some(&b'.') {
            i += 1;
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
                digits = true;
            }
        }
        if !digits {
            return Err(format!("se esperaba un número en la posición {start}"));
        }
        if matches!(b.get(i), Some(b'e' | b'E')) {
            let mut j = i + 1;
            if matches!(b.get(j), Some(b'-' | b'+')) {
                j += 1;
            }
            if b.get(j).is_some_and(u8::is_ascii_digit) {
                while b.get(j).is_some_and(u8::is_ascii_digit) {
                    j += 1;
                }
                i = j;
            }
        }
        self.at = i;
        std::str::from_utf8(&b[start..i])
            .ok()
            .and_then(|t| t.parse().ok())
            .ok_or_else(|| format!("número no válido en la posición {start}"))
    }

    /// An arc flag: one character, `0` or `1`, that may touch the next number.
    fn flag(&mut self) -> Result<bool, String> {
        self.skip_separators();
        match self.bytes.get(self.at) {
            Some(b'0') => {
                self.at += 1;
                Ok(false)
            }
            Some(b'1') => {
                self.at += 1;
                Ok(true)
            }
            _ => Err(format!("se esperaba 0 o 1 en la posición {}", self.at)),
        }
    }

    fn pair(&mut self) -> Result<(f32, f32), String> {
        Ok((self.number()?, self.number()?))
    }
}

/// Reads the `d` of a `<path>`: M L H V C S Q T A Z, absolute and relative.
pub fn path(d: &str) -> Result<Vec<Seg>, String> {
    let mut sc = Scanner {
        bytes: d.as_bytes(),
        at: 0,
    };
    let mut out = Vec::new();
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let (mut start_x, mut start_y) = (0.0f32, 0.0f32);
    // The last control point of a curve, to reflect in S and T.
    let (mut cubic_ctrl, mut quad_ctrl) = (None::<(f32, f32)>, None::<(f32, f32)>);
    let mut command: Option<u8> = None;

    while !sc.at_end() {
        let c = match sc.letter() {
            Some(letter) => {
                command = Some(letter);
                letter
            }
            None => {
                let previous = command.ok_or("el trazado no empieza con una orden")?;
                if matches!(previous, b'Z' | b'z') || !sc.has_number() {
                    return Err(format!("sobra algo tras la orden «{}»", previous as char));
                }
                // More pairs after a move are lines.
                let repeated = match previous {
                    b'M' => b'L',
                    b'm' => b'l',
                    other => other,
                };
                command = Some(repeated);
                repeated
            }
        };
        let relative = c.is_ascii_lowercase();
        let (ox, oy) = if relative { (x, y) } else { (0.0, 0.0) };
        let mut next_cubic = None;
        let mut next_quad = None;
        match c.to_ascii_uppercase() {
            b'M' => {
                let (px, py) = sc.pair()?;
                (x, y) = (ox + px, oy + py);
                (start_x, start_y) = (x, y);
                out.push(Seg::Move(x, y));
            }
            b'L' => {
                let (px, py) = sc.pair()?;
                (x, y) = (ox + px, oy + py);
                out.push(Seg::Line(x, y));
            }
            b'H' => {
                x = ox + sc.number()?;
                out.push(Seg::Line(x, y));
            }
            b'V' => {
                y = oy + sc.number()?;
                out.push(Seg::Line(x, y));
            }
            b'C' => {
                let (x1, y1) = sc.pair()?;
                let (x2, y2) = sc.pair()?;
                let (px, py) = sc.pair()?;
                let (c1x, c1y, c2x, c2y) = (ox + x1, oy + y1, ox + x2, oy + y2);
                (x, y) = (ox + px, oy + py);
                out.push(Seg::Cubic(c1x, c1y, c2x, c2y, x, y));
                next_cubic = Some((c2x, c2y));
            }
            b'S' => {
                let (x2, y2) = sc.pair()?;
                let (px, py) = sc.pair()?;
                let (c1x, c1y) = cubic_ctrl.map_or((x, y), |(cx, cy)| (2.0 * x - cx, 2.0 * y - cy));
                let (c2x, c2y) = (ox + x2, oy + y2);
                (x, y) = (ox + px, oy + py);
                out.push(Seg::Cubic(c1x, c1y, c2x, c2y, x, y));
                next_cubic = Some((c2x, c2y));
            }
            b'Q' => {
                let (qx, qy) = sc.pair()?;
                let (px, py) = sc.pair()?;
                let (qx, qy) = (ox + qx, oy + qy);
                let (ex, ey) = (ox + px, oy + py);
                out.push(quad(x, y, qx, qy, ex, ey));
                (x, y) = (ex, ey);
                next_quad = Some((qx, qy));
            }
            b'T' => {
                let (px, py) = sc.pair()?;
                let (qx, qy) = quad_ctrl.map_or((x, y), |(cx, cy)| (2.0 * x - cx, 2.0 * y - cy));
                let (ex, ey) = (ox + px, oy + py);
                out.push(quad(x, y, qx, qy, ex, ey));
                (x, y) = (ex, ey);
                next_quad = Some((qx, qy));
            }
            b'A' => {
                let (rx, ry) = sc.pair()?;
                let rotation = sc.number()?;
                let large = sc.flag()?;
                let sweep = sc.flag()?;
                let (px, py) = sc.pair()?;
                let (ex, ey) = (ox + px, oy + py);
                arc(&mut out, (x, y), (rx, ry), rotation, large, sweep, (ex, ey));
                (x, y) = (ex, ey);
            }
            b'Z' => {
                out.push(Seg::Close);
                (x, y) = (start_x, start_y);
            }
            other => return Err(format!("orden no soportada: «{}»", other as char)),
        }
        cubic_ctrl = next_cubic;
        quad_ctrl = next_quad;
    }
    Ok(out)
}

/// A quadratic curve as the cubic that draws the same line.
fn quad(x0: f32, y0: f32, qx: f32, qy: f32, x: f32, y: f32) -> Seg {
    Seg::Cubic(
        x0 + 2.0 / 3.0 * (qx - x0),
        y0 + 2.0 / 3.0 * (qy - y0),
        x + 2.0 / 3.0 * (qx - x),
        y + 2.0 / 3.0 * (qy - y),
        x,
        y,
    )
}

/// An SVG elliptical arc as cubic curves, one per quarter turn at most
/// (the conversion of the SVG specification, appendix F.6).
fn arc(
    out: &mut Vec<Seg>,
    from: (f32, f32),
    radii: (f32, f32),
    rotation: f32,
    large: bool,
    sweep: bool,
    to: (f32, f32),
) {
    let (x1, y1) = (f64::from(from.0), f64::from(from.1));
    let (x2, y2) = (f64::from(to.0), f64::from(to.1));
    let (mut rx, mut ry) = (f64::from(radii.0).abs(), f64::from(radii.1).abs());
    if x1 == x2 && y1 == y2 {
        return;
    }
    if rx == 0.0 || ry == 0.0 {
        out.push(Seg::Line(to.0, to.1));
        return;
    }
    let phi = f64::from(rotation).to_radians();
    let (sin, cos) = phi.sin_cos();
    let (dx, dy) = ((x1 - x2) / 2.0, (y1 - y2) / 2.0);
    let (x1p, y1p) = (cos * dx + sin * dy, -sin * dx + cos * dy);
    let lambda = x1p * x1p / (rx * rx) + y1p * y1p / (ry * ry);
    if lambda > 1.0 {
        let scale = lambda.sqrt();
        rx *= scale;
        ry *= scale;
    }
    let sign = if large == sweep { -1.0 } else { 1.0 };
    let numerator = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    let denominator = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let coefficient = sign * (numerator / denominator).max(0.0).sqrt();
    let (cxp, cyp) = (coefficient * rx * y1p / ry, -coefficient * ry * x1p / rx);
    let (cx, cy) = (
        cos * cxp - sin * cyp + (x1 + x2) / 2.0,
        sin * cxp + cos * cyp + (y1 + y2) / 2.0,
    );
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let theta = angle(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut delta = angle(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );
    if !sweep && delta > 0.0 {
        delta -= std::f64::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += std::f64::consts::TAU;
    }
    let parts = ((delta.abs() / std::f64::consts::FRAC_PI_2) - 1e-9)
        .ceil()
        .max(1.0) as usize;
    let step = delta / parts as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let map = |u: f64, v: f64| {
        (
            cx + rx * u * cos - ry * v * sin,
            cy + rx * u * sin + ry * v * cos,
        )
    };
    for i in 0..parts {
        let (a1, a2) = (theta + step * i as f64, theta + step * (i as f64 + 1.0));
        let (c1x, c1y) = map(a1.cos() - k * a1.sin(), a1.sin() + k * a1.cos());
        let (c2x, c2y) = map(a2.cos() + k * a2.sin(), a2.sin() - k * a2.cos());
        let (ex, ey) = if i + 1 == parts {
            (x2, y2)
        } else {
            map(a2.cos(), a2.sin())
        };
        out.push(Seg::Cubic(
            c1x as f32, c1y as f32, c2x as f32, c2y as f32, ex as f32, ey as f32,
        ));
    }
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
