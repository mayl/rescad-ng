//! The ResCad calculators: nearest value, series pair, parallel pair and
//! voltage divider, whose search strategies follow the original program, plus
//! the trimmable divider.  Results are returned sorted best first.

use crate::eseries::{SeriesMask, neighbours};

/// Upper bound on loop iterations, so absurd inputs can never hang the UI.
const MAX_STEPS: usize = 100_000;

/// Results longer than this are cut off; nobody scrolls further.
const MAX_ROWS: usize = 2000;

/// The circuit a result row describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One resistor, a series or parallel pair, or a divider R1 / R2.
    Plain,
    /// R1 + (R2 ∥ R3).
    SeriesParallel,
    /// R1 ∥ (R2 + R3).
    ParallelSeries,
    /// R1, trimmer, R2 from Vin to ground, output on the wiper.
    PotDivider,
    /// Rs + P.
    PotSeries,
    /// Rs + (P ∥ Rp).
    PotSeriesShunt,
    /// (Rs + P) ∥ Rp.
    PotShunt,
}

impl Kind {
    /// Short circuit description, for the results table.
    pub fn circuit(self) -> &'static str {
        match self {
            Kind::Plain => "",
            Kind::SeriesParallel => "R1 + R2||R3",
            Kind::ParallelSeries => "R1 || (R2+R3)",
            Kind::PotDivider => "R1 – P – R2",
            Kind::PotSeries => "Rs + P",
            Kind::PotSeriesShunt => "Rs + P||Rp",
            Kind::PotShunt => "(Rs+P) || Rp",
        }
    }

    /// Names of the parts in [`Row::resistors`], in order.
    pub fn labels(self, parts: usize) -> &'static [&'static str] {
        match self {
            Kind::Plain if parts == 1 => &["R"],
            Kind::Plain => &["R1", "R2"],
            Kind::SeriesParallel | Kind::ParallelSeries => &["R1", "R2", "R3"],
            Kind::PotDivider => &["R1", "P", "R2"],
            Kind::PotSeries => &["Rs", "P"],
            Kind::PotSeriesShunt | Kind::PotShunt => &["Rs", "P", "Rp"],
        }
    }

    /// Index of the trimmer in [`Row::resistors`], if there is one.
    pub fn pot_index(self) -> Option<usize> {
        match self {
            Kind::PotDivider | Kind::PotSeries | Kind::PotSeriesShunt | Kind::PotShunt => Some(1),
            _ => None,
        }
    }
}

/// One line of results.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// Percentage error, or for trimmer circuits half the adjustment range as a
    /// percentage of the target (smaller is finer).
    pub error_pct: f64,
    /// The parts making up the solution, named by [`Kind::labels`].
    pub resistors: Vec<f64>,
    pub kind: Kind,
}

fn par(a: f64, b: f64) -> f64 {
    a * b / (a + b)
}

fn pct(actual: f64, wanted: f64) -> f64 {
    (actual - wanted) / wanted * 100.0
}

fn sort(rows: &mut [Row]) {
    rows.sort_by(|a, b| {
        a.error_pct
            .abs()
            .total_cmp(&b.error_pct.abs())
            .then_with(|| a.resistors[0].total_cmp(&b.resistors[0]))
    });
}

fn usable(v: f64) -> bool {
    v.is_finite() && v > 0.0
}

/// The preferred value closest to `target`, plus its neighbours either side.
pub fn nearest(mask: SeriesMask, target: f64) -> Vec<Row> {
    if mask.is_empty() || !usable(target) {
        return Vec::new();
    }
    let best = neighbours(mask, target).nearest;
    let around = neighbours(mask, best);
    let mut rows: Vec<Row> = [best, around.below, around.above]
        .into_iter()
        .map(|v| Row {
            error_pct: pct(v, target),
            resistors: vec![v],
            kind: Kind::Plain,
        })
        .collect();
    sort(&mut rows);
    rows
}

