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
pub fn path(d: &str) -> Result<Vec<Seg>, String> {
    let mut scanner = Scanner {
        bytes: d.as_bytes(),
        at: 0,
    };
    let mut pen = Pen::default();
    let mut command: Option<u8> = None;
    while !scanner.at_end() {
        let letter = next_command(&mut scanner, &mut command)?;
        pen.apply(&mut scanner, letter)?;
    }
    Ok(pen.out)
}

/// The command that governs what comes next: a new letter, or the previous command
/// repeated when numbers follow (more pairs after a move are lines).
fn next_command(scanner: &mut Scanner, command: &mut Option<u8>) -> Result<u8, String> {
    if let Some(letter) = scanner.letter() {
        *command = Some(letter);
        return Ok(letter);
    }
    let previous = command.ok_or("el trazado no empieza con una orden")?;
    if matches!(previous, b'Z' | b'z') || !scanner.has_number() {
        return Err(format!("sobra algo tras la orden «{}»", previous as char));
    }
    let repeated = match previous {
        b'M' => b'L',
        b'm' => b'l',
        other => other,
    };
    *command = Some(repeated);
    Ok(repeated)
}

/// The pen of the path: where it is, where the current shape started, and the last
/// control point of a curve (to reflect in `S` and `T`).
#[derive(Default)]
struct Pen {
    x: f32,
    y: f32,
    start: (f32, f32),
    cubic_ctrl: Option<(f32, f32)>,
    quad_ctrl: Option<(f32, f32)>,
    out: Vec<Seg>,
}

impl Pen {
    fn at(&self) -> (f32, f32) {
        (self.x, self.y)
    }

    /// Runs one command, reading its numbers; lowercase letters are relative to the pen.
    fn apply(&mut self, sc: &mut Scanner, letter: u8) -> Result<(), String> {
        let origin = if letter.is_ascii_lowercase() {
            self.at()
        } else {
            (0.0, 0.0)
        };
        let (mut next_cubic, mut next_quad) = (None, None);
        match letter.to_ascii_uppercase() {
            b'M' => self.move_to(sc, origin)?,
            b'L' => self.line_to(sc, origin)?,
            b'H' => self.line_to_x(sc, origin.0)?,
            b'V' => self.line_to_y(sc, origin.1)?,
            b'C' => next_cubic = Some(self.cubic(sc, origin)?),
            b'S' => next_cubic = Some(self.smooth_cubic(sc, origin)?),
            b'Q' => next_quad = Some(self.quadratic(sc, origin)?),
            b'T' => next_quad = Some(self.smooth_quadratic(sc, origin)?),
            b'A' => self.arc_to(sc, origin)?,
            b'Z' => self.close(),
            other => return Err(format!("orden no soportada: «{}»", other as char)),
        }
        self.cubic_ctrl = next_cubic;
        self.quad_ctrl = next_quad;
        Ok(())
    }

    fn move_to(&mut self, sc: &mut Scanner, origin: (f32, f32)) -> Result<(), String> {
        let (px, py) = sc.pair()?;
        (self.x, self.y) = (origin.0 + px, origin.1 + py);
        self.start = self.at();
        self.out.push(Seg::Move(self.x, self.y));
        Ok(())
    }

    fn line_to(&mut self, sc: &mut Scanner, origin: (f32, f32)) -> Result<(), String> {
        let (px, py) = sc.pair()?;
        (self.x, self.y) = (origin.0 + px, origin.1 + py);
        self.out.push(Seg::Line(self.x, self.y));
        Ok(())
    }

    fn line_to_x(&mut self, sc: &mut Scanner, origin_x: f32) -> Result<(), String> {
        self.x = origin_x + sc.number()?;
        self.out.push(Seg::Line(self.x, self.y));
        Ok(())
    }

    fn line_to_y(&mut self, sc: &mut Scanner, origin_y: f32) -> Result<(), String> {
        self.y = origin_y + sc.number()?;
        self.out.push(Seg::Line(self.x, self.y));
        Ok(())
    }

    /// `C`: two control points and the end. Returns the second control point.
    fn cubic(&mut self, sc: &mut Scanner, origin: (f32, f32)) -> Result<(f32, f32), String> {
        let (x1, y1) = sc.pair()?;
        let (x2, y2) = sc.pair()?;
        let (px, py) = sc.pair()?;
        let first = (origin.0 + x1, origin.1 + y1);
        let second = (origin.0 + x2, origin.1 + y2);
        (self.x, self.y) = (origin.0 + px, origin.1 + py);
        self.out.push(Seg::Cubic(
            first.0, first.1, second.0, second.1, self.x, self.y,
        ));
        Ok(second)
    }

    /// `S`: like `C` but the first control point is the reflection of the last one.
    fn smooth_cubic(&mut self, sc: &mut Scanner, origin: (f32, f32)) -> Result<(f32, f32), String> {
        let (x2, y2) = sc.pair()?;
        let (px, py) = sc.pair()?;
        let first = self.reflected(self.cubic_ctrl);
        let second = (origin.0 + x2, origin.1 + y2);
        (self.x, self.y) = (origin.0 + px, origin.1 + py);
        self.out.push(Seg::Cubic(
            first.0, first.1, second.0, second.1, self.x, self.y,
        ));
        Ok(second)
    }

    /// `Q`: one control point and the end, drawn as a cubic. Returns the control point.
    fn quadratic(&mut self, sc: &mut Scanner, origin: (f32, f32)) -> Result<(f32, f32), String> {
        let (qx, qy) = sc.pair()?;
        let (px, py) = sc.pair()?;
        let control = (origin.0 + qx, origin.1 + qy);
        self.quad_to(control, (origin.0 + px, origin.1 + py));
        Ok(control)
    }

