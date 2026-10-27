//! A small baseline JPEG encoder (ITU-T T.81): JFIF, YCbCr without chroma subsampling (sharp
//! coloured annotations over text), 8×8 DCT and the standard's example tables.
//!
//! Written here because the common Rust JPEG encoders carry the IJG license, which is not on the
//! project's license list; this one uses only the published standard.

use std::f32::consts::PI;

use crate::jpeg_tables::{
    AC_CHROMA_BITS, AC_CHROMA_VALUES, AC_LUMA_BITS, AC_LUMA_VALUES, CHROMA_QUANT, DC_CHROMA_BITS,
    DC_LUMA_BITS, DC_VALUES, LUMA_QUANT, ZIGZAG,
};

/// Side of a DCT block.
const BLOCK: usize = 8;
/// Coefficients per block.
const COEFFICIENTS: usize = BLOCK * BLOCK;
/// Symbol for sixteen zero coefficients.
const ZERO_RUN: usize = 0xF0;
/// Symbol for "the rest of the block is zero".
const END_OF_BLOCK: usize = 0x00;

/// A Huffman table: code and length per symbol.
struct Huffman {
    codes: [(u32, u32); 256],
}

impl Huffman {
    /// Canonical codes from the code counts per length and the symbols (T.81 Annex C).
    fn new(bits: &[u8; 16], values: &[u8]) -> Self {
        let mut codes = [(0, 0); 256];
        let mut code = 0u32;
        let mut symbols = values.iter();
        for (length, count) in (1u32..).zip(bits) {
            for _ in 0..*count {
                if let Some(symbol) = symbols.next() {
                    codes[usize::from(*symbol)] = (code, length);
                }
                code += 1;
            }
            code <<= 1;
        }
        Self { codes }
    }

    fn write(&self, out: &mut BitWriter, symbol: usize) {
        let (code, length) = self.codes[symbol & 0xFF];
        out.write(code, length);
    }
}

/// Entropy-coded output with 0xFF byte stuffing.
struct BitWriter {
    out: Vec<u8>,
    bits: u32,
    count: u32,
}

impl BitWriter {
    fn write(&mut self, value: u32, length: u32) {
        if length == 0 {
            return;
        }
        self.bits = (self.bits << length) | (value & ((1 << length) - 1));
        self.count += length;
        while self.count >= 8 {
            let byte = u8::try_from((self.bits >> (self.count - 8)) & 0xFF).unwrap_or(0);
            self.out.push(byte);
            if byte == 0xFF {
                self.out.push(0);
            }
            self.count -= 8;
        }
        self.bits &= (1 << self.count) - 1;
    }

    /// Pads the last byte with one bits.
    fn finish(mut self) -> Vec<u8> {
        let pad = (8 - self.count % 8) % 8;
        self.write((1 << pad) - 1, pad);
        self.out
    }
}

/// The two tables used for one colour component.
struct Component<'a> {
    quant: &'a [f32; COEFFICIENTS],
    dc: &'a Huffman,
    ac: &'a Huffman,
}

/// Encodes `rgb` (`width * height * 3` bytes) as a baseline JPEG at `quality` (1–100).
pub(crate) fn encode(rgb: &[u8], width: u16, height: u16, quality: u8) -> Vec<u8> {
    let luma_quant = scaled(&LUMA_QUANT, quality);
    let chroma_quant = scaled(&CHROMA_QUANT, quality);
    let mut out = Vec::new();
    write_headers(&mut out, width, height, &luma_quant, &chroma_quant);
    let dc_luma = Huffman::new(&DC_LUMA_BITS, &DC_VALUES);
    let dc_chroma = Huffman::new(&DC_CHROMA_BITS, &DC_VALUES);
    let ac_luma = Huffman::new(&AC_LUMA_BITS, &AC_LUMA_VALUES);
    let ac_chroma = Huffman::new(&AC_CHROMA_BITS, &AC_CHROMA_VALUES);
    let divisors = |table: &[u16; COEFFICIENTS]| table.map(f32::from);
    let (luma_div, chroma_div) = (divisors(&luma_quant), divisors(&chroma_quant));
    let components = [
        Component {
            quant: &luma_div,
            dc: &dc_luma,
            ac: &ac_luma,
        },
        Component {
            quant: &chroma_div,
            dc: &dc_chroma,
            ac: &ac_chroma,
        },
        Component {
            quant: &chroma_div,
            dc: &dc_chroma,
            ac: &ac_chroma,
        },
    ];
    let cosines = cosine_table();
    let mut writer = BitWriter {
        out,
        bits: 0,
        count: 0,
    };
    let mut previous_dc = [0i32; 3];
    let (width, height) = (usize::from(width), usize::from(height));
    for block_y in (0..height).step_by(BLOCK) {
        for block_x in (0..width).step_by(BLOCK) {
            let blocks = read_blocks(rgb, width, height, block_x, block_y);
            for (index, samples) in blocks.iter().enumerate() {
                let component = &components[index];
                let coefficients = quantize(&dct(samples, &cosines), component.quant);
                encode_block(
                    &mut writer,
                    &coefficients,
                    &mut previous_dc[index],
                    component,
                );
            }
        }
    }
    let mut out = writer.finish();
    out.extend_from_slice(&[0xFF, 0xD9]);
    out
}

