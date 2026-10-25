//! IEC 60063 preferred-number (E-series) tables and nearest-value lookup.
//!
//! Every series is stored as a bit in a single membership table indexed by the
//! three significant digits of a value (100..=999), exactly like the original
//! ResCad.  A lookup is therefore a handful of array reads, regardless of how
//! many series are selected at once.

use std::fmt;

/// A set of E-series, stored as a bitmask.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SeriesMask(pub u8);

impl SeriesMask {
    pub const E6: Self = Self(1 << 0);
    pub const E12: Self = Self(1 << 1);
    pub const E24: Self = Self(1 << 2);
    pub const E48: Self = Self(1 << 3);
    pub const E96: Self = Self(1 << 4);
    pub const E192: Self = Self(1 << 5);

    /// All series in ascending order of density, with their display names.
    pub const ALL: [(Self, &'static str); 6] = [
        (Self::E6, "E6"),
        (Self::E12, "E12"),
        (Self::E24, "E24"),
        (Self::E48, "E48"),
        (Self::E96, "E96"),
        (Self::E192, "E192"),
    ];

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn toggle(&mut self, other: Self) {
        self.0 ^= other.0;
    }

    /// Default "max error" for a selection, following the original program:
    /// the finest selected series decides how far off a combination may be.
    pub fn default_tolerance(self) -> f64 {
        if self.contains(Self::E96) || self.contains(Self::E192) {
            2.0
        } else if self.contains(Self::E48) {
            4.0
        } else if self.contains(Self::E24) {
            8.0
        } else {
            9.99
        }
    }

    /// Component tolerance (%) of the coarsest selected series, per IEC 60063:
    /// the worst case when parts from all selected series may be used.
    pub fn component_tolerance(self) -> f64 {
        const TOL: [f64; 6] = [20.0, 10.0, 5.0, 2.0, 1.0, 0.5];
        Self::ALL
            .iter()
            .zip(TOL)
            .find(|((s, _), _)| self.contains(*s))
            .map_or(1.0, |(_, t)| t)
    }

    /// True when the selection contains a series whose values need three
    /// significant digits (and so five colour bands).
    pub fn is_precision(self) -> bool {
        self.0 & (Self::E48.0 | Self::E96.0 | Self::E192.0) != 0
    }
}

impl fmt::Debug for SeriesMask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = Self::ALL
            .iter()
            .filter(|(s, _)| self.contains(*s))
            .map(|(_, n)| *n)
            .collect();
        write!(f, "SeriesMask({})", names.join("|"))
    }
}

const E6: [u16; 6] = [100, 150, 220, 330, 470, 680];
const E12: [u16; 12] = [100, 120, 150, 180, 220, 270, 330, 390, 470, 560, 680, 820];
const E24: [u16; 24] = [
    100, 110, 120, 130, 150, 160, 180, 200, 220, 240, 270, 300, 330, 360, 390, 430, 470, 510, 560,
    620, 680, 750, 820, 910,
];
const E96: [u16; 96] = [
    100, 102, 105, 107, 110, 113, 115, 118, 121, 124, 127, 130, 133, 137, 140, 143, 147, 150, 154,
    158, 162, 165, 169, 174, 178, 182, 187, 191, 196, 200, 205, 210, 215, 221, 226, 232, 237, 243,
    249, 255, 261, 267, 274, 280, 287, 294, 301, 309, 316, 324, 332, 340, 348, 357, 365, 374, 383,
    392, 402, 412, 422, 432, 442, 453, 464, 475, 487, 499, 511, 523, 536, 549, 562, 576, 590, 604,
    619, 634, 649, 665, 681, 698, 715, 732, 750, 768, 787, 806, 825, 845, 866, 887, 909, 931, 953,
    976,
];
const E192: [u16; 192] = [
    100, 101, 102, 104, 105, 106, 107, 109, 110, 111, 113, 114, 115, 117, 118, 120, 121, 123, 124,
    126, 127, 129, 130, 132, 133, 135, 137, 138, 140, 142, 143, 145, 147, 149, 150, 152, 154, 156,
    158, 160, 162, 164, 165, 167, 169, 172, 174, 176, 178, 180, 182, 184, 187, 189, 191, 193, 196,
    198, 200, 203, 205, 208, 210, 213, 215, 218, 221, 223, 226, 229, 232, 234, 237, 240, 243, 246,
    249, 252, 255, 258, 261, 264, 267, 271, 274, 277, 280, 284, 287, 291, 294, 298, 301, 305, 309,
    312, 316, 320, 324, 328, 332, 336, 340, 344, 348, 352, 357, 361, 365, 370, 374, 379, 383, 388,
    392, 397, 402, 407, 412, 417, 422, 427, 432, 437, 442, 448, 453, 459, 464, 470, 475, 481, 487,
    493, 499, 505, 511, 517, 523, 530, 536, 542, 549, 556, 562, 569, 576, 583, 590, 597, 604, 612,
    619, 626, 634, 642, 649, 657, 665, 673, 681, 690, 698, 706, 715, 723, 732, 741, 750, 759, 768,
    777, 787, 796, 806, 816, 825, 835, 845, 856, 866, 876, 887, 898, 909, 920, 931, 942, 953, 965,
    976, 988,
];

/// Membership bitmask for each three-digit mantissa 0..=999 (only 100..=999 used).
static TABLE: [u8; 1000] = build_table();

