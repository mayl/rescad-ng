//! What both front ends show: the calculator modes, turning the typed inputs
//! into results, and formatting result rows as table cells.

use crate::calc::{self, DividerError, Kind, Row};
use crate::eseries::SeriesMask;
use crate::units;

/// A calculator.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Mode {
    Nearest,
    Series,
    Parallel,
    Network,
    Divider,
    /// Divider with one leg made of two resistors.
    Divider3,
    TrimDivider,
    TrimResistor,
}

impl Mode {
    /// True for the modes that take a ratio rather than a single value.
    pub fn is_divider(self) -> bool {
        matches!(self, Mode::Divider | Mode::Divider3 | Mode::TrimDivider)
    }

    /// True for the trimmer modes, which take a trim range instead of a max error.
    pub fn is_trim(self) -> bool {
        matches!(self, Mode::TrimDivider | Mode::TrimResistor)
    }

    /// One-line description of what is searched for.
    pub fn hint(self) -> &'static str {
        match self {
            Mode::Nearest => "Closest preferred values",
            Mode::Series => "R1 + R2 in series",
            Mode::Parallel => "R1 || R2 in parallel",
            Mode::Network => "R1 + (R2 || R3)  or  R1 || (R2 + R3)",
            Mode::Divider => "ratio = R2 / (R1+R2)    RO = R1 || R2",
            Mode::Divider3 => "R1+R2 / R3,  R1||R2 / R3,  R1 / R2+R3  or  R1 / R2||R3",
            Mode::TrimDivider => "ratio = R2/Rt … (R2+P)/Rt,  Rt = R1+P+R2",
            Mode::TrimResistor => "Rs + P,  Rs + (P || Rp)  or  (Rs + P) || Rp",
        }
    }

    pub fn columns(self) -> &'static [&'static str] {
        match self {
            Mode::Nearest => &["% err", "R"],
            Mode::Series => &["% err", "R1", "R2", "R1+R2"],
            Mode::Parallel => &["% err", "R1", "R2", "R1||R2"],
            Mode::Network => &["% err", "R1", "R2", "R3", "Total", "Circuit"],
            Mode::Divider => &["% err", "R1", "R2", "Ratio", "RO"],
            Mode::Divider3 => &["% err", "R1", "R2", "R3", "Ratio", "RO", "Circuit"],
            Mode::TrimDivider => &["±adj %", "R1", "P", "R2", "Wiper", "RO"],
            Mode::TrimResistor => &["±adj %", "Rs", "P", "Rp", "Wiper", "Circuit"],
        }
    }

    /// The message shown when nothing qualifies.
    pub fn empty_message(self) -> &'static str {
        if self.is_trim() {
            "No combination covers the trim range"
        } else {
            "No combinations within the error limit"
        }
    }
}

/// The inputs as typed.
#[derive(Clone, PartialEq, Debug)]
pub struct Params {
    pub mode: Mode,
    pub value: String,
    pub ratio_num: String,
    pub ratio_den: String,
    pub ro_max: String,
    pub ro_min: String,
    pub mask: SeriesMask,
    pub max_err: f64,
    /// Required adjustment range, ± % of the target.
    pub trim: f64,
}

impl Default for Params {
    fn default() -> Self {
        let mask = SeriesMask::E12;
        Self {
            mode: Mode::Nearest,
            value: "1k23".into(),
            ratio_num: "0.5".into(),
            ratio_den: "1".into(),
            ro_max: "10k".into(),
            ro_min: "1k".into(),
            mask,
            max_err: mask.default_tolerance(),
            trim: auto_trim(Mode::TrimDivider, mask, Some(0.5)),
        }
    }
}

impl Params {
    /// The divider ratio typed in, if valid.
    pub fn ratio(&self) -> Option<f64> {
        ratio_of(&self.ratio_num, &self.ratio_den)
    }

    /// What a trimmer row's wiper position is measured against.
    fn trim_target(&self) -> f64 {
        match self.mode {
            Mode::TrimDivider => self.ratio(),
            _ => units::parse(&self.value),
        }
        .unwrap_or(f64::NAN)
    }

    /// The default trim range for the current mode, series and ratio.
    pub fn auto_trim(&self) -> f64 {
        auto_trim(self.mode, self.mask, self.ratio())
    }
}

/// The divider ratio `num / den`, if both parse.
pub fn ratio_of(num: &str, den: &str) -> Option<f64> {
    Some(units::parse(num)? / units::parse(den)?)
}

/// The trim range that covers the worst case of the selected series'
/// tolerance: for a divider both resistors may drift, moving the ratio by up
/// to 2 · (1 - ratio) · tolerance; for a resistor, by the tolerance itself.
pub fn auto_trim(mode: Mode, mask: SeriesMask, ratio: Option<f64>) -> f64 {
    let tol = mask.component_tolerance();
    match (mode, ratio) {
        (Mode::TrimDivider, Some(r)) if r > 0.0 && r < 1.0 => {
            ((2.0 * (1.0 - r) * tol * 100.0).round() / 100.0).max(0.01)
        }
        (Mode::TrimDivider, _) => tol,
        _ => tol,
    }
}

