//! Pixel effects: mosaic (block average) and blur (three passes of a separable box blur, close
//! to a Gaussian). Both read and write only the pixels inside their own box.

use tiny_skia::Pixmap;

use super::{to_offset, to_pixel};
use crate::geometry::Rect;

/// Box-blur passes; three approximate a Gaussian.
const BLUR_PASSES: usize = 3;

/// A premultiplied RGBA pixel.
type Pixel = [u8; 4];

/// The part of `canvas` covered by `rect` (in image pixels), as canvas columns and rows.
struct Area {
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    /// Where the effect's own grid starts, in canvas columns and rows (may be negative).
    grid_x: i64,
    grid_y: i64,
}

fn area(canvas: &Pixmap, origin: (f32, f32), rect: &Rect) -> Option<Area> {
    let snapped = rect.round_out().offset(-origin.0, -origin.1);
    #[allow(clippy::cast_precision_loss, reason = "canvas sides are at most 16384")]
    let canvas_rect = Rect::new(0.0, 0.0, canvas.width() as f32, canvas.height() as f32);
    let inside = snapped.intersection(&canvas_rect)?;
    let size = |v: f32| usize::try_from(to_pixel(v)).ok();
    Some(Area {
        left: size(inside.x)?,
        top: size(inside.y)?,
        width: size(inside.width)?,
        height: size(inside.height)?,
        grid_x: i64::from(to_offset(snapped.x)),
        grid_y: i64::from(to_offset(snapped.y)),
    })
}

/// Bytes per canvas row.
fn stride(canvas: &Pixmap) -> usize {
    usize::try_from(canvas.width()).unwrap_or(0) * 4
}

/// Copies the pixels of `area` out of the canvas.
fn read(canvas: &Pixmap, area: &Area) -> Vec<Pixel> {
    let stride = stride(canvas);
    let data = canvas.data();
    (area.top..area.top + area.height)
        .flat_map(|row| {
            let start = row * stride + area.left * 4;
            data[start..start + area.width * 4]
                .chunks_exact(4)
                .map(|p| [p[0], p[1], p[2], p[3]])
        })
        .collect()
}

/// Writes `pixels` back into `area`.
fn write(canvas: &mut Pixmap, area: &Area, pixels: &[Pixel]) {
    let stride = stride(canvas);
    let data = canvas.data_mut();
    for (row, line) in pixels.chunks_exact(area.width).enumerate() {
        let start = (area.top + row) * stride + area.left * 4;
        for (target, pixel) in data[start..start + area.width * 4]
            .chunks_exact_mut(4)
            .zip(line)
        {
            target.copy_from_slice(pixel);
        }
    }
}

/// Replaces each `block`-pixel square of `rect` by its average colour. The grid starts at the
/// box's top-left corner, so the result does not depend on the region being rendered.
pub(super) fn mosaic(canvas: &mut Pixmap, origin: (f32, f32), rect: &Rect, block: u32) {
    let Some(area) = area(canvas, origin, rect) else {
        return;
    };
    let mut pixels = read(canvas, &area);
    let block = i64::from(block.max(1));
    let cell = |position: usize, grid: i64| {
        (i64::try_from(position).unwrap_or(0) - grid).div_euclid(block)
    };
    let columns: Vec<i64> = (0..area.width)
        .map(|x| cell(area.left + x, area.grid_x))
        .collect();
    let rows: Vec<i64> = (0..area.height)
        .map(|y| cell(area.top + y, area.grid_y))
        .collect();
    let mut start_y = 0;
    while start_y < area.height {
        let end_y = (start_y..area.height)
            .find(|y| rows[*y] != rows[start_y])
            .unwrap_or(area.height);
        let mut start_x = 0;
        while start_x < area.width {
            let end_x = (start_x..area.width)
                .find(|x| columns[*x] != columns[start_x])
                .unwrap_or(area.width);
            fill_average(&mut pixels, area.width, (start_x, end_x), (start_y, end_y));
            start_x = end_x;
        }
        start_y = end_y;
    }
    write(canvas, &area, &pixels);
}

fn fill_average(pixels: &mut [Pixel], width: usize, xs: (usize, usize), ys: (usize, usize)) {
    let mut sum = [0u64; 4];
    for y in ys.0..ys.1 {
        for pixel in &pixels[y * width + xs.0..y * width + xs.1] {
            for (total, channel) in sum.iter_mut().zip(pixel) {
                *total += u64::from(*channel);
            }
        }
    }
    let count = u64::try_from((xs.1 - xs.0) * (ys.1 - ys.0))
        .unwrap_or(1)
        .max(1);
    let average = sum.map(|total| u8::try_from((total + count / 2) / count).unwrap_or(u8::MAX));
    for y in ys.0..ys.1 {
        pixels[y * width + xs.0..y * width + xs.1].fill(average);
    }
}

/// Blurs `rect` with radius `radius` pixels; pixels outside the box are never read.
pub(super) fn blur(canvas: &mut Pixmap, origin: (f32, f32), rect: &Rect, radius: u32) {
    let Some(area) = area(canvas, origin, rect) else {
        return;
    };
    let radius = usize::try_from(radius).unwrap_or(usize::MAX);
    let mut pixels = read(canvas, &area);
    let mut line = Vec::new();
    let mut out = Vec::new();
    for _ in 0..BLUR_PASSES {
        for row in pixels.chunks_exact_mut(area.width) {
            box_blur(row, radius, &mut out);
            row.copy_from_slice(&out);
        }
        for x in 0..area.width {
            line.clear();
            line.extend((0..area.height).map(|y| pixels[y * area.width + x]));
            box_blur(&line, radius, &mut out);
            for (y, pixel) in out.iter().enumerate() {
                pixels[y * area.width + x] = *pixel;
            }
        }
    }
    write(canvas, &area, &pixels);
}

