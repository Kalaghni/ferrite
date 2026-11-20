//! A small PNG writer: RGB, one fixed-Huffman deflate block whose matches
//! only look one pixel back or one row up. Plans are mostly flat colour,
//! so that is enough to keep them small without a compression crate.

struct Bits {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl Bits {
    /// `count` bits of `v`, least significant first.
    fn put(&mut self, v: u32, count: u32) {
        self.acc |= v << self.n;
        self.n += count;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }
    /// A Huffman code, which deflate stores most significant bit first.
    fn code(&mut self, code: u32, len: u32) {
        let mut r = 0;
        for i in 0..len {
            r |= ((code >> i) & 1) << (len - 1 - i);
        }
        self.put(r, len);
    }
    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

const LEN_BASE: [u16; 29] =
    [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385,
    24577,
];
const DIST_EXTRA: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

fn literal(b: &mut Bits, sym: u32) {
    match sym {
        0..=143 => b.code(0x30 + sym, 8),
        144..=255 => b.code(0x190 + sym - 144, 9),
        256..=279 => b.code(sym - 256, 7),
        _ => b.code(0xC0 + sym - 280, 8),
    }
}

fn deflate(data: &[u8], dists: &[usize]) -> Vec<u8> {
    let mut b = Bits { out: Vec::new(), acc: 0, n: 0 };
    b.put(1, 1); // final block
    b.put(1, 2); // fixed Huffman
    let mut i = 0;
    while i < data.len() {
        let mut best = (0usize, 0usize);
        for &d in dists {
            if d == 0 || d > i || d > 32768 {
                continue;
            }
            let mut n = 0;
            while n < 258 && i + n < data.len() && data[i + n] == data[i + n - d] {
                n += 1;
            }
            if n > best.0 {
                best = (n, d);
            }
        }
        if best.0 >= 3 {
            let (len, dist) = best;
            let k = LEN_BASE.iter().rposition(|&l| l as usize <= len).unwrap();
            literal(&mut b, 257 + k as u32);
            b.put((len - LEN_BASE[k] as usize) as u32, LEN_EXTRA[k] as u32);
            let k = DIST_BASE.iter().rposition(|&d| d as usize <= dist).unwrap();
            b.code(k as u32, 5);
            b.put((dist - DIST_BASE[k] as usize) as u32, DIST_EXTRA[k] as u32);
            i += len;
        } else {
            literal(&mut b, data[i] as u32);
            i += 1;
        }
    }
    literal(&mut b, 256);
    b.finish()
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &x in bytes {
        c ^= x as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in bytes.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Encode `rgb` (top row first, three bytes a pixel) as a PNG.
pub fn encode(w: usize, h: usize, rgb: &[u8]) -> Vec<u8> {
    assert_eq!(rgb.len(), w * h * 3);
    let stride = w * 3 + 1;
    let mut raw = Vec::with_capacity(stride * h);
    for row in rgb.chunks(w * 3) {
        raw.push(0); // no filter
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    z.extend(deflate(&raw, &[3, stride]));
    z.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                s.push(T[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(super::base64(b"Man"), "TWFu");
        assert_eq!(super::base64(b"Ma"), "TWE=");
        assert_eq!(super::base64(b"M"), "TQ==");
    }

    #[test]
    fn crc_and_adler_match_known_values() {
        assert_eq!(super::crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(super::adler32(b"Wikipedia"), 0x11E6_0398);
    }
}