/// Pairs R1 + R2 ≈ `target`.  R1 walks down the series from the nearest value
/// to just under half the target; R2 is the best match for the remainder.
pub fn series(mask: SeriesMask, target: f64, max_err: f64) -> Vec<Row> {
    let mut rows = Vec::new();
    if mask.is_empty() || !usable(target) {
        return rows;
    }
    let limit = target / 2.01;
    let mut r1 = neighbours(mask, target).nearest;
    for _ in 0..MAX_STEPS {
        if r1 <= limit {
            break;
        }
        if target > r1 {
            let r2 = neighbours(mask, target - r1).nearest;
            let err = pct(r1 + r2, target);
            if err.abs() < max_err {
                // Show the larger resistor first, as the original did.
                rows.push(Row {
                    error_pct: err,
                    resistors: vec![r1.max(r2), r1.min(r2)],
                    kind: Kind::Plain,
                });
            }
        }
        r1 = neighbours(mask, r1).below;
    }
    sort(&mut rows);
    rows
}

/// Pairs R1 ∥ R2 ≈ `target`.  R1 walks up the series from the nearest value
/// to just over twice the target; R2 is the best match for the remainder.
pub fn parallel(mask: SeriesMask, target: f64, max_err: f64) -> Vec<Row> {
    let mut rows = Vec::new();
    if mask.is_empty() || !usable(target) {
        return rows;
    }
    let limit = target * 2.01;
    let mut r1 = neighbours(mask, target).nearest;
    for _ in 0..MAX_STEPS {
        if r1 >= limit {
            break;
        }
        if r1 > target {
            let r2 = neighbours(mask, target * r1 / (r1 - target)).nearest;
            let rp = r1 * r2 / (r1 + r2);
            let err = pct(rp, target);
            if err.abs() < max_err {
                // Show the smaller resistor first, as the original did.
                rows.push(Row {
                    error_pct: err,
                    resistors: vec![r1.min(r2), r1.max(r2)],
                    kind: Kind::Plain,
                });
            }
        }
        r1 = neighbours(mask, r1).above;
    }
    sort(&mut rows);
    rows
}

#[derive(Clone, Debug, PartialEq)]
pub enum DividerError {
    RatioOutOfRange,
    BadInput,
    /// The requested trim range would reach 0 or 1.
    TrimOutOfRange,
}

/// Voltage dividers Vout/Vin = R2 / (R1 + R2) ≈ `ratio`, keeping the output
/// (Thévenin) resistance R1 ∥ R2 within `[ro_min, ro_max]`.
///
/// R1 is the top resistor (Vin to Vout) and R2 the bottom (Vout to ground).
/// Since R1 ∥ R2 = R1 · ratio, R1 sweeps the range implied by the output
/// resistance limits and R2 is tried at the preferred values either side of
/// its ideal value.
pub fn divider(
    mask: SeriesMask,
    ratio: f64,
    ro_min: f64,
    ro_max: f64,
    max_err: f64,
) -> Result<Vec<Row>, DividerError> {
    if !ratio.is_finite() || ratio <= 0.0 || ratio > 0.9999 {
        return Err(DividerError::RatioOutOfRange);
    }
    if !usable(ro_min) || !usable(ro_max) {
        return Err(DividerError::BadInput);
    }
    let mut rows = Vec::new();
    if mask.is_empty() {
        return Ok(rows);
    }
    let (ro_min, ro_max) = if ro_min <= ro_max {
        (ro_min, ro_max)
    } else {
        (ro_max, ro_min)
    };

    let r1_end = neighbours(mask, ro_max / ratio).above;
    let mut r1 = neighbours(mask, ro_min / ratio).below;
    for _ in 0..MAX_STEPS {
        if r1 >= r1_end {
            break;
        }
        let ideal_r2 = ratio * r1 / (1.0 - ratio);
        let n = neighbours(mask, ideal_r2);
        let mut candidates = vec![n.below, n.above];
        if n.nearest != n.below && n.nearest != n.above {
            candidates.push(n.nearest);
        }
        for r2 in candidates {
            let ro = r1 * r2 / (r1 + r2);
            if ro < ro_min || ro > ro_max {
                continue;
            }
            let err = pct(r2 / (r1 + r2), ratio);
            if err.abs() < max_err {
                rows.push(Row {
                    error_pct: err,
                    resistors: vec![r1, r2],
                    kind: Kind::Plain,
                });
            }
        }
        r1 = neighbours(mask, r1).above;
    }
    sort(&mut rows);
    Ok(rows)
}