/// A quantization table scaled for `quality` (the usual 50-is-the-example convention).
fn scaled(table: &[u16; COEFFICIENTS], quality: u8) -> [u16; COEFFICIENTS] {
    let quality = u32::from(quality.clamp(1, 100));
    let scale = if quality < 50 {
        5000 / quality
    } else {
        200 - 2 * quality
    };
    table.map(|base| {
        let value = (u32::from(base) * scale + 50) / 100;
        u16::try_from(value.clamp(1, 255)).unwrap_or(255)
    })
}

fn segment(out: &mut Vec<u8>, marker: u8, body: &[u8]) {
    let length = u16::try_from(body.len() + 2).unwrap_or(u16::MAX);
    out.extend_from_slice(&[0xFF, marker]);
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(body);
}

fn write_headers(out: &mut Vec<u8>, width: u16, height: u16, luma: &[u16; 64], chroma: &[u16; 64]) {
    out.extend_from_slice(&[0xFF, 0xD8]);
    segment(out, 0xE0, b"JFIF\0\x01\x01\x00\x00\x01\x00\x01\x00\x00");
    let mut tables = Vec::with_capacity(130);
    for (id, table) in [(0u8, luma), (1, chroma)] {
        tables.push(id);
        tables.extend(
            ZIGZAG
                .iter()
                .map(|n| u8::try_from(table[*n]).unwrap_or(u8::MAX)),
        );
    }
    segment(out, 0xDB, &tables);
    let [h1, h0] = height.to_be_bytes();
    let [w1, w0] = width.to_be_bytes();
    segment(
        out,
        0xC0,
        &[8, h1, h0, w1, w0, 3, 1, 0x11, 0, 2, 0x11, 1, 3, 0x11, 1],
    );
    let mut huffman = Vec::new();
    for (class_id, bits, values) in [
        (0x00, &DC_LUMA_BITS, &DC_VALUES[..]),
        (0x10, &AC_LUMA_BITS, &AC_LUMA_VALUES[..]),
        (0x01, &DC_CHROMA_BITS, &DC_VALUES[..]),
        (0x11, &AC_CHROMA_BITS, &AC_CHROMA_VALUES[..]),
    ] {
        huffman.push(class_id);
        huffman.extend_from_slice(bits);
        huffman.extend_from_slice(values);
    }
    segment(out, 0xC4, &huffman);
    segment(out, 0xDA, &[3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);
}

/// The Y, Cb and Cr samples (level-shifted) of the block at (`x0`, `y0`); edges repeat.
fn read_blocks(rgb: &[u8], width: usize, height: usize, x0: usize, y0: usize) -> [[f32; 64]; 3] {
    let mut blocks = [[0.0; COEFFICIENTS]; 3];
    for dy in 0..BLOCK {
        let y = (y0 + dy).min(height - 1);
        for dx in 0..BLOCK {
            let x = (x0 + dx).min(width - 1);
            let at = (y * width + x) * 3;
            let [red, green, blue] = [rgb[at], rgb[at + 1], rgb[at + 2]].map(f32::from);
            let index = dy * BLOCK + dx;
            blocks[0][index] = 0.299 * red + 0.587 * green + 0.114 * blue - 128.0;
            blocks[1][index] = -0.168_736 * red - 0.331_264 * green + 0.5 * blue;
            blocks[2][index] = 0.5 * red - 0.418_688 * green - 0.081_312 * blue;
        }
    }
    blocks
}

/// `c[u][x] = C(u) / 2 * cos((2x + 1) u π / 16)`, with `C(0) = 1/√2`.
fn cosine_table() -> [[f32; BLOCK]; BLOCK] {
    let mut table = [[0.0; BLOCK]; BLOCK];
    for (u, row) in (0u8..).zip(table.iter_mut()) {
        let c = if u == 0 {
            std::f32::consts::FRAC_1_SQRT_2
        } else {
            1.0
        };
        for (x, value) in (0u8..).zip(row.iter_mut()) {
            *value = c / 2.0 * ((2.0 * f32::from(x) + 1.0) * f32::from(u) * PI / 16.0).cos();
        }
    }
    table
}

/// The two-dimensional DCT of a block, row-major (`[v * 8 + u]`).
fn dct(samples: &[f32; 64], cosines: &[[f32; BLOCK]; BLOCK]) -> [f32; 64] {
    let mut rows = [0.0; COEFFICIENTS];
    for y in 0..BLOCK {
        for u in 0..BLOCK {
            rows[y * BLOCK + u] = (0..BLOCK)
                .map(|x| cosines[u][x] * samples[y * BLOCK + x])
                .sum();
        }
    }
    let mut out = [0.0; COEFFICIENTS];
    for v in 0..BLOCK {
        for u in 0..BLOCK {
            out[v * BLOCK + u] = (0..BLOCK)
                .map(|y| cosines[v][y] * rows[y * BLOCK + u])
                .sum();
        }
    }
    out
}

/// Quantized coefficients in zigzag order.
fn quantize(coefficients: &[f32; 64], divisors: &[f32; 64]) -> [i32; 64] {
    ZIGZAG.map(|n| {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "DCT outputs are within ±2048"
        )]
        let value = (coefficients[n] / divisors[n]).round() as i32;
        value
    })
}

