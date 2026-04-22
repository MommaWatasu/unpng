use alloc::{vec, vec::Vec};

pub enum FilterError {
    UnexpectedEOF,
    UnkownFilter(u8),
}

// Peath predictor (defined in RFC 2083)
fn peath_predictor(a: u8, b: u8, c: u8) -> u8 {
    let a = a as i32;
    let b = b as i32;
    let c = c as i32;
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();

    if pa <= pb && pa <= pc {
        a as u8
    } else if pb <= pc {
        b as u8
    } else {
        c as u8
    }
}

pub fn unfilter(
    data: &[u8],
    width: usize,
    height: usize,
    bytes_per_pixel: usize,
) -> Result<Vec<u8>, FilterError> {
    // the number of bytes of pixel data in a row
    let stride = width * bytes_per_pixel;
    // the size of a row, including filter type byte
    let row_size = stride + 1;

    if data.len() < row_size * height {
        return Err(FilterError::UnexpectedEOF);
    }

    let mut output = vec![0u8; stride * height];
    // the previous row, initialized to 0 for the first row
    let mut prev_row = vec![0u8; stride];

    for y in 0..height {
        let row_start = y * row_size;
        let filter_type = data[row_start];
        let src = &data[row_start + 1..row_start + 1 + stride];
        let dst = &mut output[y * stride..(y + 1) * stride];

        match filter_type {
            0 => dst.copy_from_slice(src), // None
            1 => {
                // Sub
                for i in 0..stride {
                    let left = if i >= bytes_per_pixel {
                        dst[i - bytes_per_pixel]
                    } else {
                        0
                    };
                    dst[i] = src[i].wrapping_add(left);
                }
            }
            2 => {
                // Up
                for i in 0..stride {
                    dst[i] = src[i].wrapping_add(prev_row[i]);
                }
            }
            3 => {
                // Average
                for i in 0..stride {
                    let left = if i >= bytes_per_pixel {
                        dst[i - bytes_per_pixel]
                    } else {
                        0
                    };
                    let up = prev_row[i];
                    let avg = ((left as u16 + up as u16) / 2) as u8;
                    dst[i] = src[i].wrapping_add(avg);
                }
            }
            4 => {
                // Paeth
                for i in 0..stride {
                    let left = if i >= bytes_per_pixel {
                        dst[i - bytes_per_pixel]
                    } else {
                        0
                    };
                    let up = prev_row[i];
                    let up_left = if i >= bytes_per_pixel {
                        prev_row[i - bytes_per_pixel]
                    } else {
                        0
                    };
                    let paeth = peath_predictor(left, up, up_left);
                    dst[i] = src[i].wrapping_add(paeth);
                }
            }
            n => return Err(FilterError::UnkownFilter(n)),
        }
        // Update the previous row with the current row
        prev_row.copy_from_slice(dst);
    }

    Ok(output)
}
