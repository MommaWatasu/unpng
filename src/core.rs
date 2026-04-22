use alloc::vec::Vec;

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

pub fn unpng(data: Vec<u8>) -> bool {
    if data[0..8] == PNG_SIGNATURE {
        return true;
    } else {
        return false;
    }
}

pub struct Chunk<'a> {
    pub kind: [u8; 4],
    pub data: &'a [u8],
    pub crc: u32,
}

pub struct ChunkIter<'a> {
    data: &'a [u8],
}

impl<'a> ChunkIter<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, PngError> {
        if data.len() < 8 {
            return Err(PngError::UnexpectedEof);
        }
        if data[..8] != PNG_SIGNATURE {
            return Err(PngError::InvalidSignature);
        }
        Ok(Self { data: &data[8..] })
    }
}

impl<'a> Iterator for ChunkIter<'a> {
    type Item = Result<Chunk<'a>, PngError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            return None;
        }
        // at least 12 bytes are needed
        if self.data.len() < 12 {
            return Some(Err(PngError::UnexpectedEof));
        }

        let length = u32::from_be_bytes(self.data[0..4].try_into().unwrap()) as usize;
        let kind = self.data[4..8].try_into().unwrap();

        if self.data.len() < 12 + length {
            return Some(Err(PngError::UnexpectedEof));
        }

        let data = &self.data[8..8 + length];
        let crc = u32::from_be_bytes(self.data[8 + length..12 + length].try_into().unwrap());

        self.data = &self.data[12 + length..];
        Some(Ok(Chunk { kind, data, crc }))
    }
}

pub struct ImageHeader {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub color_type: ColorType,
    pub interlace: bool,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ColorType {
    Grayscale,
    Rgb,
    Indexed,
    GrayscaleAlpha,
    Rgba,
}

impl ColorType {
    pub fn channels(&self) -> usize {
        match self {
            ColorType::Grayscale => 1,
            ColorType::Rgb => 3,
            ColorType::Indexed => 1,
            ColorType::GrayscaleAlpha => 2,
            ColorType::Rgba => 4,
        }
    }
}

#[derive(Debug)]
pub enum PngError {
    InvalidSignature,
    UnexpectedEof,
    InvalidChunk,
    UnknownColorType(u8),
    InvalidIhdr,
    CrcMismatch,
    ZlibError,
    FilterError,
}

pub fn parse_ihdr(chunk: &Chunk) -> Result<ImageHeader, PngError> {
    if chunk.kind != *b"IHDR" || chunk.data.len() != 13 {
        return Err(PngError::InvalidIhdr);
    }

    let d = chunk.data;
    let width = u32::from_be_bytes(d[0..4].try_into().unwrap());
    let height = u32::from_be_bytes(d[4..8].try_into().unwrap());
    let bit_length = d[8];
    let color_type = match d[9] {
        0 => ColorType::Grayscale,
        2 => ColorType::Rgb,
        3 => ColorType::Indexed,
        4 => ColorType::GrayscaleAlpha,
        6 => ColorType::Rgba,
        n => return Err(PngError::UnknownColorType(n)),
    };

    let interlace = match d[12] {
        0 => false,
        1 => true,
        _ => return Err(PngError::InvalidIhdr),
    };

    Ok(ImageHeader {
        width,
        height,
        bit_depth: bit_length,
        color_type,
        interlace,
    })
}

pub fn collect_idat<'a>(
    chunks: impl Iterator<Item = Result<Chunk<'a>, PngError>>,
) -> Result<Vec<u8>, PngError> {
    let mut buf = Vec::new();
    for chunk in chunks {
        let chunk = chunk?;
        if chunk.kind == *b"IDAT" {
            buf.extend_from_slice(chunk.data);
        }
    }
    Ok(buf)
}
