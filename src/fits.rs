use anyhow::{Context, Error, Result};
use fitsrs::{DataValue, Fits, HDU};
use std::io::Cursor;

pub struct FitsImageData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<f32>,
}

pub fn read_fits(reader: Cursor<Vec<u8>>) -> Result<FitsImageData, Error> {
    let mut hdu_list = Fits::from_reader(reader);

    while let Some(Ok(hdu)) = hdu_list.next() {
        match hdu {
            HDU::XBinaryTable(hdu) => {
                let width = hdu
                    .get_header()
                    .get_parsed::<i64>("ZNAXIS1")
                    .context("ZNAXIS1 header not found")?
                    .context("Failed to parse ZNAXIS1 as u32")? as u32;
                let height = hdu
                    .get_header()
                    .get_parsed::<i64>("ZNAXIS2")
                    .context("ZNAXIS2 header not found")?
                    .context("Failed to parse ZNAXIS2 as u32")? as u32;
                let pixels: Vec<f32> = hdu_list
                    .get_data(&hdu)
                    .map(|m| match m {
                        DataValue::Float {
                            value,
                            column: _,
                            idx: _,
                        } => value,
                        _ => {
                            unreachable!("Inconsistent data type")
                        }
                    })
                    .collect();
                return Ok(FitsImageData {
                    width,
                    height,
                    pixels,
                });
            }
            _ => {}
        }
    }
    Err(Error::msg("Could not find image HDU"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Cursor;
    use std::io::Read;
    use test_log::test;

    #[test]
    fn test_read_compressed_fits() {
        let mut file = File::open("tests/data/test.fits.fz").unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();
        let cursor = Cursor::new(buffer);
        let image_data = read_fits(cursor).unwrap();
        assert!(image_data.width * image_data.height == image_data.pixels.len() as u32);
        assert!(image_data.pixels.len() == 5760000);
    }
}
