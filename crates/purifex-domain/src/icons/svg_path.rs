//! Reads the `d` of an SVG `<path>` into moves, lines and cubic curves: M L H V C
//! S Q T A Z, absolute and relative. Arcs and quadratic curves are converted to
//! cubic ones, so anything that draws can stroke the result with two primitives.

/// One step of an outline, on the 24x24 grid of the icons.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    Move(f32, f32),
    Line(f32, f32),
    /// Two control points and the end.
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
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

/// Reads the `d` of a `<path>`.
#[expect(clippy::too_many_lines, reason = "legacy: split")]
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
#[expect(clippy::too_many_lines, reason = "legacy: split")]
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