/// Run the selected calculator, or explain what is wrong with the inputs.
pub fn compute(p: &Params) -> Result<Vec<Row>, String> {
    let need =
        |text: &str, what: &str| units::parse(text).ok_or_else(|| format!("Enter a valid {what}"));
    let divider_inputs = || -> Result<(f64, f64, f64), String> {
        let num = need(&p.ratio_num, "ratio")?;
        let den = need(&p.ratio_den, "ratio")?;
        let ro_max = need(&p.ro_max, "maximum output resistance")?;
        let ro_min = need(&p.ro_min, "minimum output resistance")?;
        Ok((num / den, ro_min, ro_max))
    };
    let divider_error = |e| match e {
        DividerError::RatioOutOfRange => "Ratio must be between 0 and 1".to_string(),
        DividerError::BadInput => "Output resistance limits must be positive".to_string(),
        DividerError::TrimOutOfRange => "The trim range would take the ratio past 1".to_string(),
    };
    match p.mode {
        Mode::Nearest => Ok(calc::nearest(p.mask, need(&p.value, "value")?)),
        Mode::Series => Ok(calc::series(p.mask, need(&p.value, "value")?, p.max_err)),
        Mode::Parallel => Ok(calc::parallel(p.mask, need(&p.value, "value")?, p.max_err)),
        Mode::Network => Ok(calc::network(p.mask, need(&p.value, "value")?, p.max_err)),
        Mode::Divider => {
            let (ratio, ro_min, ro_max) = divider_inputs()?;
            calc::divider(p.mask, ratio, ro_min, ro_max, p.max_err).map_err(divider_error)
        }
        Mode::Divider3 => {
            let (ratio, ro_min, ro_max) = divider_inputs()?;
            calc::divider3(p.mask, ratio, ro_min, ro_max, p.max_err).map_err(divider_error)
        }
        Mode::TrimDivider => {
            let (ratio, ro_min, ro_max) = divider_inputs()?;
            calc::trim_divider(p.mask, ratio, ro_min, ro_max, p.trim).map_err(divider_error)
        }
        Mode::TrimResistor => calc::trim_resistor(p.mask, need(&p.value, "value")?, p.trim)
            .ok_or_else(|| "The trim range must be between 0 and 90 %".to_string()),
    }
}

/// A result row as table cells, matching [`Mode::columns`].
pub fn cells(p: &Params, row: &Row) -> Vec<String> {
    let r = &row.resistors;
    let ohms = units::format;
    if p.mode.is_trim() {
        let target = p.trim_target();
        let mut out = vec![format!("±{:.2}", row.error_pct), ohms(r[0]), ohms(r[1])];
        out.push(r.get(2).map_or("–".into(), |&v| ohms(v)));
        out.push(format!("{:.0}%", calc::wiper(row, target) * 100.0));
        out.push(if row.kind == Kind::PotDivider {
            ohms((r[0] + r[1] + r[2]) * target * (1.0 - target))
        } else {
            row.kind.circuit().into()
        });
        return out;
    }
    let mut out = vec![units::format_error(row.error_pct)];
    out.extend(r.iter().map(|&v| ohms(v)));
    match (p.mode, &r[..]) {
        (Mode::Network, &[a, b, c]) => {
            let total = if row.kind == Kind::SeriesParallel {
                a + b * c / (b + c)
            } else {
                a * (b + c) / (a + b + c)
            };
            out.push(units::format_digits(total, 5));
            out.push(row.kind.circuit().into());
        }
        (Mode::Series, &[a, b]) => out.push(ohms(a + b)),
        (Mode::Parallel, &[a, b]) => out.push(ohms(a * b / (a + b))),
        (Mode::Divider | Mode::Divider3, _) => {
            if let Some((top, bottom)) = row.kind.divider_legs(r) {
                out.push(format!("{:.5}", bottom / (top + bottom)));
                out.push(ohms(top * bottom / (top + bottom)));
            }
            if p.mode == Mode::Divider3 {
                out.push(row.kind.circuit().into());
            }
        }
        _ => {}
    }
    out
}

/// A row as tab-separated text, for the clipboard.
pub fn row_text(p: &Params, row: &Row) -> String {
    cells(p, row).join("\t")
}

/// All rows with a header line, tab-separated.
pub fn all_rows_text(p: &Params, rows: &[Row]) -> String {
    let mut text = p.mode.columns().join("\t");
    text.push('\n');
    for row in rows {
        text.push_str(&row_text(p, row));
        text.push('\n');
    }
    text
}

/// How good the first column is: 0 good, 1 fair, 2 poor.  Errors are rated
/// by size; trimmer ranges against the trim range asked for (a trimmer 20 %
/// low needs 1.25× it at best).
pub fn rating(p: &Params, row: &Row) -> u8 {
    let (x, good, fair) = if p.mode.is_trim() {
        (row.error_pct / p.trim, 1.6, 2.5)
    } else {
        (row.error_pct.abs(), 0.5, 2.0)
    };
    if x < good {
        0
    } else if x < fair {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_fills_its_columns() {
        for mode in [
            Mode::Nearest,
            Mode::Series,
            Mode::Parallel,
            Mode::Network,
            Mode::Divider,
            Mode::Divider3,
            Mode::TrimDivider,
            Mode::TrimResistor,
        ] {
            let p = Params {
                mode,
                ..Params::default()
            };
            let rows = compute(&p).unwrap();
            assert!(!rows.is_empty(), "{mode:?}");
            for row in &rows {
                assert_eq!(cells(&p, row).len(), mode.columns().len(), "{mode:?}");
            }
        }
    }

    #[test]
    fn bad_input_is_explained() {
        let p = Params {
            value: "4k7k".into(),
            ..Params::default()
        };
        assert_eq!(compute(&p), Err("Enter a valid value".into()));
    }
}
