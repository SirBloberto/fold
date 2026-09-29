const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const WINDOW: usize = 32_768;
const LONGEST: usize = 258;
const TRIES: usize = 48;

pub fn encode(width: usize, height: usize, pixels: &[u32]) -> Vec<u8> {
    let mut header = Vec::new();
    header.extend((width as u32).to_be_bytes());
    header.extend((height as u32).to_be_bytes());
    header.extend([8, 2, 0, 0, 0]);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"IDAT", &zlib(&filtered(width, pixels)));
    chunk(&mut png, b"IEND", &[]);
    png
}

fn filtered(width: usize, pixels: &[u32]) -> Vec<u8> {
    let stride = width * 3;
    let bytes: Vec<u8> = pixels
        .iter()
        .flat_map(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8])
        .collect();
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / stride.max(1));
    let blank = vec![0; stride];
    for (row, line) in bytes.chunks(stride).enumerate() {
        let above = if row == 0 {
            &blank[..]
        } else {
            &bytes[(row - 1) * stride..row * stride]
        };
        let options: [Vec<u8>; 3] = [
            line.to_vec(),
            (0..stride)
                .map(|i| line[i].wrapping_sub(if i < 3 { 0 } else { line[i - 3] }))
                .collect(),
            (0..stride)
                .map(|i| line[i].wrapping_sub(above[i]))
                .collect(),
        ];
        let cost = |row: &Vec<u8>| {
            row.iter()
                .map(|&b| (b as i8).unsigned_abs() as u32)
                .sum::<u32>()
        };
        let best = (0..3).min_by_key(|&f| cost(&options[f])).unwrap_or(0);
        out.push(best as u8);
        out.extend(&options[best]);
    }
    out
}

fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend((data.len() as u32).to_be_bytes());
    let start = png.len();
    png.extend(kind);
    png.extend(data);
    let crc = crc32(&png[start..]);
    png.extend(crc.to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in bytes.chunks(5552) {
        for &byte in chunk {
            a += byte as u32;
            b += a;
        }
        a %= 65_521;
        b %= 65_521;
    }
    (b << 16) | a
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut bits = Bits::default();
    bits.push(1, 1);
    bits.push(1, 2);
    let mut head = vec![usize::MAX; 1 << 15];
    let mut previous = vec![usize::MAX; data.len()];
    let key = |i: usize| {
        ((data[i] as usize) << 10 ^ (data[i + 1] as usize) << 5 ^ data[i + 2] as usize) & 0x7fff
    };
    let mut i = 0;
    while i < data.len() {
        let (mut length, mut distance) = (0, 0);
        if i + 3 <= data.len() {
            let mut candidate = head[key(i)];
            let limit = LONGEST.min(data.len() - i);
            for _ in 0..TRIES {
                if candidate == usize::MAX || i - candidate > WINDOW {
                    break;
                }
                let same = (0..limit)
                    .take_while(|&k| data[candidate + k] == data[i + k])
                    .count();
                if same > length {
                    (length, distance) = (same, i - candidate);
                }
                candidate = previous[candidate];
            }
        }
        let step = if length >= 3 {
            bits.pair(length, distance);
            length
        } else {
            bits.literal(data[i] as u16);
            1
        };
        let end = (i + step).min(data.len().saturating_sub(2));
        for (j, link) in previous.iter_mut().enumerate().take(end).skip(i) {
            let k = key(j);
            *link = head[k];
            head[k] = j;
        }
        i += step;
    }
    bits.literal(256);
    let mut out = vec![0x78, 0x01];
    out.extend(bits.finish());
    out.extend(adler32(data).to_be_bytes());
    out
}

#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    held: u64,
    count: u32,
}

impl Bits {
    fn push(&mut self, value: u32, count: u32) {
        self.held |= (value as u64) << self.count;
        self.count += count;
        while self.count >= 8 {
            self.bytes.push(self.held as u8);
            self.held >>= 8;
            self.count -= 8;
        }
    }

    fn code(&mut self, code: u32, count: u32) {
        self.push(code.reverse_bits() >> (32 - count), count);
    }

    fn literal(&mut self, symbol: u16) {
        let symbol = symbol as u32;
        match symbol {
            0..=143 => self.code(0x30 + symbol, 8),
            144..=255 => self.code(0x190 + symbol - 144, 9),
            256..=279 => self.code(symbol - 256, 7),
            _ => self.code(0xc0 + symbol - 280, 8),
        }
    }

