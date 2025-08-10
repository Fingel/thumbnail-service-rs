use ndarray::{Array, Array2, Axis, stack};
use ndarray_linalg::LeastSquaresSvd;

#[derive(Debug)]
#[allow(dead_code)]
struct ZscaleBounds {
    min: f32,
    max: f32,
}

fn calc_zscale(sample_data: &[f32]) -> ZscaleBounds {
    let contrast = 0.1; // Hardcoded for now

    let nsamples = sample_data.len();
    let lsq_fit = least_squares_line_fit(sample_data);
    let zmin = sample_data[0];
    let zmax = sample_data[nsamples - 1];
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

fn least_squares_line_fit(sample_data: &[f32]) -> LeastSquareResult {
    let num_samples = sample_data.len();
    let x: Vec<f32> = (0..num_samples).map(|i| i as f32).collect();

    let a: Array2<f32> = stack![Axis(1), x, vec![1.0; num_samples]];
    let y = Array::from(sample_data.to_vec());
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

fn gamma_adjust_table() -> Vec<u8> {
    let size = 255 + 1; // Max size minus min size plus 1
    let mut table = vec![0; size];
    (0..size).for_each(|i| {
        table[i] = (size as f32 * (i as f32 / 255.0).powf(1.0 / 2.5)) as u8;
    });
    table
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
    let gamma_lookup = gamma_adjust_table();
    image_data
        .into_iter()
        .map(|e| gamma_lookup[e as usize])
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
    let min_max = calc_zscale(&sampled_data);
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
}