/// Three-resistor networks ≈ `target`: R1 + (R2 ∥ R3), where R1 carries at
/// least half the value and the parallel pair makes up the rest, and
/// R1 ∥ (R2 + R3), where R1 is at most twice the value and the series pair
/// shunts it down.  These get far closer than any pair from the same series.
pub fn network(mask: SeriesMask, target: f64, max_err: f64) -> Vec<Row> {
    let mut rows = Vec::new();
    if mask.is_empty() || !usable(target) {
        return rows;
    }
    let mut steps = 0;
    let push = |rows: &mut Vec<Row>, total: f64, resistors: Vec<f64>, kind| {
        let err = pct(total, target);
        if err.abs() < max_err {
            rows.push(Row {
                error_pct: err,
                resistors,
                kind,
            });
        }
    };

    // R1 + (R2 ∥ R3): R2 walks up from the remainder to twice it, R3 completes the pair.
    let mut r1 = neighbours(mask, target).below;
    while r1 >= target / 2.0 && steps < MAX_STEPS {
        let rest = target - r1;
        let mut r2 = neighbours(mask, rest).above;
        while r2 <= rest * 2.01 && steps < MAX_STEPS {
            steps += 1;
            let r3 = neighbours(mask, rest * r2 / (r2 - rest)).nearest;
            push(
                &mut rows,
                r1 + par(r2, r3),
                vec![r1, r2.min(r3), r2.max(r3)],
                Kind::SeriesParallel,
            );
            r2 = neighbours(mask, r2).above;
        }
        r1 = neighbours(mask, r1).below;
    }

    // R1 ∥ (R2 + R3): R2 walks down from the needed branch value to half of it.
    let mut r1 = neighbours(mask, target).above;
    while r1 <= target * 2.01 && steps < MAX_STEPS {
        let branch = target * r1 / (r1 - target);
        let mut r2 = neighbours(mask, branch).below;
        while r2 >= branch / 2.01 && steps < MAX_STEPS {
            steps += 1;
            let r3 = neighbours(mask, branch - r2).nearest;
            push(
                &mut rows,
                par(r1, r2 + r3),
                vec![r1, r2.max(r3), r2.min(r3)],
                Kind::ParallelSeries,
            );
            r2 = neighbours(mask, r2).below;
        }
        r1 = neighbours(mask, r1).above;
    }

    sort(&mut rows);
    rows.dedup();
    rows.truncate(MAX_ROWS);
    rows
}

/// Trimmer tolerance the trim searches allow for; ±20 % is typical for trimmers.
pub const POT_TOLERANCE: f64 = 0.2;

/// Trimmer values searched: the usual 1-2-5 sequence from 10R to 2M.
const POTS: [f64; 17] = [
    10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1e3, 2e3, 5e3, 10e3, 20e3, 50e3, 100e3, 200e3, 500e3,
    1e6, 2e6,
];

/// The adjustment range `(min, max)` of a trimmer row, with the trimmer
/// scaled by `pot_scale` (1 for nominal).
fn trim_range(kind: Kind, r: &[f64], pot_scale: f64) -> (f64, f64) {
    let p = r[1] * pot_scale;
    match kind {
        Kind::PotDivider => {
            let rt = r[0] + p + r[2];
            (r[2] / rt, (r[2] + p) / rt)
        }
        Kind::PotSeries => (r[0], r[0] + p),
        Kind::PotSeriesShunt => (r[0], r[0] + par(p, r[2])),
        Kind::PotShunt => (par(r[0], r[2]), par(r[0] + p, r[2])),
        _ => (f64::NAN, f64::NAN),
    }
}

