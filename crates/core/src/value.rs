//! Decimal fixed point for the values a reader changes and the plots they
//! drive. Builder's core has no floats, so a value is an integer count of
//! billionths. Sums of values typed with up to nine decimals are exact; the
//! functions (`sqrt`, `exp`, `ln`, `sin`, `cos`) are accurate to about one
//! billionth near the values a teaching example uses.

pub const S: i64 = 1_000_000_000;
const PI: i64 = 3_141_592_654;
const LN2: i64 = 693_147_181;
const E: i64 = 2_718_281_828;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Fx(pub i64);

pub type R = Result<Fx, String>;

fn fit(v: i128) -> R {
    if v > i64::MAX as i128 || v < i64::MIN as i128 {
        Err("A value grew too large to show.".into())
    } else {
        Ok(Fx(v as i64))
    }
}

// add, sub, mul and div return a Result (overflow, division by zero), so
// they are methods rather than the operator traits.
#[allow(clippy::should_implement_trait)]
impl Fx {
    pub const ZERO: Fx = Fx(0);
    pub const ONE: Fx = Fx(S);

    pub fn int(n: i64) -> Fx {
        Fx(n * S)
    }

    pub fn pi() -> Fx {
        Fx(PI)
    }

    pub fn e() -> Fx {
        Fx(E)
    }

    /// Reads a decimal such as `-1.25`.
    pub fn parse(text: &str) -> Option<Fx> {
        let t = text.trim();
        let (neg, t) = match t.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, t.strip_prefix('+').unwrap_or(t)),
        };
        if t.is_empty() {
            return None;
        }
        let (whole, frac) = t.split_once('.').unwrap_or((t, ""));
        if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        if whole.is_empty() && frac.is_empty() {
            return None;
        }
        let w: i64 = if whole.is_empty() { 0 } else { whole.parse().ok()? };
        let mut f: i64 = 0;
        let mut scale = S / 10;
        for c in frac.chars().take(9) {
            f += (c as i64 - '0' as i64) * scale;
            scale /= 10;
        }
        let v = w.checked_mul(S)?.checked_add(f)?;
        Some(Fx(if neg { -v } else { v }))
    }

    /// Writes the value with `digits` decimals, rounding half away from zero,
    /// and a minus sign (U+2212 is the layout's job, not this one's).
    pub fn format(self, digits: u32) -> String {
        let unit = 10i128.pow(9 - digits.min(9));
        let v = self.0 as i128;
        let r = if v >= 0 {
            (v + unit / 2) / unit
        } else {
            (v - unit / 2) / unit
        };
        let neg = r < 0;
        let r = r.abs();
        let p = 10i128.pow(digits.min(9));
        let s = if digits == 0 {
            format!("{r}")
        } else {
            format!("{}.{:0w$}", r / p, r % p, w = digits as usize)
        };
        if neg {
            format!("-{s}")
        } else {
            s
        }
    }

    /// The fewest decimals (up to `max`) that show the value exactly.
    pub fn digits_needed(self, max: u32) -> u32 {
        (0..max).find(|d| self.0 % 10i64.pow(9 - d) == 0).unwrap_or(max)
    }

    pub fn add(self, o: Fx) -> R {
        fit(self.0 as i128 + o.0 as i128)
    }
    pub fn sub(self, o: Fx) -> R {
        fit(self.0 as i128 - o.0 as i128)
    }
    pub fn neg(self) -> Fx {
        Fx(-self.0)
    }
    pub fn mul(self, o: Fx) -> R {
        let p = self.0 as i128 * o.0 as i128;
        let s = S as i128;
        fit(if p >= 0 { (p + s / 2) / s } else { (p - s / 2) / s })
    }
    pub fn div(self, o: Fx) -> R {
        if o.0 == 0 {
            return Err("A value divides by zero.".into());
        }
        let n = self.0 as i128 * S as i128;
        let d = o.0 as i128;
        let q = n / d;
        let r = n % d;
        let up = (r.abs() * 2 >= d.abs()) as i128;
        fit(if (n < 0) != (d < 0) { q - up } else { q + up })
    }

    pub fn sqrt(self) -> R {
        if self.0 < 0 {
            return Err("A square root of a negative value has no real value.".into());
        }
        Ok(Fx(isqrt(self.0 as i128 * S as i128) as i64))
    }

    pub fn exp(self) -> R {
        if self.0 > 40 * S {
            return Err("A value grew too large to show.".into());
        }
        if self.0 < -40 * S {
            return Ok(Fx::ZERO);
        }
        // x = k ln 2 + r, |r| <= ln2 / 2
        let k = (self.0 + LN2 / 2 * self.0.signum()) / LN2;
        let r = self.0 - k * LN2;
        // the series runs at a billion times the precision, then rounds back
        let big = S as i128 * S as i128;
        let r = r as i128 * S as i128;
        let mut term = big;
        let mut sum = term;
        for n in 1..40 {
            term = term * r / (big * n);
            if term == 0 {
                break;
            }
            sum += term;
        }
        let v = if k >= 0 { sum << k } else { sum >> (-k) };
        fit((v + S as i128 / 2) / S as i128)
    }

    pub fn ln(self) -> R {
        if self.0 <= 0 {
            return Err("A logarithm of zero or a negative value has no real value.".into());
        }
        let mut m = self.0 as i128;
        let s = S as i128;
        let mut k: i64 = 0;
        while m >= 2 * s {
            m /= 2;
            k += 1;
        }
        while m < s {
            m *= 2;
            k -= 1;
        }
        // ln m = 2 atanh z, z = (m - 1) / (m + 1)
        let z = (m - s) * s / (m + s);
        let z2 = z * z / s;
        let mut p = z;
        let mut sum = 0i128;
        for n in 0..60 {
            let t = p / (2 * n + 1);
            if t == 0 {
                break;
            }
            sum += t;
            p = p * z2 / s;
        }
        fit(2 * sum + k as i128 * LN2 as i128)
    }

    fn reduce_angle(self) -> i128 {
        let two_pi = 2 * PI as i128;
        let mut x = self.0 as i128 % two_pi;
        if x > PI as i128 {
            x -= two_pi;
        }
        if x < -(PI as i128) {
            x += two_pi;
        }
        x
    }

    pub fn sin(self) -> R {
        let x = self.reduce_angle();
        let s = S as i128;
        let x2 = x * x / s;
        let mut term = x;
        let mut sum = x;
        for n in 1..30 {
            term = -term * x2 / s / ((2 * n) * (2 * n + 1));
            if term == 0 {
                break;
            }
            sum += term;
        }
        fit(sum)
    }

    pub fn cos(self) -> R {
        Fx(self.0 + PI / 2).sin()
    }

    pub fn tan(self) -> R {
        self.sin()?.div(self.cos()?)
    }

    pub fn pow(self, p: Fx) -> R {
        if p.0 % S == 0 && (p.0 / S).abs() <= 64 {
            let n = p.0 / S;
            let mut acc = Fx::ONE;
            for _ in 0..n.abs() {
                acc = acc.mul(self)?;
            }
            return if n < 0 { Fx::ONE.div(acc) } else { Ok(acc) };
        }
        if self.0 <= 0 {
            return Err("A power of a negative value needs a whole-number exponent.".into());
        }
        self.ln()?.mul(p)?.exp()
    }
}

