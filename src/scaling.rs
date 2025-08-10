use ndarray::{Array, Array2, Axis, stack};
use ndarray_linalg::LeastSquaresSvd;

/// Generate this table using the gamma_lookup_table() function
/// This saves 20-30ms as opposed to calculating it at runtime
static GAMMA_LOOKUP: [u8; 256] = [
    0, 27, 36, 43, 48, 53, 57, 60, 64, 67, 70, 72, 75, 77, 80, 82, 84, 86, 88, 90, 92, 94, 96, 97,
    99, 101, 102, 104, 105, 107, 108, 110, 111, 112, 114, 115, 116, 118, 119, 120, 122, 123, 124,
    125, 126, 127, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144,
    145, 146, 147, 148, 149, 149, 150, 151, 152, 153, 154, 155, 156, 156, 157, 158, 159, 160, 161,
    161, 162, 163, 164, 164, 165, 166, 167, 168, 168, 169, 170, 171, 171, 172, 173, 173, 174, 175,
    176, 176, 177, 178, 178, 179, 180, 180, 181, 182, 182, 183, 184, 184, 185, 186, 186, 187, 188,
    188, 189, 189, 190, 191, 191, 192, 193, 193, 194, 194, 195, 196, 196, 197, 197, 198, 199, 199,
    200, 200, 201, 201, 202, 203, 203, 204, 204, 205, 205, 206, 207, 207, 208, 208, 209, 209, 210,
    210, 211, 211, 212, 212, 213, 214, 214, 215, 215, 216, 216, 217, 217, 218, 218, 219, 219, 220,
    220, 221, 221, 222, 222, 223, 223, 224, 224, 225, 225, 226, 226, 227, 227, 228, 228, 229, 229,
    229, 230, 230, 231, 231, 232, 232, 233, 233, 234, 234, 235, 235, 235, 236, 236, 237, 237, 238,
    238, 239, 239, 239, 240, 240, 241, 241, 242, 242, 243, 243, 243, 244, 244, 245, 245, 246, 246,
    246, 247, 247, 248, 248, 249, 249, 249, 250, 250, 251, 251, 251, 252, 252, 253, 253, 253, 254,
    254, 255, 255, 255,
];

#[allow(dead_code)]
fn gamma_adjust_table() -> [u8; 256] {
    // If powf ever becomes constified, we can use this as a const function
    // instead of hardcoding the GAMMA_LOOKUP table like we do now.
    let mut table = [0; 256];
    let mut i = 0;
    while i < 256 {
        table[i] = (256.0 * (i as f32 / 255.0).powf(1.0 / 2.5)) as u8;
        i += 1;
    }
    table
}

#[derive(Debug)]
#[allow(dead_code)]
struct ZscaleBounds {
    min: f32,
    max: f32,
}

fn calc_zscale(sample_data: Vec<f32>) -> ZscaleBounds {
    let contrast = 0.1; // Hardcoded for now

    let nsamples = sample_data.len();
    let zmin = sample_data[0];
    let zmax = sample_data[nsamples - 1];
    let lsq_fit = least_squares_line_fit(sample_data);
    let mut slope = lsq_fit.slope;

    if contrast > 0.0 {
        slope /= contrast;
    }

    let fitted_dy = slope * nsamples as f32 / 2.0;

    ZscaleBounds {
        min: zmin.max(lsq_fit.intercept - fitted_dy),
        max: zmax.min(lsq_fit.intercept + fitted_dy),
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct LeastSquareResult {
    slope: f32,
    intercept: f32,
    num_iterations: usize,
    num_samples: usize,
    rms: f32,
}

fn least_squares_line_fit(sample_data: Vec<f32>) -> LeastSquareResult {
    let num_samples = sample_data.len();
    let x: Vec<f32> = (0..num_samples).map(|i| i as f32).collect();
    let a: Array2<f32> = stack![Axis(1), x, vec![1.0; num_samples]];
    let y = Array::from(sample_data);
    let result = a.least_squares(&y).unwrap();
    let mean_residual = result
        .residual_sum_of_squares
        .unwrap()
        .first()
        .unwrap_or(&0.0)
        / num_samples as f32;
    let rms = mean_residual.sqrt();

    LeastSquareResult {
        slope: result.solution[0],
        intercept: result.solution[1],
        num_iterations: 1,
        num_samples,
        rms,
    }
}

fn linear_scale(mut image_data: Vec<f32>, zmin: f32, zmax: f32) -> Vec<u8> {
    let mut max = zmax;
    let mut min = zmin;
    if zmax == zmin {
        max = zmax + 1.0;
        min = zmin - 1.0;
    }
    let scale = 255.0 / (max - min);
    let adjust = scale * min;
    for pixel in &mut image_data {
        *pixel = pixel.clamp(min, max);
        *pixel *= scale;
        *pixel -= adjust;
        *pixel = pixel.round();
    }
    image_data
        .into_iter()
        .map(|e| GAMMA_LOOKUP[e as usize])
        .collect()
}

/// Return 2000 samples from the image data, sorted
fn extract_samples(pixels: &Vec<f32>) -> Vec<f32> {
    let num_samples = 2000;
    let steps = pixels.len() / num_samples;
    let mut samples: Vec<f32> = pixels.iter().step_by(steps).skip(1).cloned().collect();
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples
}

pub fn scaled_image(pixels: Vec<f32>) -> Vec<u8> {
    let sampled_data = extract_samples(&pixels);
    let median = sampled_data[sampled_data.len() / 2];
    let min_max = calc_zscale(sampled_data);
    linear_scale(pixels, median, min_max.max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;
    use image::ImageBuffer;
    use rand::prelude::*;
    use std::time::Instant;

    #[test]
    fn test_scaled_image() {
        // Generate random distribution of 5760000 pixels
        let mut rng = rand::rng();
        let pixels: Vec<f32> = (0..5760000).map(|_| rng.random::<f32>()).collect();
        let now = Instant::now();
        let scaled = scaled_image(pixels);
        let elapsed = now.elapsed();
        tracing::debug!("Scaling elapsed time: {:?}", elapsed);
        assert_eq!(scaled.len(), 5760000);
        let mut image =
            DynamicImage::ImageLuma8(ImageBuffer::from_vec(2400, 2400, scaled).unwrap());
        image = image.resize(300, 300, image::imageops::FilterType::Triangle);
        image.save("tests/output/random.jpeg").unwrap();
    }

    #[test]
    fn test_gamma_adjust() {
        let table = gamma_adjust_table();
        assert_eq!(table, GAMMA_LOOKUP);
    }
}