/// Wiper position (0–1) at which a trimmer row gives `target`.  For a divider
/// it is measured from the R2 end; for a variable resistor it is the
/// fraction of the track in circuit.
pub fn wiper(row: &Row, target: f64) -> f64 {
    let r = &row.resistors;
    match row.kind {
        Kind::PotDivider => (target * (r[0] + r[1] + r[2]) - r[2]) / r[1],
        Kind::PotSeries => (target - r[0]) / r[1],
        Kind::PotSeriesShunt => {
            let d = target - r[0];
            d * r[2] / (r[2] - d) / r[1]
        }
        Kind::PotShunt => (target * r[2] / (r[2] - target) - r[0]) / r[1],
        _ => f64::NAN,
    }
}

/// Keep a trimmer candidate if its range covers `[lo_need, hi_need]` even
/// with the trimmer at the low end of its tolerance, and the target lies in
/// the middle half of the trimmer's travel.
fn push_trim(rows: &mut Vec<Row>, kind: Kind, resistors: Vec<f64>, target: f64, need: (f64, f64)) {
    if resistors.iter().any(|&v| !usable(v)) {
        return;
    }
    let (lo, hi) = trim_range(kind, &resistors, 1.0 - POT_TOLERANCE);
    if lo <= need.0 && hi >= need.1 {
        let (lo, hi) = trim_range(kind, &resistors, 1.0);
        let row = Row {
            error_pct: (hi - lo) / 2.0 / target * 100.0,
            resistors,
            kind,
        };
        // A heavily shunted trimmer may only reach the target at the very end
        // of its travel, where the adjustment is coarse; keep it mid-travel.
        if (0.25..=0.75).contains(&wiper(&row, target)) {
            rows.push(row);
        }
    }
}

/// Finest adjustment first, then the target nearest mid-travel.
fn sort_trim(rows: &mut Vec<Row>, target: f64) {
    let offset = |r: &Row| (wiper(r, target) - 0.5).abs();
    rows.sort_by(|a, b| {
        a.error_pct
            .total_cmp(&b.error_pct)
            .then_with(|| offset(a).total_cmp(&offset(b)))
    });
    rows.dedup();
    rows.truncate(MAX_ROWS);
}

/// The preferred values either side of `v` (and `v`'s nearest, if distinct).
fn candidates(mask: SeriesMask, v: f64) -> Vec<f64> {
    let n = neighbours(mask, v);
    let mut out = vec![n.below, n.above];
    if n.nearest != n.below && n.nearest != n.above {
        out.push(n.nearest);
    }
    out
}

