//! An animated GIF from frames the page has drawn. The page rasterises each
//! frame's SVG on a canvas and hands back the pixels; the core chooses one
//! palette for all frames and writes the file. GIF holds 256 colours, so the
//! palette is the 256 most used colours after grouping each channel to five
//! bits; anti-aliased edges take the nearest of those.

const BINS: usize = 1 << 15;

fn bin(r: u8, g: u8, b: u8) -> usize {
    ((r as usize >> 3) << 10) | ((g as usize >> 3) << 5) | (b as usize >> 3)
}

fn dist(a: [u8; 3], b: [u8; 3]) -> i32 {
    let d = |x: u8, y: u8| (x as i32 - y as i32) * (x as i32 - y as i32);
    d(a[0], b[0]) * 3 + d(a[1], b[1]) * 4 + d(a[2], b[2]) * 2
}

/// The palette and each pixel's index into it.
pub fn quantise(rgba: &[u8]) -> (Vec<[u8; 3]>, Vec<u8>) {
    let mut count = vec![0u64; BINS];
    let mut sum = vec![[0u64; 3]; BINS];
    for px in rgba.as_chunks::<4>().0 {
        let i = bin(px[0], px[1], px[2]);
        count[i] += 1;
        for c in 0..3 {
            sum[i][c] += px[c] as u64;
        }
    }
    let mut used: Vec<usize> = (0..BINS).filter(|i| count[*i] > 0).collect();
    used.sort_by(|a, b| count[*b].cmp(&count[*a]).then(a.cmp(b)));
    used.truncate(256);
    let palette: Vec<[u8; 3]> = used
        .iter()
        .map(|i| {
            let n = count[*i];
            [
                (sum[*i][0] / n) as u8,
                (sum[*i][1] / n) as u8,
                (sum[*i][2] / n) as u8,
            ]
        })
        .collect();
    let mut lookup = vec![u16::MAX; BINS];
    for (pi, bi) in used.iter().enumerate() {
        lookup[*bi] = pi as u16;
    }
    let mut out = Vec::with_capacity(rgba.len() / 4);
    for px in rgba.as_chunks::<4>().0 {
        let i = bin(px[0], px[1], px[2]);
        if lookup[i] == u16::MAX {
            let c = [px[0], px[1], px[2]];
            let best = (0..palette.len())
                .min_by_key(|p| dist(palette[*p], c))
                .unwrap_or(0);
            lookup[i] = best as u16;
        }
        out.push(lookup[i] as u8);
    }
    (palette, out)
}