/// One box-blur pass over `line` into `out`, repeating the edge pixels beyond the ends.
fn box_blur(line: &[Pixel], radius: usize, out: &mut Vec<Pixel>) {
    out.clear();
    let len = line.len();
    let at = |i: isize| line[usize::try_from(i).unwrap_or(0).min(len - 1)];
    let r = isize::try_from(radius).unwrap_or(isize::MAX);
    let window = u32::try_from(2 * radius + 1).unwrap_or(u32::MAX);
    let mut sum = [0u32; 4];
    for i in -r..=r {
        add(&mut sum, at(i), true);
    }
    for i in 0..isize::try_from(len).unwrap_or(isize::MAX) {
        out.push(sum.map(|total| u8::try_from((total + window / 2) / window).unwrap_or(u8::MAX)));
        add(&mut sum, at(i + r + 1), true);
        add(&mut sum, at(i - r), false);
    }
}

fn add(sum: &mut [u32; 4], pixel: Pixel, plus: bool) {
    for (total, channel) in sum.iter_mut().zip(pixel) {
        if plus {
            *total += u32::from(channel);
        } else {
            *total -= u32::from(channel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(width: u32, height: u32, pixel: impl Fn(u32, u32) -> Pixel) -> Pixmap {
        let mut pixmap = Pixmap::new(width, height).unwrap();
        for y in 0..height {
            for x in 0..width {
                let start = usize::try_from((y * width + x) * 4).unwrap();
                pixmap.data_mut()[start..start + 4].copy_from_slice(&pixel(x, y));
            }
        }
        pixmap
    }

    fn at(pixmap: &Pixmap, x: u32, y: u32) -> Pixel {
        let start = usize::try_from((y * pixmap.width() + x) * 4).unwrap();
        let d = &pixmap.data()[start..start + 4];
        [d[0], d[1], d[2], d[3]]
    }

    #[test]
    fn mosaic_averages_blocks_inside_the_box_only() {
        let mut pixmap = canvas(8, 4, |x, _| {
            if x.is_multiple_of(2) {
                [0, 0, 0, 255]
            } else {
                [200, 100, 50, 255]
            }
        });
        mosaic(&mut pixmap, (0.0, 0.0), &Rect::new(0.0, 0.0, 4.0, 4.0), 2);
        assert_eq!(at(&pixmap, 0, 0), [100, 50, 25, 255]);
        assert_eq!(at(&pixmap, 3, 3), [100, 50, 25, 255]);
        assert_eq!(
            at(&pixmap, 4, 0),
            [0, 0, 0, 255],
            "outside the box is untouched"
        );
        assert_eq!(at(&pixmap, 5, 0), [200, 100, 50, 255]);
    }

    #[test]
    fn mosaic_grid_follows_the_box_not_the_canvas() {
        let mut whole = canvas(6, 1, |x, _| [u8::try_from(x * 10).unwrap(), 0, 0, 255]);
        mosaic(&mut whole, (0.0, 0.0), &Rect::new(1.0, 0.0, 4.0, 1.0), 2);
        let mut shifted = canvas(4, 1, |x, _| {
            [u8::try_from((x + 2) * 10).unwrap(), 0, 0, 255]
        });
        mosaic(&mut shifted, (2.0, 0.0), &Rect::new(1.0, 0.0, 4.0, 1.0), 2);
        assert_eq!(at(&whole, 3, 0), at(&shifted, 1, 0));
        assert_eq!(at(&whole, 1, 0), [15, 0, 0, 255]);
    }

    #[test]
    fn blur_smooths_and_keeps_flat_areas() {
        let mut flat = canvas(6, 6, |_, _| [40, 80, 120, 255]);
        blur(&mut flat, (0.0, 0.0), &Rect::new(0.0, 0.0, 6.0, 6.0), 2);
        assert_eq!(at(&flat, 3, 3), [40, 80, 120, 255]);
        let mut edge = canvas(8, 1, |x, _| {
            if x < 4 {
                [0, 0, 0, 255]
            } else {
                [255, 255, 255, 255]
            }
        });
        blur(&mut edge, (0.0, 0.0), &Rect::new(0.0, 0.0, 8.0, 1.0), 1);
        let middle = at(&edge, 4, 0)[0];
        assert!(middle > 100 && middle < 255, "{middle}");
        assert_eq!(at(&edge, 0, 0), [0, 0, 0, 255]);
    }

    #[test]
    fn effects_outside_the_canvas_do_nothing() {
        let mut pixmap = canvas(2, 2, |_, _| [1, 2, 3, 255]);
        let before = pixmap.clone();
        mosaic(&mut pixmap, (0.0, 0.0), &Rect::new(10.0, 10.0, 4.0, 4.0), 2);
        blur(&mut pixmap, (0.0, 0.0), &Rect::new(10.0, 10.0, 4.0, 4.0), 2);
        assert_eq!(pixmap, before);
        let mut single = vec![];
        box_blur(&[[9, 9, 9, 9]], 5, &mut single);
        assert_eq!(single, [[9, 9, 9, 9]]);
    }
}