/// Trimmable voltage dividers: R1 from Vin to the top of a trimmer P, R2 from
/// its bottom to ground, Vout on the wiper.  The wiper reaches any ratio in
/// `R2 / Rt ..= (R2 + P) / Rt`, where `Rt = R1 + P + R2`.
///
/// A combination qualifies when that range covers `ratio` ± `trim_pct` % even
/// with the trimmer [`POT_TOLERANCE`] low, so the target stays reachable over
/// the part tolerances.  The output resistance with the wiper at the target,
/// `Rt · ratio · (1 - ratio)`, must lie within `[ro_min, ro_max]`.  For each
/// trimmer and R2, R1 is chosen to put the target nearest mid-travel.
pub fn trim_divider(
    mask: SeriesMask,
    ratio: f64,
    ro_min: f64,
    ro_max: f64,
    trim_pct: f64,
) -> Result<Vec<Row>, DividerError> {
    if !ratio.is_finite() || ratio <= 0.0 || ratio > 0.9999 {
        return Err(DividerError::RatioOutOfRange);
    }
    if !usable(ro_min) || !usable(ro_max) {
        return Err(DividerError::BadInput);
    }
    let t = trim_pct / 100.0;
    if !(t > 0.0 && t < 1.0 && ratio * (1.0 + t) < 1.0) {
        return Err(DividerError::TrimOutOfRange);
    }
    let mut rows = Vec::new();
    if mask.is_empty() {
        return Ok(rows);
    }
    let (ro_min, ro_max) = if ro_min <= ro_max {
        (ro_min, ro_max)
    } else {
        (ro_max, ro_min)
    };

    let need = (ratio * (1.0 - t), ratio * (1.0 + t));
    let k = ratio * (1.0 - ratio);
    let (rt_min, rt_max) = (ro_min / k, ro_max / k);
    let mut steps = 0;
    for p in POTS {
        if p >= rt_max {
            break;
        }
        // Coverage needs R2 ≤ lo·Rt, and R2 ≥ hi·Rt - P.
        let start = (need.1 * rt_min - p).max(need.0 * rt_min * 1e-3);
        let mut r2 = neighbours(mask, start).below;
        while r2 <= need.0 * rt_max * 1.01 && steps < MAX_STEPS {
            steps += 1;
            // R1 that puts the target at mid-travel: ratio · Rt = R2 + P / 2.
            let ideal_r1 = (r2 + p / 2.0) / ratio - r2 - p;
            if ideal_r1 > 0.0 {
                for r1 in candidates(mask, ideal_r1) {
                    let rt = r1 + p + r2;
                    if (rt_min..=rt_max).contains(&rt) {
                        push_trim(&mut rows, Kind::PotDivider, vec![r1, p, r2], ratio, need);
                    }
                }
            }
            r2 = neighbours(mask, r2).above;
        }
    }
    sort_trim(&mut rows, ratio);
    Ok(rows)
}