fn isqrt(n: i128) -> i128 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(t: &str) -> Fx {
        Fx::parse(t).unwrap()
    }

    #[test]
    fn decimals_read_and_write_exactly() {
        assert_eq!(fx("1.25"), Fx(1_250_000_000));
        assert_eq!(fx("-0.5"), Fx(-500_000_000));
        assert_eq!(fx(".5"), Fx(500_000_000));
        assert_eq!(fx("3"), Fx::int(3));
        assert!(Fx::parse("1.2.3").is_none() && Fx::parse("").is_none() && Fx::parse("x").is_none());
        assert_eq!(fx("1.25").format(2), "1.25");
        assert_eq!(fx("1.25").format(1), "1.3", "half rounds away from zero");
        assert_eq!(fx("-1.25").format(1), "-1.3");
        assert_eq!(fx("2").format(0), "2");
        assert_eq!(fx("0.1").digits_needed(4), 1);
        assert_eq!(fx("7").digits_needed(4), 0);
    }

    #[test]
    fn arithmetic_is_exact_for_decimals() {
        // 0.1 + 0.2 is exactly 0.3 here
        assert_eq!(fx("0.1").add(fx("0.2")).unwrap(), fx("0.3"));
        assert_eq!(fx("1.5").mul(fx("2")).unwrap().add(fx("1")).unwrap(), fx("4"));
        assert_eq!(fx("1").div(fx("4")).unwrap(), fx("0.25"));
        assert_eq!(fx("1").div(fx("3")).unwrap(), Fx(333_333_333));
        assert_eq!(fx("2").pow(fx("10")).unwrap(), fx("1024"));
        assert_eq!(fx("2").pow(fx("-2")).unwrap(), fx("0.25"));
        assert!(fx("1").div(Fx::ZERO).is_err());
    }

    fn near(a: Fx, b: i64, tol: i64) -> bool {
        (a.0 - b).abs() <= tol
    }

    #[test]
    fn functions_match_hand_values_to_a_few_billionths() {
        // values from tables: sqrt 2 = 1.414213562, e = 2.718281828,
        // ln 10 = 2.302585093, sin 1 = 0.841470985, cos 1 = 0.540302306
        assert!(near(fx("2").sqrt().unwrap(), 1_414_213_562, 1));
        assert!(near(Fx::ONE.exp().unwrap(), 2_718_281_828, 4));
        assert!(near(fx("10").ln().unwrap(), 2_302_585_093, 4));
        assert!(near(Fx::ONE.sin().unwrap(), 841_470_985, 3));
        assert!(near(Fx::ONE.cos().unwrap(), 540_302_306, 3));
        assert!(
            near(fx("7").sin().unwrap(), 656_986_599, 5),
            "sin 7 after reducing by 2 pi"
        );
        assert!(near(fx("2").pow(fx("0.5")).unwrap(), 1_414_213_562, 6));
        assert!(fx("-1").sqrt().is_err() && Fx::ZERO.ln().is_err());
    }
}
