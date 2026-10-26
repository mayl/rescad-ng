//! Little schematics of result circuits, as shapes for the front ends to
//! draw: an icon in each result row, and a larger labelled one of the
//! selected row.  Coordinates are in abstract units, y downwards; a resistor
//! body is 10 × 5 units.

use crate::calc::Kind;

/// One thing to draw.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Line {
        from: (f32, f32),
        to: (f32, f32),
    },
    /// A resistor body with its top-left corner at `x, y`, named `label`;
    /// a trimmer has an arrow through it.
    Resistor {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        label: &'static str,
        trimmer: bool,
    },
    /// A junction or output node.
    Dot {
        x: f32,
        y: f32,
    },
    /// An open-circle terminal.
    Terminal {
        x: f32,
        y: f32,
    },
    /// A ground symbol on the right-hand end of a line at `x, y`.
    Ground {
        x: f32,
        y: f32,
    },
}

/// A circuit's shapes and the size of their bounding box.
#[derive(Clone, Debug, PartialEq)]
pub struct Sketch {
    pub width: f32,
    pub height: f32,
    pub shapes: Vec<Shape>,
}

/// Radius of terminals and dots.
pub const DOT_RADIUS: f32 = 1.3;

const RW: f32 = 10.0;
const RH: f32 = 5.0;
/// Wire between parts in series.
const GAP: f32 = 3.0;
/// Space between parallel branches.
const VGAP: f32 = 2.0;
/// Wire from a parallel group's rail to its branches.
const RAIL: f32 = 2.5;
const HEIGHT: f32 = 16.0;
const MID: f32 = HEIGHT / 2.0;