/// Variable resistors adjustable over `target` ± `trim_pct` %, built from a
/// trimmer P padded with fixed resistors: Rs + P, Rs + (P ∥ Rp) and
/// (Rs + P) ∥ Rp.  The padding narrows the range to just what is needed, so
/// the adjustment is much finer than a bare trimmer's.  Coverage allows for
/// [`POT_TOLERANCE`] as in [`trim_divider`].  Returns `None` for an invalid
/// trim range.
pub fn trim_resistor(mask: SeriesMask, target: f64, trim_pct: f64) -> Option<Vec<Row>> {
    let t = trim_pct / 100.0;
    if !(t > 0.0 && t < 1.0) {
        return None;
    }
    let mut rows = Vec::new();
    if mask.is_empty() || !usable(target) {
        return Some(rows);
    }
    let need = (target * (1.0 - t), target * (1.0 + t));
    let mut steps = 0;
    for p in POTS {
        // Rs + P, centred: Rs = target - P/2.
        if target - p / 2.0 > 0.0 {
            for rs in candidates(mask, target - p / 2.0) {
                push_trim(&mut rows, Kind::PotSeries, vec![rs, p], target, need);
            }
        }

        // Rs + (P ∥ Rp): Rs is the main part; the shunted trimmer spans
        // twice the remainder.
        let mut rs = neighbours(mask, need.0 * 1.001).below;
        while rs >= target / 2.0 && rs > target - p / 2.0 && steps < MAX_STEPS {
            steps += 1;
            let q = 2.0 * (target - rs);
            for rp in candidates(mask, q * p / (p - q)) {
                push_trim(&mut rows, Kind::PotSeriesShunt, vec![rs, p, rp], target, need);
            }
            rs = neighbours(mask, rs).below;
        }

        // (Rs + P) ∥ Rp: Rp shunts the branch down; Rp walks up to 10× target.
        let mut rp = neighbours(mask, need.1).above;
        while rp <= target * 10.0 && steps < MAX_STEPS {
            steps += 1;
            let rs = target * rp / (rp - target) - p / 2.0;
            if rs > 0.0 {
                for rs in candidates(mask, rs) {
                    push_trim(&mut rows, Kind::PotShunt, vec![rs, p, rp], target, need);
                }
            }
            rp = neighbours(mask, rp).above;
        }
    }
    sort_trim(&mut rows, target);
    Some(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    const E12: SeriesMask = SeriesMask::E12;
    const E24: SeriesMask = SeriesMask::E24;

    #[test]
    fn nearest_lists_neighbours() {
        let rows = nearest(E12, 1230.0);
        let vals: Vec<f64> = rows.iter().map(|r| r.resistors[0]).collect();
        assert_eq!(vals, vec![1200.0, 1000.0, 1500.0]);
        assert!((rows[0].error_pct - (-2.439)).abs() < 0.01);
    }

    #[test]
    fn series_finds_exact_pairs() {
        // 1k6 has no E12 match but 1k + 560 ≈ 1k56 and 820 + 820 = 1k64 are close.
        let rows = series(E12, 1600.0, 9.99);
        assert!(!rows.is_empty());
        for r in &rows {
            let sum = r.resistors[0] + r.resistors[1];
            assert!((sum - 1600.0).abs() / 1600.0 * 100.0 < 9.99);
        }
        // 2k = 1k + 1k is exact in E6.
        let rows = series(SeriesMask::E6, 2000.0, 9.99);
        assert_eq!(rows[0].resistors, vec![1000.0, 1000.0]);
        assert_eq!(rows[0].error_pct, 0.0);
    }

    #[test]
    fn parallel_finds_exact_pairs() {
        // 1k ∥ 1k = 500R; 750R ∥ 3k = 600R.
        let rows = parallel(SeriesMask::E6, 500.0, 9.99);
        assert_eq!(rows[0].resistors, vec![1000.0, 1000.0]);
        let rows = parallel(E24, 600.0, 8.0);
        assert!(
            rows.iter()
                .any(|r| r.resistors == vec![750.0, 3000.0] && r.error_pct.abs() < 1e-9)
        );
        for r in &rows {
            let (a, b) = (r.resistors[0], r.resistors[1]);
            let rp = a * b / (a + b);
            assert!((rp - 600.0).abs() / 600.0 * 100.0 < 8.0);
        }
    }

    #[test]
    fn divider_half() {
        let rows = divider(E12, 0.5, 1000.0, 10000.0, 9.99).unwrap();
        assert!(!rows.is_empty());
        let best = &rows[0];
        assert_eq!(best.error_pct, 0.0);
        assert_eq!(best.resistors[0], best.resistors[1]);
        for r in &rows {
            let (r1, r2) = (r.resistors[0], r.resistors[1]);
            let ro = r1 * r2 / (r1 + r2);
            assert!((1000.0..=10000.0).contains(&ro));
        }
    }

    #[test]
    fn divider_rejects_bad_ratio() {
        assert_eq!(
            divider(E12, 1.0, 1e3, 1e4, 5.0),
            Err(DividerError::RatioOutOfRange)
        );
        assert_eq!(
            divider(E12, 0.0, 1e3, 1e4, 5.0),
            Err(DividerError::RatioOutOfRange)
        );
        assert_eq!(
            divider(E12, 0.5, 0.0, 1e4, 5.0),
            Err(DividerError::BadInput)
        );
    }

    #[test]
    fn empty_mask_is_harmless() {
        let none = SeriesMask::default();
        assert!(nearest(none, 1000.0).is_empty());
        assert!(series(none, 1000.0, 5.0).is_empty());
        assert!(parallel(none, 1000.0, 5.0).is_empty());
        assert!(divider(none, 0.5, 1e3, 1e4, 5.0).unwrap().is_empty());
        assert!(network(none, 1000.0, 5.0).is_empty());
        assert!(trim_divider(none, 0.5, 1e3, 1e4, 1.0).unwrap().is_empty());
        assert!(trim_resistor(none, 1000.0, 1.0).unwrap().is_empty());
    }

    #[test]
    fn extreme_inputs_terminate() {
        let all = SeriesMask(0x3f);
        let _ = series(all, 1e15, 2.0);
        let _ = parallel(all, 1e-15, 2.0);
        let _ = divider(all, 0.0001, 1e-3, 1e12, 2.0).unwrap();
        let _ = network(all, 1e15, 50.0);
        let _ = network(all, 1e-15, 50.0);
        let _ = trim_divider(all, 0.0001, 1e-3, 1e12, 0.01).unwrap();
        let _ = trim_divider(all, 0.99, 1e-3, 1e12, 0.5).unwrap();
        let _ = trim_resistor(all, 1e-6, 0.01).unwrap();
        let _ = trim_resistor(all, 1e12, 90.0).unwrap();
    }

    fn total(r: &Row) -> f64 {
        let v = &r.resistors;
        match r.kind {
            Kind::SeriesParallel => v[0] + par(v[1], v[2]),
            Kind::ParallelSeries => par(v[0], v[1] + v[2]),
            _ => unreachable!(),
        }
    }

    #[test]
    fn network_beats_pairs() {
        // 1k23 from E12: the best pair is 1k2 + 27R (-0.24 %); three parts do better.
        let rows = network(E12, 1230.0, 1.0);
        assert!(!rows.is_empty());
        assert!(rows[0].error_pct.abs() < 0.05);
        assert!(rows.iter().any(|r| r.kind == Kind::SeriesParallel));
        assert!(rows.iter().any(|r| r.kind == Kind::ParallelSeries));
        for r in &rows {
            assert!((pct(total(r), 1230.0) - r.error_pct).abs() < 1e-9);
            assert!(r.error_pct.abs() < 1.0);
        }
    }

    #[test]
    fn trim_divider_covers_target_with_low_trimmer() {
        let ratio = 2.5 / 3.3;
        let rows = trim_divider(SeriesMask::E96, ratio, 1e3, 10e3, 0.5).unwrap();
        assert!(!rows.is_empty());
        for r in &rows {
            let [r1, p, r2] = r.resistors[..] else { panic!() };
            assert!(POTS.contains(&p));
            let pl = p * (1.0 - POT_TOLERANCE);
            let rt = r1 + pl + r2;
            assert!(r2 / rt <= ratio * 0.995 + 1e-12);
            assert!((r2 + pl) / rt >= ratio * 1.005 - 1e-12);
            let ro = (r1 + p + r2) * ratio * (1.0 - ratio);
            assert!((1e3..=10e3).contains(&ro));
            assert!((0.0..=1.0).contains(&wiper(r, ratio)));
        }
        assert!(rows.windows(2).all(|w| w[0].error_pct <= w[1].error_pct));
        // Much finer than a bare trimmer, yet it still covers ±0.5 %.
        assert!(rows[0].error_pct < 2.0);
    }

    #[test]
    fn trim_resistor_covers_target() {
        let target = 3570.0;
        let rows = trim_resistor(E24, target, 5.0).unwrap();
        for kind in [Kind::PotSeries, Kind::PotSeriesShunt, Kind::PotShunt] {
            assert!(rows.iter().any(|r| r.kind == kind), "{kind:?}");
        }
        for r in &rows {
            let (lo, hi) = trim_range(r.kind, &r.resistors, 1.0 - POT_TOLERANCE);
            assert!(lo <= target * 0.95 + 1e-9 && hi >= target * 1.05 - 1e-9);
            let w = wiper(r, target);
            assert!((0.25..=0.75).contains(&w), "{r:?} wiper {w}");
        }
        assert!(rows.windows(2).all(|w| w[0].error_pct <= w[1].error_pct));
        // Padding gets the range close to the ±5 % needed (±5 % / 0.8 = ±6.25 %).
        assert!(rows[0].error_pct < 8.0);
    }

    #[test]
    fn trim_rejects_bad_range() {
        assert_eq!(
            trim_divider(E12, 0.9, 1e3, 1e4, 20.0),
            Err(DividerError::TrimOutOfRange)
        );
        assert_eq!(
            trim_divider(E12, 0.5, 1e3, 1e4, 0.0),
            Err(DividerError::TrimOutOfRange)
        );
        assert_eq!(
            trim_divider(E12, 1.5, 1e3, 1e4, 1.0),
            Err(DividerError::RatioOutOfRange)
        );
        assert_eq!(trim_resistor(E12, 1e3, 0.0), None);
    }
}
