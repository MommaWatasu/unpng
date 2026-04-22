use alloc::{vec, vec::Vec};

pub enum DeflateError {
    UnexpectedEof,
    InvalidBlockType,
    InvalidStoredBlock,
    InvalidHuffmanTree,
    InvalidSymbol,
    OutputTooLarge,
}

pub struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    bits: u64,
    nbits: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            bits: 0,
            nbits: 0,
        }
    }

    fn fill(&mut self) {
        while self.nbits <= 56 && self.pos < self.data.len() {
            self.bits |= (self.data[self.pos] as u64) << self.nbits;
            self.nbits += 8;
            self.pos += 1;
        }
    }

    pub fn read_bits(&mut self, n: usize) -> Result<u32, DeflateError> {
        self.fill();
        if self.nbits < n {
            return Err(DeflateError::UnexpectedEof);
        }
        let val = (self.bits & ((1u64 << n) - 1)) as u32;
        self.bits >>= n;
        self.nbits -= n;
        Ok(val)
    }

    pub fn read_bit(&mut self) -> Result<bool, DeflateError> {
        Ok(self.read_bits(1)? != 0)
    }

    pub fn align_to_byte(&mut self) {
        let rem = self.nbits % 8;
        self.bits >>= rem;
        self.nbits -= rem;
    }

    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], DeflateError> {
        self.align_to_byte();
        // convert bits which remains in buffer into bytes
        let buffered = self.nbits / 8;

        self.nbits = 0;
        self.bits = 0;
        let start = self.pos - buffered;
        if start + n > self.data.len() {
            return Err(DeflateError::UnexpectedEof);
        }
        self.pos = start + n;
        Ok(&self.data[start..start + n])
    }
}

//---------- Huffman Tree----------
pub struct HuffmanTree {
    table: [u16; 1 << 15],
    max_bits: usize,
}

impl HuffmanTree {
    // build Huffman Tree(RFC 1951)
    pub fn build(lengths: &[u8]) -> Result<Self, DeflateError> {
        const MAX_BITS: usize = 15;
        let mut bl_count = [0u32; MAX_BITS + 1];

        for &len in lengths {
            if len as usize > MAX_BITS {
                return Err(DeflateError::InvalidHuffmanTree);
            }
            if len > 0 {
                bl_count[len as usize] += 1;
            }
        }

        let mut next_code = [0u32; MAX_BITS + 1];
        let mut code = 0u32;
        for bits in 1..=MAX_BITS {
            code = (code + bl_count[bits - 1]) << 1;
            next_code[bits] = code;
        }

        let max_bits = lengths.iter().copied().max().unwrap_or(0) as usize;
        let mut table = [0u16; 1 << 15];

        for (sym, &len) in lengths.iter().enumerate() {
            if len == 0 {
                continue;
            }
            let len = len as usize;
            let code = next_code[len];
            next_code[len] += 1;

            let reserved = reverse_bits(code, len);

            let step = 1 << len;
            let mut idx = reserved as usize;
            while idx < (1 << 15) {
                table[idx] = ((sym as u16) << 4) | (len as u16);
                idx += step;
            }
        }

        Ok(Self { table, max_bits })
    }

    pub fn decode(&self, reader: &mut BitReader) -> Result<u16, DeflateError> {
        reader.fill();

        let peek = (reader.bits & ((1u64 << self.max_bits) - 1)) as usize;
        let entry = self.table[peek];
        let len = (entry & 0xF) as usize;
        let sym = entry >> 4;

        if len == 0 {
            return Err(DeflateError::InvalidSymbol);
        }

        reader.bits >>= len;
        reader.nbits -= len;
        Ok(sym)
    }
}

fn reverse_bits(code: u32, len: usize) -> u32 {
    let mut result = 0u32;
    let mut c = code;
    for _ in 0..len {
        result = (result << 1) | (c & 1);
        c >>= 1;
    }
    result
}

pub fn inflate(input: &[u8]) -> Result<Vec<u8>, DeflateError> {
    let mut reader = BitReader::new(input);
    let mut output: Vec<u8> = Vec::new();

    loop {
        let bfinal = reader.read_bit()?;
        let btype = reader.read_bits(2)?;

        match btype {
            0b00 => decode_stored(&mut reader, &mut output)?,
            0b01 => decode_fixed(&mut reader, &mut output)?,
            0b10 => decode_dynamic(&mut reader, &mut output)?,
            _ => return Err(DeflateError::InvalidBlockType),
        }

        if bfinal {
            break;
        }
    }

    Ok(output)
}

// BTYPE=0b00
// Non Compressed
fn decode_stored(reader: &mut BitReader, output: &mut Vec<u8>) -> Result<(), DeflateError> {
    let len = reader.read_bits(16)? as usize;
    let nlen = reader.read_bits(16)? as usize;
    if len & 0xFFFF != (!nlen) & 0xFFFF {
        return Err(DeflateError::InvalidStoredBlock);
    }
    let bytes = reader.read_bytes(len)?;
    output.extend_from_slice(bytes);
    Ok(())
}