struct Bits {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl Bits {
    fn put(&mut self, code: u16, width: u32) {
        self.acc |= (code as u32) << self.n;
        self.n += width;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }
    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

/// LZW with 8-bit pixels, as GIF writes it.
pub fn lzw(indices: &[u8]) -> Vec<u8> {
    const CLEAR: u16 = 256;
    const END: u16 = 257;
    let mut table = vec![0u16; 4096 * 256];
    let mut bits = Bits {
        out: Vec::new(),
        acc: 0,
        n: 0,
    };
    let mut width = 9;
    let mut next: u16 = 258;
    bits.put(CLEAR, width);
    let Some((&first, rest)) = indices.split_first() else {
        bits.put(END, width);
        return bits.finish();
    };
    let mut prefix = first as u16;
    for &k in rest {
        let slot = prefix as usize * 256 + k as usize;
        if table[slot] != 0 {
            prefix = table[slot];
            continue;
        }
        bits.put(prefix, width);
        if next < 4096 {
            table[slot] = next;
            next += 1;
            if next > (1 << width) && width < 12 {
                width += 1;
            }
        } else {
            bits.put(CLEAR, width);
            table.iter_mut().for_each(|t| *t = 0);
            next = 258;
            width = 9;
        }
        prefix = k as u16;
    }
    bits.put(prefix, width);
    // the decoder adds one entry after this code and may widen first
    if next < 4096 && next + 1 > (1 << width) && width < 12 {
        width += 1;
    }
    bits.put(END, width);
    bits.finish()
}

/// A looping GIF. `rgba` holds every frame's pixels, one after another;
/// `delays` gives each frame's time in hundredths of a second.
pub fn encode(rgba: &[u8], w: u16, h: u16, delays: &[u16]) -> Result<Vec<u8>, String> {
    let frame = w as usize * h as usize * 4;
    if frame == 0 || delays.is_empty() || rgba.len() != frame * delays.len() {
        return Err("The animation's frames did not arrive whole; try the export again.".into());
    }
    let (mut palette, indices) = quantise(rgba);
    palette.resize(256, [0, 0, 0]);
    let mut out = b"GIF89a".to_vec();
    out.extend(w.to_le_bytes());
    out.extend(h.to_le_bytes());
    out.extend([0xF7, 0, 0]);
    for c in &palette {
        out.extend(c);
    }
    out.extend([0x21, 0xFF, 0x0B]);
    out.extend(b"NETSCAPE2.0");
    out.extend([3, 1, 0, 0, 0]);
    let per = w as usize * h as usize;
    for (i, d) in delays.iter().enumerate() {
        out.extend([0x21, 0xF9, 4, 0]);
        out.extend(d.to_le_bytes());
        out.extend([0, 0]);
        out.push(0x2C);
        out.extend([0, 0, 0, 0]);
        out.extend(w.to_le_bytes());
        out.extend(h.to_le_bytes());
        out.push(0);
        out.push(8);
        for block in lzw(&indices[i * per..(i + 1) * per]).chunks(255) {
            out.push(block.len() as u8);
            out.extend(block);
        }
        out.push(0);
    }
    out.push(0x3B);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plain LZW decoder, written from the GIF specification, to check the
    /// encoder against.
    fn unlzw(data: &[u8]) -> Vec<u8> {
        let mut dict: Vec<Vec<u8>> = (0..256).map(|i| vec![i as u8]).collect();
        dict.push(vec![]);
        dict.push(vec![]);
        let (mut pos, mut width) = (0usize, 9u32);
        let read = |pos: &mut usize, width: u32| -> u16 {
            let mut v = 0u32;
            for b in 0..width {
                let byte = data[(*pos + b as usize) / 8];
                v |= (((byte >> ((*pos + b as usize) % 8)) & 1) as u32) << b;
            }
            *pos += width as usize;
            v as u16
        };
        let mut out = Vec::new();
        let mut prev: Option<Vec<u8>> = None;
        loop {
            let code = read(&mut pos, width);
            if code == 256 {
                dict.truncate(258);
                width = 9;
                prev = None;
                continue;
            }
            if code == 257 {
                break;
            }
            let entry = if (code as usize) < dict.len() {
                dict[code as usize].clone()
            } else {
                let p = prev.clone().unwrap();
                let mut e = p.clone();
                e.push(p[0]);
                e
            };
            out.extend(&entry);
            if let Some(p) = prev {
                let mut e = p.clone();
                e.push(entry[0]);
                dict.push(e);
                if dict.len() == (1 << width) && width < 12 {
                    width += 1;
                }
            }
            prev = Some(entry);
        }
        out
    }

    #[test]
    fn lzw_round_trips_through_a_reference_decoder() {
        let mut data = Vec::new();
        for i in 0..20000u32 {
            data.push(((i * 7 + i / 13) % 5 + (i % 997 == 0) as u32 * 200) as u8);
        }
        assert_eq!(unlzw(&lzw(&data)), data);
        let flat = vec![3u8; 9000];
        assert_eq!(unlzw(&lzw(&flat)), flat);
        assert_eq!(unlzw(&lzw(&[])), Vec::<u8>::new());
    }

    #[test]
    fn a_flat_palette_keeps_exact_colours() {
        let px = [59, 35, 64, 255, 255, 246, 224, 255, 59, 35, 64, 255];
        let (pal, idx) = quantise(&px);
        assert_eq!(pal, vec![[59, 35, 64], [255, 246, 224]]);
        assert_eq!(idx, vec![0, 1, 0]);
    }

    #[test]
    fn the_file_has_a_header_frames_and_a_trailer() {
        let px = vec![200u8; 2 * 2 * 4 * 2];
        let g = encode(&px, 2, 2, &[100, 50]).unwrap();
        assert_eq!(&g[..6], b"GIF89a");
        assert_eq!(*g.last().unwrap(), 0x3B);
        assert_eq!(
            g.windows(2).filter(|w| w == &[0x21, 0xF9]).count(),
            2,
            "two frames"
        );
        assert!(encode(&px, 3, 2, &[1, 1]).is_err(), "a short buffer is refused");
    }
}
