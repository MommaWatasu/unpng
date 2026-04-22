//#![no_std]

pub mod core;
pub mod deflate;
pub mod filter;
pub mod zlib;

pub mod prelude {
    pub use crate::core::unpng;
}

extern crate alloc;

use alloc::vec::Vec;
use core::*;
use filter::unfilter;
use zlib::zlib_decompress;

pub fn decode(input: &[u8]) -> Result<(ImageHeader, Vec<u8>), PngError> {
    let mut iter = ChunkIter::new(input)?;

    // read IHDR chunk
    let ihdr_chunk = iter.next().ok_or(PngError::UnexpectedEof)??;
    let header = parse_ihdr(&ihdr_chunk)?;

    // collect IDAT and decompress zlib data
    let idat_data = collect_idat(iter)?;
    let deflated = zlib_decompress(&idat_data).map_err(|_| PngError::ZlibError)?;

    // unfilter the image data
    let bytes_per_pixel = header.color_type.channels() * header.bit_depth as usize / 8;
    let pixels = unfilter(
        &deflated,
        header.width as usize,
        header.height as usize,
        bytes_per_pixel,
    )
    .map_err(|_| PngError::FilterError)?;

    Ok((header, pixels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode() {
        let png_data = include_bytes!("../unpng_test.png");
        let (header, pixels) = decode(png_data).unwrap();
        assert_eq!(header.width, 3);
        assert_eq!(header.height, 3);
        assert_eq!(header.bit_depth, 8);
        assert_eq!(header.color_type, ColorType::Rgb);
        assert_eq!(pixels.len(), 27);
    }
}
