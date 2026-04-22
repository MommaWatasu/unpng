use crate::deflate::{DeflateError, inflate};

use alloc::vec::Vec;

pub enum ZlibError {
    UnexpectedEOF,
    InvalidHeader,
    DeflateError(DeflateError),
}

impl From<DeflateError> for ZlibError {
    fn from(err: DeflateError) -> Self {
        ZlibError::DeflateError(err)
    }
}

pub fn zlib_decompress(data: &[u8]) -> Result<Vec<u8>, ZlibError> {
    if data.len() < 6 {
        return Err(ZlibError::UnexpectedEOF);
    }

    let cmf = data[0];
    let flg = data[1];

    // check compression method(CM = 8 means DEFLATE)
    if cmf & 0x0F != 8 {
        return Err(ZlibError::InvalidHeader);
    }

    // check header checksum
    if ((cmf as u16) << 8 | flg as u16) % 31 != 0 {
        return Err(ZlibError::InvalidHeader);
    }

    // check FDICT flag, which is not supported
    if flg & 0x20 != 0 {
        return Err(ZlibError::InvalidHeader);
    }

    let compressed_data = &data[2..];
    let decompressed = inflate(compressed_data)?;
    Ok(decompressed)
}
