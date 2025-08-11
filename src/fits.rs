use anyhow::{Error, Result};
use fitsio::FitsFile;
use memfd::MemfdOptions;
use ndarray::ArrayD;
use std::{io::Write, os::fd::AsRawFd};

pub struct FitsImageData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<f32>,
}

pub fn read_fits_fitsio(image_data: &[u8]) -> Result<FitsImageData, Error> {
    let opts = MemfdOptions::default().allow_sealing(false);
    let mfd = opts.create("image-data")?;
    mfd.as_file().write_all(image_data)?;

    let path = format!("/proc/self/fd/{}", mfd.as_file().as_raw_fd());
    let mut fits_f = FitsFile::open(path).expect("could not open file descriptor");

    let fits_data: ArrayD<f32> = {
        let hdus: Vec<_> = fits_f.iter().collect();
        hdus.iter()
            .find_map(|hdu| hdu.read_image(&mut fits_f).ok())
            .expect("Could not read image data from any HDU")
    };
    let dim = fits_data.dim();
    let height = dim[0] as u32;
    let width = dim[1] as u32;
    let pixels = fits_data.into_raw_vec_and_offset().0;
    Ok(FitsImageData {
        width,
        height,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Read;
    use std::time::Instant;
    use test_log::test;

    #[test]
    fn test_read_compressed_fits_cfitsio() {
        let mut file = File::open("tests/data/test.fits.fz").unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();
        let now = Instant::now();
        let image_data = read_fits_fitsio(&buffer[..]).unwrap();
        let elapsed = now.elapsed();
        tracing::debug!("CFITSIO read time: {:?}", elapsed);
        assert!(image_data.width * image_data.height == image_data.pixels.len() as u32);
        assert!(image_data.pixels.len() == 5760000);
    }
}
