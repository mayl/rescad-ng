//! IEC 60062 resistor colour code.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub name: &'static str,
    /// sRGB.
    pub colour: [u8; 3],
}

const DIGITS: [Band; 10] = [
    Band {
        name: "black",
        colour: [20, 20, 20],
    },
    Band {
        name: "brown",
        colour: [130, 75, 30],
    },
    Band {
        name: "red",
        colour: [210, 30, 30],
    },
    Band {
        name: "orange",
        colour: [245, 135, 20],
    },
    Band {
        name: "yellow",
        colour: [245, 220, 30],
    },
    Band {
        name: "green",
        colour: [30, 160, 60],
    },
    Band {
        name: "blue",
        colour: [30, 80, 220],
    },
    Band {
        name: "violet",
        colour: [140, 60, 200],
    },
    Band {
        name: "grey",
        colour: [140, 140, 140],
    },
    Band {
        name: "white",
        colour: [250, 250, 250],
    },
];
const GOLD: Band = Band {
    name: "gold",
    colour: [212, 175, 55],
};
const SILVER: Band = Band {
    name: "silver",
    colour: [192, 192, 200],
};

fn multiplier(exp: i32) -> Option<Band> {
    match exp {
        -2 => Some(SILVER),
        -1 => Some(GOLD),
        0..=9 => Some(DIGITS[exp as usize]),
        _ => None,
    }
}

/// Colour bands for a value (rounded to three significant digits).
///
/// Values with a third significant digit, or any value when `precision` is
/// set (E48 and finer), get the five-band 1 % code; others get the four-band
/// 5 % code.  Returns an empty list for values the code cannot express.
pub fn bands(value: f64, precision: bool) -> Vec<Band> {
    if !value.is_finite() || value <= 0.0 {
        return Vec::new();
    }
    let mut exp = value.log10().floor() as i32 - 2;
    let mut m = (value / 10f64.powi(exp)).round() as i32;
    if m >= 1000 {
        m /= 10;
        exp += 1;
    }
    let (d1, d2, d3) = (m / 100, m / 10 % 10, m % 10);
    let out = if precision || d3 != 0 {
        multiplier(exp).map(|mul| {
            vec![
                DIGITS[d1 as usize],
                DIGITS[d2 as usize],
                DIGITS[d3 as usize],
                mul,
                DIGITS[1],
            ]
        })
    } else {
        multiplier(exp + 1).map(|mul| vec![DIGITS[d1 as usize], DIGITS[d2 as usize], mul, GOLD])
    };
    out.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: f64, precision: bool) -> Vec<&'static str> {
        bands(v, precision).iter().map(|b| b.name).collect()
    }

    #[test]
    fn four_band() {
        assert_eq!(names(4700.0, false), ["yellow", "violet", "red", "gold"]);
        assert_eq!(names(10.0, false), ["brown", "black", "black", "gold"]);
        assert_eq!(names(2.2, false), ["red", "red", "gold", "gold"]);
        assert_eq!(names(0.47, false), ["yellow", "violet", "silver", "gold"]);
    }

    #[test]
    fn five_band() {
        assert_eq!(
            names(1240.0, false),
            ["brown", "red", "yellow", "brown", "brown"]
        );
        assert_eq!(
            names(10_000.0, true),
            ["brown", "black", "black", "red", "brown"]
        );
    }

    #[test]
    fn out_of_range() {
        assert!(bands(0.0, false).is_empty());
        assert!(bands(0.001, true).is_empty());
    }
}
