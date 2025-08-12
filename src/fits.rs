use anyhow::{Error, Result, anyhow};
use fitsio::{FitsFile, HeaderValue};
use memfd::MemfdOptions;
use std::{io::Write, os::fd::AsRawFd};

pub struct FitsImageData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<f32>,
}

pub fn read_fits(image_data: &[u8]) -> Result<FitsImageData, Error> {
    let opts = MemfdOptions::default().allow_sealing(false);
    let mfd = opts.create("image-data")?;
    mfd.as_file().write_all(image_data)?;

    let path = format!("/proc/self/fd/{}", mfd.as_file().as_raw_fd());
    let mut fits_f = FitsFile::open(path).expect("could not open file descriptor");

    let hdus: Vec<_> = fits_f.iter().collect();
    for hdu in hdus.iter() {
        if let Ok(pixels) = hdu.read_image::<Vec<f32>>(&mut fits_f) {
            let height = hdu
                .read_key::<HeaderValue<i64>>(&mut fits_f, "ZNAXIS2")?
                .value as u32;
            let width = hdu
                .read_key::<HeaderValue<i64>>(&mut fits_f, "ZNAXIS1")?
                .value as u32;
            return Ok(FitsImageData {
                width,
                height,
                pixels,
            });
        }
    }

    Err(anyhow!("No image data found"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Read;
    use std::time::Instant;
    use test_log::test;

    #[test]
    fn test_read_compressed_fits() {
        let mut file = File::open("tests/data/test.fits.fz").unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();
        let now = Instant::now();
        let image_data = read_fits(&buffer[..]).unwrap();
        let elapsed = now.elapsed();
        tracing::debug!("read time: {:?}", elapsed);
        assert!(image_data.width * image_data.height == image_data.pixels.len() as u32);
        assert!(image_data.pixels.len() == 5760000);
    }
}