fn copy_match(output: &mut Vec<u8>, length: usize, distance: usize) -> Result<(), DeflateError> {
    let start = output
        .len()
        .checked_sub(distance)
        .ok_or(DeflateError::InvalidSymbol)?;
    for i in 0..length {
        let byte = output[start + i];
        output.push(byte);
    }
    Ok(())
}

fn decode_symbols(
    reader: &mut BitReader,
    output: &mut Vec<u8>,
    lit_tree: &HuffmanTree,
    dist_tree: &HuffmanTree,
) -> Result<(), DeflateError> {
    const LENGTH_EXTRA: [u8; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const LENGTH_BASE: [u16; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const DIST_EXTRA: [u8; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];
    const DIST_BASE: [u32; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];

    loop {
        let sym = lit_tree.decode(reader)?;
        match sym {
            0..=255 => output.push(sym as u8),
            256 => break,
            257..=285 => {
                let idx = (sym - 257) as usize;
                let extra_len = reader.read_bits(LENGTH_EXTRA[idx] as usize)? as usize;
                let length = LENGTH_BASE[idx] as usize + extra_len;

                let dist_sym = dist_tree.decode(reader)? as usize;
                let extra_dist = reader.read_bits(DIST_EXTRA[dist_sym] as usize)? as u32;
                let distance = (DIST_BASE[dist_sym] + extra_dist) as usize;

                copy_match(output, length, distance)?;
            }
            _ => return Err(DeflateError::InvalidSymbol),
        }
    }
    Ok(())
}

// BTYPE=0b01
// Fixed Huffman
fn decode_fixed(reader: &mut BitReader, output: &mut Vec<u8>) -> Result<(), DeflateError> {
    let lit_tree = fixed_literal_tree()?;
    let dist_tree = fixed_distance_tree()?;
    decode_symbols(reader, output, &lit_tree, &dist_tree)
}

// Huffman Tree defined by RFC 1951
fn fixed_literal_tree() -> Result<HuffmanTree, DeflateError> {
    let mut length = [0u8; 288];
    for i in 0..=143 {
        length[i] = 8;
    }
    for i in 144..=255 {
        length[i] = 9;
    }
    for i in 256..=279 {
        length[i] = 7;
    }
    for i in 280..=287 {
        length[i] = 8;
    }
    HuffmanTree::build(&length)
}

fn fixed_distance_tree() -> Result<HuffmanTree, DeflateError> {
    let length = [5u8; 32];
    HuffmanTree::build(&length)
}

// BTYPE=0b10
// Dynamic Huffman
fn decode_dynamic(reader: &mut BitReader, output: &mut Vec<u8>) -> Result<(), DeflateError> {
    let hlit = reader.read_bits(5)? as usize + 257;
    let hdist = reader.read_bits(5)? as usize + 1;
    let hclen = reader.read_bits(4)? as usize + 4;
    const CLEN_ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];

    // read code length for code length
    let mut clen_lengths = [0u8; 19];
    for i in 0..hclen {
        clen_lengths[CLEN_ORDER[i]] = reader.read_bits(3)? as u8;
    }

    // build Huffman Tree for code length
    let clen_tree = HuffmanTree::build(&clen_lengths)?;

    // decode code length for literal/length and distance
    let mut lit_dist_lengths = vec![0u8; hlit + hdist];
    let mut i = 0;
    while i < hlit + hdist {
        let sym = clen_tree.decode(reader)? as usize;
        match sym {
            0..=15 => {
                // Literal code length 0-15
                lit_dist_lengths[i] = sym as u8;
                i += 1;
            }
            16 => {
                // Repeat previous code length 3-6 times
                let prev = *lit_dist_lengths
                    .last()
                    .ok_or(DeflateError::InvalidHuffmanTree)?;
                let repeat = reader.read_bits(2)? as usize + 3;
                for _ in 0..repeat {
                    lit_dist_lengths[i] = prev;
                    i += 1;
                }
            }
            17 => {
                // Repeat 0 3-6 times
                let count = reader.read_bits(3)? as usize + 3;
                if i + count > hlit + hdist {
                    return Err(DeflateError::InvalidHuffmanTree);
                }
                i += count;
            }
            18 => {
                // Repeat 0 11-138 times
                let count = reader.read_bits(7)? as usize + 11;
                if i + count > hlit + hdist {
                    return Err(DeflateError::InvalidHuffmanTree);
                }
                i += count;
            }
            _ => return Err(DeflateError::InvalidHuffmanTree),
        }
    }

    let lit_tree = HuffmanTree::build(&lit_dist_lengths[..hlit])?;
    let dist_tree = HuffmanTree::build(&lit_dist_lengths[hlit..])?;

    decode_symbols(reader, output, &lit_tree, &dist_tree)
}