    /// `T`: like `Q` but the control point is the reflection of the last one.
    fn smooth_quadratic(
        &mut self,
        sc: &mut Scanner,
        origin: (f32, f32),
    ) -> Result<(f32, f32), String> {
        let (px, py) = sc.pair()?;
        let control = self.reflected(self.quad_ctrl);
        self.quad_to(control, (origin.0 + px, origin.1 + py));
        Ok(control)
    }

    fn quad_to(&mut self, control: (f32, f32), end: (f32, f32)) {
        self.out
            .push(quad(self.x, self.y, control.0, control.1, end.0, end.1));
        (self.x, self.y) = end;
    }

    fn arc_to(&mut self, sc: &mut Scanner, origin: (f32, f32)) -> Result<(), String> {
        let radii = sc.pair()?;
        let rotation = sc.number()?;
        let large = sc.flag()?;
        let sweep = sc.flag()?;
        let (px, py) = sc.pair()?;
        let end = (origin.0 + px, origin.1 + py);
        let from = self.at();
        arc(&mut self.out, from, radii, rotation, (large, sweep), end);
        (self.x, self.y) = end;
        Ok(())
    }

    fn close(&mut self) {
        self.out.push(Seg::Close);
        (self.x, self.y) = self.start;
    }

    /// The reflection of the last control point through the pen (or the pen itself).
    fn reflected(&self, control: Option<(f32, f32)>) -> (f32, f32) {
        control.map_or(self.at(), |(cx, cy)| (2.0 * self.x - cx, 2.0 * self.y - cy))
    }
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
/// (the conversion of the SVG specification, appendix F.6). `flags` are
/// `(large arc, sweep)`.
fn arc(
    out: &mut Vec<Seg>,
    from: (f32, f32),
    radii: (f32, f32),
    rotation: f32,
    flags: (bool, bool),
    to: (f32, f32),
) {
    if from == to {
        return;
    }
    let (rx, ry) = (f64::from(radii.0).abs(), f64::from(radii.1).abs());
    if rx == 0.0 || ry == 0.0 {
        out.push(Seg::Line(to.0, to.1));
        return;
    }
    let ellipse = Ellipse::through(from, to, (rx, ry), f64::from(rotation), flags);
    ellipse.push_cubics(out, to);
}

/// The ellipse an arc lies on, in the center form of the SVG specification.
struct Ellipse {
    center: (f64, f64),
    radii: (f64, f64),
    /// Sine and cosine of the rotation of the ellipse.
    sin_cos: (f64, f64),
    /// Where the arc starts and how far it goes (radians, signed).
    theta: f64,
    delta: f64,
}

impl Ellipse {
    /// The center form from the end points (F.6.5 of the specification). Radii too small
    /// to reach are scaled up until they do.
    fn through(
        from: (f32, f32),
        to: (f32, f32),
        radii: (f64, f64),
        rotation: f64,
        (large, sweep): (bool, bool),
    ) -> Ellipse {
        let (x1, y1) = (f64::from(from.0), f64::from(from.1));
        let (x2, y2) = (f64::from(to.0), f64::from(to.1));
        let (mut rx, mut ry) = radii;
        let (sin, cos) = rotation.to_radians().sin_cos();
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
        let center = (
            cos * cxp - sin * cyp + (x1 + x2) / 2.0,
            sin * cxp + cos * cyp + (y1 + y2) / 2.0,
        );
        let start = ((x1p - cxp) / rx, (y1p - cyp) / ry);
        let end = ((-x1p - cxp) / rx, (-y1p - cyp) / ry);
        let theta = angle((1.0, 0.0), start);
        let mut delta = angle(start, end);
        if !sweep && delta > 0.0 {
            delta -= std::f64::consts::TAU;
        } else if sweep && delta < 0.0 {
            delta += std::f64::consts::TAU;
        }
        Ellipse {
            center,
            radii: (rx, ry),
            sin_cos: (sin, cos),
            theta,
            delta,
        }
    }

    /// A point of the unit circle, taken onto the ellipse.
    fn map(&self, u: f64, v: f64) -> (f64, f64) {
        let ((cx, cy), (rx, ry), (sin, cos)) = (self.center, self.radii, self.sin_cos);
        (
            cx + rx * u * cos - ry * v * sin,
            cy + rx * u * sin + ry * v * cos,
        )
    }

    /// The arc as one cubic per quarter turn at most; the last one ends exactly at `to`.
    fn push_cubics(&self, out: &mut Vec<Seg>, to: (f32, f32)) {
        let parts = ((self.delta.abs() / std::f64::consts::FRAC_PI_2) - 1e-9)
            .ceil()
            .max(1.0) as usize;
        let step = self.delta / parts as f64;
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        for i in 0..parts {
            let (a1, a2) = (
                self.theta + step * i as f64,
                self.theta + step * (i as f64 + 1.0),
            );
            let (c1x, c1y) = self.map(a1.cos() - k * a1.sin(), a1.sin() + k * a1.cos());
            let (c2x, c2y) = self.map(a2.cos() + k * a2.sin(), a2.sin() - k * a2.cos());
            let (ex, ey) = if i + 1 == parts {
                (f64::from(to.0), f64::from(to.1))
            } else {
                self.map(a2.cos(), a2.sin())
            };
            out.push(Seg::Cubic(
                c1x as f32, c1y as f32, c2x as f32, c2y as f32, ex as f32, ey as f32,
            ));
        }
    }
}

/// The signed angle from one vector to another.
fn angle(u: (f64, f64), v: (f64, f64)) -> f64 {
    (u.0 * v.1 - u.1 * v.0).atan2(u.0 * v.0 + u.1 * v.1)
}