const fn build_table() -> [u8; 1000] {
    let mut t = [0u8; 1000];
    let mut i = 0;
    while i < E6.len() {
        t[E6[i] as usize] |= SeriesMask::E6.0;
        i += 1;
    }
    i = 0;
    while i < E12.len() {
        t[E12[i] as usize] |= SeriesMask::E12.0;
        i += 1;
    }
    i = 0;
    while i < E24.len() {
        t[E24[i] as usize] |= SeriesMask::E24.0;
        i += 1;
    }
    i = 0;
    while i < E96.len() {
        // E48 is every other E96 value.
        if i % 2 == 0 {
            t[E96[i] as usize] |= SeriesMask::E48.0;
        }
        t[E96[i] as usize] |= SeriesMask::E96.0;
        i += 1;
    }
    i = 0;
    while i < E192.len() {
        t[E192[i] as usize] |= SeriesMask::E192.0;
        i += 1;
    }
    t
}

fn member(mask: SeriesMask, mantissa: i32) -> bool {
    TABLE[mantissa as usize] & mask.0 != 0
}

fn value(mantissa: i32, exp: i32) -> f64 {
    // Multiplying/dividing by an exact power of ten keeps values such as 4.7
    // as close to their decimal spelling as f64 allows.
    if exp >= 0 {
        mantissa as f64 * 10f64.powi(exp)
    } else {
        mantissa as f64 / 10f64.powi(-exp)
    }
}

/// The preferred values around a target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neighbours {
    /// Largest preferred value strictly below the target's 3-digit rounding.
    pub below: f64,
    /// The preferred value closest to the target.
    pub nearest: f64,
    /// Smallest preferred value strictly above the target's 3-digit rounding.
    pub above: f64,
}

/// Find the preferred values around `v` in the selected series.
///
/// `v` is first rounded to three significant digits (the resolution of
/// E192); `below`/`above` are the neighbours strictly either side of that
/// rounding and `nearest` is the rounding itself when it is a preferred value,
/// else whichever neighbour is closer (ties go to the lower one).
pub fn neighbours(mask: SeriesMask, v: f64) -> Neighbours {
    if mask.is_empty() || !v.is_finite() || v <= 0.0 {
        return Neighbours {
            below: v,
            nearest: v,
            above: v,
        };
    }

    // Normalise to a mantissa in [100, 1000).
    let mut exp = v.log10().floor() as i32 - 2;
    let mut norm = v / 10f64.powi(exp);
    if norm >= 1000.0 {
        norm /= 10.0;
        exp += 1;
    } else if norm < 100.0 {
        norm *= 10.0;
        exp -= 1;
    }
    let mut n = norm.round() as i32;
    if n >= 1000 {
        n = 100;
        exp += 1;
    }

    let (mut k, mut e) = (n, exp);
    loop {
        k -= 1;
        if k < 100 {
            k = 999;
            e -= 1;
        }
        if member(mask, k) {
            break;
        }
    }
    let below = value(k, e);

    let (mut k, mut e) = (n, exp);
    loop {
        k += 1;
        if k > 999 {
            k = 100;
            e += 1;
        }
        if member(mask, k) {
            break;
        }
    }
    let above = value(k, e);

    let nearest = if member(mask, n) {
        value(n, exp)
    } else if (v - below).abs() > (v - above).abs() {
        above
    } else {
        below
    };

    Neighbours {
        below,
        nearest,
        above,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_sizes() {
        for (mask, name) in SeriesMask::ALL {
            let count = (100..1000).filter(|&k| member(mask, k)).count();
            let expected: usize = name[1..].parse().unwrap();
            assert_eq!(count, expected, "{name}");
        }
    }

    #[test]
    fn coarser_series_are_subsets() {
        for k in 100..1000 {
            if member(SeriesMask::E6, k) {
                assert!(member(SeriesMask::E12, k));
            }
            if member(SeriesMask::E12, k) {
                assert!(member(SeriesMask::E24, k));
            }
            if member(SeriesMask::E48, k) {
                assert!(member(SeriesMask::E96, k));
            }
            if member(SeriesMask::E96, k) {
                assert!(member(SeriesMask::E192, k));
            }
        }
    }

    #[test]
    fn nearest_e12() {
        let n = neighbours(SeriesMask::E12, 1230.0);
        assert_eq!(n.below, 1200.0);
        assert_eq!(n.nearest, 1200.0);
        assert_eq!(n.above, 1500.0);
    }

    #[test]
    fn exact_value_has_distinct_neighbours() {
        let n = neighbours(SeriesMask::E24, 4700.0);
        assert_eq!(n.nearest, 4700.0);
        assert_eq!(n.below, 4300.0);
        assert_eq!(n.above, 5100.0);
    }

    #[test]
    fn decade_wrap() {
        let n = neighbours(SeriesMask::E6, 100.0);
        assert_eq!(n.below, 68.0);
        assert_eq!(n.above, 150.0);
        let n = neighbours(SeriesMask::E6, 800.0);
        assert_eq!(n.nearest, 680.0);
        assert_eq!(n.above, 1000.0);
        let n = neighbours(SeriesMask::E6, 999.7);
        assert_eq!(n.nearest, 1000.0);
    }

    #[test]
    fn small_values() {
        let n = neighbours(SeriesMask::E12, 4.6);
        assert_eq!(n.nearest, 4.7);
        let n = neighbours(SeriesMask::E96, 0.0103);
        assert_eq!(n.nearest, 0.0102);
    }

    #[test]
    fn combined_series() {
        let mask = SeriesMask(SeriesMask::E6.0 | SeriesMask::E96.0);
        let n = neighbours(mask, 1500.0);
        assert_eq!(n.nearest, 1500.0);
        assert_eq!(n.below, 1470.0);
        assert_eq!(n.above, 1540.0);
    }
}