    fn pair(&mut self, length: usize, distance: usize) {
        let slot = LENGTH_BASE
            .iter()
            .rposition(|&b| b as usize <= length)
            .unwrap_or(0);
        self.literal(257 + slot as u16);
        self.push(
            (length - LENGTH_BASE[slot] as usize) as u32,
            LENGTH_EXTRA[slot] as u32,
        );
        let slot = DISTANCE_BASE
            .iter()
            .rposition(|&b| b as usize <= distance)
            .unwrap_or(0);
        self.code(slot as u32, 5);
        self.push(
            (distance - DISTANCE_BASE[slot] as usize) as u32,
            DISTANCE_EXTRA[slot] as u32,
        );
    }

    fn finish(mut self) -> Vec<u8> {
        if self.count > 0 {
            self.bytes.push(self.held as u8);
        }
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Reader<'a> {
        bytes: &'a [u8],
        at: usize,
    }

    impl Reader<'_> {
        fn bit(&mut self) -> u32 {
            let bit = (self.bytes[self.at / 8] >> (self.at % 8)) & 1;
            self.at += 1;
            bit as u32
        }

        fn bits(&mut self, count: u8) -> usize {
            (0..count).map(|k| (self.bit() as usize) << k).sum()
        }

        fn code(&mut self, count: u32) -> u32 {
            (0..count).fold(0, |code, _| code << 1 | self.bit())
        }

        fn symbol(&mut self) -> u32 {
            let seven = self.code(7);
            if seven <= 0x17 {
                return seven + 256;
            }
            let eight = seven << 1 | self.bit();
            match eight {
                0x30..=0xbf => eight - 0x30,
                0xc0..=0xc7 => eight - 0xc0 + 280,
                _ => (eight << 1 | self.bit()) - 0x190 + 144,
            }
        }
    }

    fn inflate(stream: &[u8]) -> Vec<u8> {
        let mut reader = Reader {
            bytes: &stream[2..],
            at: 3,
        };
        let mut out: Vec<u8> = Vec::new();
        loop {
            match reader.symbol() {
                256 => return out,
                literal @ 0..=255 => out.push(literal as u8),
                symbol => {
                    let slot = (symbol - 257) as usize;
                    let length = LENGTH_BASE[slot] as usize + reader.bits(LENGTH_EXTRA[slot]);
                    let slot = reader.code(5) as usize;
                    let distance = DISTANCE_BASE[slot] as usize + reader.bits(DISTANCE_EXTRA[slot]);
                    for _ in 0..length {
                        out.push(out[out.len() - distance]);
                    }
                }
            }
        }
    }

    #[test]
    fn checksums_match_their_standards() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
    }

    #[test]
    fn compressed_data_comes_back_unchanged() {
        let mut data: Vec<u8> = (0..20_000u32).map(|i| (i * 7 % 251) as u8).collect();
        data.extend(std::iter::repeat_n(42, 5000));
        data.extend(b"fold fold fold folded space");
        let stream = zlib(&data);
        assert_eq!(inflate(&stream), data);
        assert_eq!(stream[stream.len() - 4..], adler32(&data).to_be_bytes());
        assert!(stream.len() < data.len() / 4);
    }

    #[test]
    fn pixels_survive_encoding() {
        let (width, height) = (37, 21);
        let pixels: Vec<u32> = (0..width * height)
            .map(|i| (i as u32).wrapping_mul(0x9e37_79b9) >> 8)
            .collect();
        let png = encode(width, height, &pixels);
        let data = &png[33 + 8..png.len() - 12 - 4];
        let rows = inflate(data);
        let stride = width * 3;
        let mut previous = vec![0u8; stride];
        for (row, line) in rows.chunks(stride + 1).enumerate() {
            let mut decoded = vec![0u8; stride];
            for i in 0..stride {
                let left = if i < 3 { 0 } else { decoded[i - 3] };
                decoded[i] = match line[0] {
                    0 => line[1 + i],
                    1 => line[1 + i].wrapping_add(left),
                    _ => line[1 + i].wrapping_add(previous[i]),
                };
            }
            for x in 0..width {
                let want = pixels[row * width + x];
                let got =
                    u32::from_be_bytes([0, decoded[x * 3], decoded[x * 3 + 1], decoded[x * 3 + 2]]);
                assert_eq!(got, want, "({x}, {row})");
            }
            previous = decoded;
        }
    }
}
