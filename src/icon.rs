//! The application icon: a four-band 4k7 resistor, drawn procedurally so no
//! image files or decoders are needed.  Shared with `build.rs`, which turns it
//! into the Windows `.ico` resource.

/// Render the icon as straight RGBA pixels, `size` × `size`.
pub fn render(size: usize) -> Vec<u8> {
    const SS: usize = 4; // supersampling per axis, for smooth edges
    let mut rgba = vec![0u8; size * size * 4];
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (px as f32 + (sx as f32 + 0.5) / SS as f32) / size as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / SS as f32) / size as f32;
                    if let Some(c) = sample(x, y) {
                        for i in 0..3 {
                            acc[i] += c[i] as f32;
                        }
                        acc[3] += 1.0;
                    }
                }
            }
            let n = (SS * SS) as f32;
            let o = (py * size + px) * 4;
            if acc[3] > 0.0 {
                for i in 0..3 {
                    rgba[o + i] = (acc[i] / acc[3]).round() as u8;
                }
                rgba[o + 3] = (acc[3] / n * 255.0).round() as u8;
            }
        }
    }
    rgba
}

/// Colour at normalised coordinates (0..1), or `None` for transparent.
fn sample(x: f32, y: f32) -> Option<[u8; 3]> {
    const BODY: [u8; 3] = [222, 196, 150];
    const LEAD: [u8; 3] = [165, 165, 172];
    const BANDS: [(f32, [u8; 3]); 4] = [
        (0.26, [245, 220, 30]), // yellow
        (0.37, [140, 60, 200]), // violet
        (0.48, [210, 30, 30]),  // red
        (0.66, [212, 175, 55]), // gold
    ];
    let (left, right, top, bottom, r) = (0.12, 0.88, 0.27, 0.73, 0.13);
    // Rounded rectangle body.
    let cx = x.clamp(left + r, right - r);
    let cy = y.clamp(top + r, bottom - r);
    let in_body = (x - cx).powi(2) + (y - cy).powi(2) <= r * r;
    if in_body {
        let band = BANDS.iter().find(|(bx, _)| x >= *bx && x < *bx + 0.075);
        return Some(band.map_or(BODY, |b| b.1));
    }
    if (0.455..0.545).contains(&y) {
        return Some(LEAD);
    }
    None
}
