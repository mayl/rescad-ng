//! Parsing and formatting of resistor values in RKM / SI-prefix notation.
//!
//! Accepted input mirrors the original ResCad: the multiplier letter may take
//! the place of the decimal point (`4k7`, `2R2`, `1M5`), may follow a decimal
//! number (`4.7k`, `1.5M`) or may be omitted (`470`).  `R`, `r` and `.` all
//! mean "ohms".  Prefixes: `p n u µ m R k K M G T` (note `m` = milli and
//! `M` = mega).  Spaces and an optional trailing `Ω`/`ohm` are ignored.

/// Parse a value such as `4k7`, `1.5M`, `2R2`, `470` or `33 kΩ`.
pub fn parse(text: &str) -> Option<f64> {
    let mut s = text.trim();
    for suffix in ["ohms", "ohm", "Ohms", "Ohm", "Ω", "ω"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            s = stripped.trim_end();
            break;
        }
    }

    let mut mantissa: f64 = 0.0;
    let mut digits = 0usize;
    let mut decimals = 0i32;
    let mut point_seen = false;
    let mut multiplier: Option<f64> = None;

    for c in s.chars() {
        match c {
            '0'..='9' => {
                mantissa = mantissa * 10.0 + c.to_digit(10).unwrap() as f64;
                digits += 1;
                if point_seen {
                    decimals += 1;
                }
            }
            ' ' | '_' | '\'' => {}
            '.' | ',' => {
                if point_seen {
                    return None;
                }
                point_seen = true;
            }
            _ => {
                let m = prefix_multiplier(c)?;
                if multiplier.is_some() {
                    return None;
                }
                multiplier = Some(m);
                // A prefix acts as the decimal point unless one already appeared
                // (`1.2k`), in which case it only scales.
                point_seen = true;
            }
        }
    }

    if digits == 0 {
        return None;
    }
    Some(mantissa / 10f64.powi(decimals) * multiplier.unwrap_or(1.0))
}

fn prefix_multiplier(c: char) -> Option<f64> {
    Some(match c {
        'p' | 'P' => 1e-12,
        'n' | 'N' => 1e-9,
        'u' | 'U' | 'µ' | 'μ' => 1e-6,
        'm' => 1e-3,
        'R' | 'r' => 1.0,
        'k' | 'K' => 1e3,
        'M' => 1e6,
        'G' | 'g' => 1e9,
        'T' | 't' => 1e12,
        _ => return None,
    })
}

const PREFIXES: [char; 9] = ['p', 'n', 'u', 'm', 'R', 'k', 'M', 'G', 'T'];
const OHMS_INDEX: i32 = 4;

/// Format a value with three significant digits in RKM notation:
/// `4700` → `4k7`, `100` → `100R`, `2.2` → `2R2`, `1e6` → `1M`.
pub fn format(v: f64) -> String {
    format_digits(v, 3)
}

/// Format a value with `digits` (1–6) significant digits in RKM notation,
/// e.g. `format_digits(1229.43, 5)` → `1k2294`.
pub fn format_digits(v: f64, digits: i32) -> String {
    let digits = digits.clamp(1, 6);
    if !v.is_finite() {
        return "∞".to_string();
    }
    if v <= 0.0 {
        return "0R".to_string();
    }

    // Round to three significant digits first so 999.7 becomes "1k" rather than "1000R".
    let mag = v.log10().floor() as i32;
    let scale = 10f64.powi(mag - (digits - 1));
    let v = (v / scale).round() * scale;

    let mut idx = OHMS_INDEX;
    let mut x = v;
    while x < 1.0 && idx > 0 {
        x *= 1000.0;
        idx -= 1;
    }
    while x >= 999.5 && idx < PREFIXES.len() as i32 - 1 {
        x /= 1000.0;
        idx += 1;
    }

    // x is in [1, 1000): keep `digits` significant digits in all.
    let int_digits = if x >= 99.95 {
        3
    } else if x >= 9.995 {
        2
    } else {
        1
    };
    let decimals = (digits - int_digits).max(0) as usize;
    let text = format!("{x:.decimals$}");

    let prefix = PREFIXES[idx as usize];
    match text.split_once('.') {
        Some((int, frac)) => {
            let frac = frac.trim_end_matches('0');
            format!("{int}{prefix}{frac}")
        }
        None => format!("{text}{prefix}"),
    }
}

/// Format a percentage error with an explicit sign, e.g. `+0.42` / `-1.07`.
pub fn format_error(pct: f64) -> String {
    if pct.abs() < 0.005 {
        "0.00".to_string()
    } else {
        format!("{pct:+.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= b.abs() * 1e-12
    }

    #[test]
    fn parse_rkm() {
        assert!(close(parse("1k23").unwrap(), 1230.0));
        assert!(close(parse("4k7").unwrap(), 4700.0));
        assert!(close(parse("2R2").unwrap(), 2.2));
        assert!(close(parse("4r7").unwrap(), 4.7));
        assert!(close(parse("1M5").unwrap(), 1.5e6));
        assert!(close(parse("100R").unwrap(), 100.0));
        assert!(close(parse("470").unwrap(), 470.0));
        assert!(close(parse("10k").unwrap(), 10000.0));
        assert!(close(parse("2m2").unwrap(), 2.2e-3));
        assert!(close(parse("k47").unwrap(), 470.0));
    }

    #[test]
    fn parse_decimal_and_suffixes() {
        assert!(close(parse("4.7k").unwrap(), 4700.0));
        assert!(close(parse(" 33 kΩ ").unwrap(), 33000.0));
        assert!(close(parse("1.5 Mohm").unwrap(), 1.5e6));
        assert!(close(parse("0.5").unwrap(), 0.5));
        assert!(close(parse("2,2k").unwrap(), 2200.0));
    }

    #[test]
    fn parse_rejects_garbage() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("k"), None);
        assert_eq!(parse("1kk"), None);
        assert_eq!(parse("1x"), None);
        assert_eq!(parse("1.2.3"), None);
        assert_eq!(parse("4k7.1"), None);
    }

    #[test]
    fn format_rkm() {
        assert_eq!(format(1230.0), "1k23");
        assert_eq!(format(4700.0), "4k7");
        assert_eq!(format(10000.0), "10k");
        assert_eq!(format(100.0), "100R");
        assert_eq!(format(2.2), "2R2");
        assert_eq!(format(1e6), "1M");
        assert_eq!(format(0.47), "470m");
        assert_eq!(format(3296.7), "3k3");
        assert_eq!(format(999.7), "1k");
        assert_eq!(format(15.0), "15R");
        assert_eq!(format(976e3), "976k");
        assert_eq!(format_digits(1229.43, 5), "1k2294");
        assert_eq!(format_digits(1230.0, 5), "1k23");
        assert_eq!(format_digits(99.996, 4), "100R");
    }

    #[test]
    fn format_parse_round_trip() {
        for v in [
            1.0, 1.02, 4.7, 10.0, 47.5, 100.0, 1.13e3, 82e3, 1.5e6, 9.76e8,
        ] {
            let back = parse(&format(v)).unwrap();
            assert!(close(back, v), "{v} -> {} -> {back}", format(v));
        }
    }
}