/// Bits needed for `value`, and its JPEG bit pattern (negative values are one less).
fn magnitude(value: i32) -> (u32, u32) {
    let size = 32 - value.unsigned_abs().leading_zeros();
    let bits = if value < 0 { value - 1 } else { value };
    (bits.cast_unsigned() & ((1 << size) - 1), size)
}

fn encode_block(
    out: &mut BitWriter,
    block: &[i32; 64],
    previous_dc: &mut i32,
    tables: &Component<'_>,
) {
    let (bits, size) = magnitude(block[0] - *previous_dc);
    *previous_dc = block[0];
    tables.dc.write(out, symbol(0, size));
    out.write(bits, size);
    let mut run = 0;
    for value in &block[1..] {
        if *value == 0 {
            run += 1;
            continue;
        }
        while run > 15 {
            tables.ac.write(out, ZERO_RUN);
            run -= 16;
        }
        let (bits, size) = magnitude(*value);
        tables.ac.write(out, symbol(run, size));
        out.write(bits, size);
        run = 0;
    }
    if run > 0 {
        tables.ac.write(out, END_OF_BLOCK);
    }
}

/// The Huffman symbol for a zero run and a magnitude size.
fn symbol(run: usize, size: u32) -> usize {
    (run << 4) | usize::try_from(size).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnitudes_follow_the_standard() {
        assert_eq!(magnitude(0), (0, 0));
        assert_eq!(magnitude(1), (1, 1));
        assert_eq!(magnitude(-1), (0, 1));
        assert_eq!(magnitude(5), (5, 3));
        assert_eq!(magnitude(-5), (2, 3));
        assert_eq!(magnitude(-1024), (1023, 11));
    }

    #[test]
    fn quality_scales_the_tables() {
        assert_eq!(scaled(&LUMA_QUANT, 50), LUMA_QUANT);
        assert!(scaled(&LUMA_QUANT, 100).iter().all(|q| *q == 1));
        assert!(scaled(&LUMA_QUANT, 1).iter().all(|q| *q == 255));
    }

    #[test]
    fn huffman_codes_are_canonical() {
        let table = Huffman::new(&DC_LUMA_BITS, &DC_VALUES);
        assert_eq!(table.codes[0], (0b00, 2));
        assert_eq!(table.codes[1], (0b010, 3));
        assert_eq!(table.codes[11], (0b1_1111_1110, 9));
    }

    #[test]
    fn bytes_equal_to_ff_are_stuffed() {
        let mut writer = BitWriter {
            out: Vec::new(),
            bits: 0,
            count: 0,
        };
        writer.write(0xFF, 8);
        writer.write(0b1, 1);
        assert_eq!(writer.finish(), [0xFF, 0x00, 0xFF, 0x00]);
    }

    #[test]
    fn a_flat_block_has_only_a_dc_coefficient() {
        let flat = [10.0; 64];
        let coefficients = dct(&flat, &cosine_table());
        assert!((coefficients[0] - 80.0).abs() < 1e-3);
        assert!(coefficients[1..].iter().all(|c| c.abs() < 1e-3));
    }
}