enum Part {
    R(&'static str),
    Pot(&'static str),
    Series(Vec<Part>),
    Parallel(Vec<Part>),
}

use Part::*;

impl Part {
    fn size(&self) -> (f32, f32) {
        match self {
            R(_) | Pot(_) => (RW, RH),
            Series(parts) => {
                let sizes = parts.iter().map(Part::size);
                let w = sizes.clone().map(|s| s.0).sum::<f32>() + GAP * (parts.len() - 1) as f32;
                (w, sizes.map(|s| s.1).fold(0.0, f32::max))
            }
            Parallel(parts) => {
                let sizes = parts.iter().map(Part::size);
                let w = sizes.clone().map(|s| s.0).fold(0.0, f32::max) + 2.0 * RAIL;
                (
                    w,
                    sizes.map(|s| s.1).sum::<f32>() + VGAP * (parts.len() - 1) as f32,
                )
            }
        }
    }

    /// Draws the part with its left end at `x, y`; returns its right end.
    fn draw(&self, x: f32, y: f32, out: &mut Vec<Shape>) -> f32 {
        let (w, h) = self.size();
        match self {
            R(label) | Pot(label) => out.push(Shape::Resistor {
                x,
                y: y - RH / 2.0,
                w: RW,
                h: RH,
                label,
                trimmer: matches!(self, Pot(_)),
            }),
            Series(parts) => {
                let mut cx = x;
                for (i, part) in parts.iter().enumerate() {
                    if i > 0 {
                        out.push(line(cx, y, cx + GAP, y));
                        cx += GAP;
                    }
                    cx = part.draw(cx, y, out);
                }
            }
            Parallel(parts) => {
                let right = x + w;
                let mut top = y - h / 2.0;
                let (mut first, mut last) = (y, y);
                for (i, part) in parts.iter().enumerate() {
                    let (pw, ph) = part.size();
                    let py = top + ph / 2.0;
                    if i == 0 {
                        first = py;
                    }
                    last = py;
                    // Branches are centred, with wire making up the width.
                    let px = x + (w - pw) / 2.0;
                    out.push(line(x, py, px, py));
                    part.draw(px, py, out);
                    out.push(line(px + pw, py, right, py));
                    top += ph + VGAP;
                }
                out.push(line(x, first, x, last));
                out.push(line(right, first, right, last));
            }
        }
        x + w
    }
}

fn line(x1: f32, y1: f32, x2: f32, y2: f32) -> Shape {
    Shape::Line {
        from: (x1, y1),
        to: (x2, y2),
    }
}

/// A two-terminal network: ○─ part ─○.
fn two_terminal(part: Part) -> Sketch {
    let mut shapes = Vec::new();
    let x0 = DOT_RADIUS;
    shapes.push(Shape::Terminal { x: x0, y: MID });
    shapes.push(line(x0 + DOT_RADIUS, MID, x0 + GAP, MID));
    let end = part.draw(x0 + GAP, MID, &mut shapes);
    let x1 = end + GAP;
    shapes.push(line(end, MID, x1 - DOT_RADIUS, MID));
    shapes.push(Shape::Terminal { x: x1, y: MID });
    Sketch {
        width: x1 + DOT_RADIUS,
        height: HEIGHT,
        shapes,
    }
}

/// A divider on its side: Vin ○─ top ─●─ bottom ─⏚, with Vout taken up from
/// the middle node.
fn divider(top: Part, bottom: Part) -> Sketch {
    let mut shapes = Vec::new();
    let x0 = DOT_RADIUS;
    shapes.push(Shape::Terminal { x: x0, y: MID });
    shapes.push(line(x0 + DOT_RADIUS, MID, x0 + GAP, MID));
    let end = top.draw(x0 + GAP, MID, &mut shapes);
    let node = end + GAP + 1.0;
    shapes.push(line(end, MID, node + GAP + 1.0, MID));
    shapes.push(Shape::Dot { x: node, y: MID });
    let out_y = DOT_RADIUS;
    shapes.push(line(node, MID, node, out_y + DOT_RADIUS));
    shapes.push(Shape::Terminal { x: node, y: out_y });
    let end = bottom.draw(node + GAP + 1.0, MID, &mut shapes);
    shapes.push(line(end, MID, end + GAP, MID));
    shapes.push(Shape::Ground {
        x: end + GAP,
        y: MID,
    });
    Sketch {
        width: end + GAP + 3.5,
        height: HEIGHT,
        shapes,
    }
}

/// The schematic of a result row's circuit, for kinds that combine parts in
/// more than one way.
pub fn sketch(kind: Kind) -> Option<Sketch> {
    Some(match kind {
        Kind::Plain | Kind::PotDivider => return None,
        Kind::SeriesParallel => {
            two_terminal(Series(vec![R("R1"), Parallel(vec![R("R2"), R("R3")])]))
        }
        Kind::ParallelSeries => {
            two_terminal(Parallel(vec![R("R1"), Series(vec![R("R2"), R("R3")])]))
        }
        Kind::PotSeries => two_terminal(Series(vec![R("Rs"), Pot("P")])),
        Kind::PotSeriesShunt => {
            two_terminal(Series(vec![R("Rs"), Parallel(vec![Pot("P"), R("Rp")])]))
        }
        Kind::PotShunt => two_terminal(Parallel(vec![Series(vec![R("Rs"), Pot("P")]), R("Rp")])),
        Kind::DividerTopSeries => divider(Series(vec![R("R1"), R("R2")]), R("R3")),
        Kind::DividerTopParallel => divider(Parallel(vec![R("R1"), R("R2")]), R("R3")),
        Kind::DividerBottomSeries => divider(R("R1"), Series(vec![R("R2"), R("R3")])),
        Kind::DividerBottomParallel => divider(R("R1"), Parallel(vec![R("R2"), R("R3")])),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sketches_fit_their_box_and_name_every_part() {
        for kind in [
            Kind::SeriesParallel,
            Kind::ParallelSeries,
            Kind::PotSeries,
            Kind::PotSeriesShunt,
            Kind::PotShunt,
            Kind::DividerTopSeries,
            Kind::DividerTopParallel,
            Kind::DividerBottomSeries,
            Kind::DividerBottomParallel,
        ] {
            let s = sketch(kind).unwrap();
            let inside = |x: f32, y: f32| {
                (-0.01..=s.width + 0.01).contains(&x) && (-0.01..=s.height + 0.01).contains(&y)
            };
            let mut labels = Vec::new();
            for shape in &s.shapes {
                match *shape {
                    Shape::Line { from, to } => {
                        assert!(inside(from.0, from.1) && inside(to.0, to.1), "{kind:?}")
                    }
                    Shape::Resistor {
                        x, y, w, h, label, ..
                    } => {
                        assert!(inside(x, y) && inside(x + w, y + h), "{kind:?}");
                        labels.push(label);
                    }
                    Shape::Dot { x, y } | Shape::Terminal { x, y } | Shape::Ground { x, y } => {
                        assert!(inside(x, y), "{kind:?}")
                    }
                }
            }
            labels.sort();
            let mut want = kind.labels(3).to_vec();
            want.sort();
            assert_eq!(labels, want, "{kind:?}");
        }
        assert!(sketch(Kind::Plain).is_none());
    }
}
