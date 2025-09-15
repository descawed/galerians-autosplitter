use anyhow::{Result, bail};
use opencv::prelude::*;
use opencv::core::{CV_32F, CV_8UC1, CV_8UC3, CV_32FC1, Point3_, Rect, Size, ElemMul, sum_elems};
//use opencv::highgui::{destroy_all_windows, imshow, wait_key_def};
use opencv::imgproc::{COLOR_BGR2GRAY, cvt_color, resize_def};
use serde::{Deserialize, Serialize};

const GRAYSCALE_NORM: f64 = 1.0 / 255.0;
const BLACK_MAX: u8 = 10;
pub const MATCH_THRESHOLD: f64 = 0.65;
pub const BACKGROUND_WIDTH: i32 = 320;
pub const BACKGROUND_HEIGHT: i32 = 240;
const SEARCH_X: i32 = 12;
const SEARCH_Y: i32 = 9;
const MIN_SEARCH_WIDTH: i32 = BACKGROUND_WIDTH - SEARCH_X;
const MIN_SEARCH_HEIGHT: i32 = BACKGROUND_HEIGHT - SEARCH_Y;

pub fn gray_float(mat: Mat) -> Result<Mat> {
    let mat = if mat.typ() == CV_8UC1 {
        let mut new_mat = Mat::default();
        mat.convert_to(&mut new_mat, CV_32F, GRAYSCALE_NORM, 0.0)?;
        new_mat
    } else {
        mat
    };

    if mat.typ() != CV_32FC1 {
        bail!("Image must be 8-bit grayscale or 32-bit floating point");
    }

    Ok(mat)
}

const fn is_black(pixel: &Point3_<u8>) -> bool {
    pixel.x < BLACK_MAX && pixel.y < BLACK_MAX && pixel.z < BLACK_MAX
}

pub fn is_fade_out(mat: &Mat, max_brightness: f64) -> Result<bool> {
    if mat.typ() != CV_32FC1 {
        bail!("Image must be 32-bit floating point grayscale");
    }
    let num_pixels = (mat.rows() * mat.cols()) as f64;
    let average_pixel = sum_elems(&mat)?.0[0] / num_pixels;
    Ok(average_pixel < max_brightness)
}

fn sum_elems_square(mat: &Mat) -> Result<f64> {
    Ok(sum_elems(&mat.elem_mul(mat).into_result()?)?.0[0])
}

fn crop(mat: &Mat, x: i32, y: i32, width: i32, height: i32) -> Result<Mat> {
    let roi = Rect::new(x, y, width, height);
    let cropped = mat.roi(roi)?;
    Ok(cropped.clone_pointee())
}

fn scale_to(mat: &Mat, width: i32, height: i32) -> Result<Mat> {
    let mut scaled = Mat::default();
    resize_def(mat, &mut scaled, Size::new(width, height))?;

    Ok(scaled)
}

fn zncc(capture: &Mat, reference: &Mat, mask: &Mat) -> Result<f64> {
    let mask_sum = sum_elems(&mask)?.0[0];

    let capture = capture.elem_mul(mask).into_result()?;
    let capture = (&capture - (sum_elems(&capture)? / mask_sum)).into_result()?;
    let capture = capture.to_mat()?;
    let capture_square_sum = sum_elems_square(&capture)?;

    let reference = reference.elem_mul(mask).into_result()?;
    let reference = (&reference - (sum_elems(&reference)? / mask_sum)).into_result()?;
    let reference = reference.to_mat()?;
    let reference_square_sum = sum_elems_square(&reference)?;

    let denom = (capture_square_sum * reference_square_sum).sqrt();
    if denom == 0.0 {
        return Ok(0.0);
    }

    let num = sum_elems(&capture.elem_mul(&reference).into_result()?)?.0[0];

    Ok(num / denom)
}

/*fn debug_show(mat: &Mat) -> Result<()> {
    imshow("Debug", mat)?;
    wait_key_def()?;
    destroy_all_windows()?;

    Ok(())
}*/

#[derive(Debug, Clone)]
pub struct MaskedImage(Mat);

impl MaskedImage {
    pub const fn unmasked(image: Mat) -> Self {
        Self(image)
    }
}

#[derive(Debug, Clone)]
pub struct MaskImage {
    mask: Mat,
    sum: f64,
}

impl MaskImage {
    pub fn new(mask: Mat) -> Result<Self> {
        let mask = gray_float(mask)?;
        let sum = sum_elems(&mask)?.0[0];

        Ok(Self { mask, sum })
    }

    pub fn mask(&self, image: &Mat) -> Result<MaskedImage> {
        if image.typ() != CV_32FC1 {
            bail!("Image must be 32-bit floating point grayscale");
        }

        let image = image.elem_mul(&self.mask).into_result()?;
        let image = (&image - (sum_elems(&image)? / self.sum)).into_result()?;
        Ok(MaskedImage(image.to_mat()?))
    }
}

#[derive(Debug, Clone)]
pub struct ReferenceImage {
    image: MaskedImage,
    image_square_sum: f64,
}

impl ReferenceImage {
    pub fn new(image: MaskedImage) -> Result<Self> {
        // pre-calculate what we can
        let image_square_sum = sum_elems_square(&image.0)?;

        Ok(Self {
            image,
            image_square_sum,
        })
    }

    pub fn match_score(&self, capture: &MaskedImage) -> Result<f64> {
        let capture_square_sum = sum_elems_square(&capture.0)?;

        let denom = (capture_square_sum * self.image_square_sum).sqrt();
        if denom == 0.0 {
            return Ok(0.0);
        }

        let num = sum_elems(&(&capture.0).elem_mul(&self.image.0).into_result()?)?.0[0];

        Ok(num / denom)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CaptureTransformJson {
    crx: i32,
    cry: i32,
    crw: i32,
    crh: i32,
    brx: i32,
    bry: i32,
    brw: i32,
    brh: i32,
}

#[derive(Debug, Clone)]
pub struct CaptureTransform {
    capture_roi: Rect,
    bg_roi: Rect,
}

impl CaptureTransform {
    pub const fn new(capture_roi: Rect, bg_roi: Rect) -> Self {
        Self { capture_roi, bg_roi }
    }

    pub const fn from_json(json: &CaptureTransformJson) -> Self {
        Self {
            capture_roi: Rect::new(json.crx, json.cry, json.crw, json.crh),
            bg_roi: Rect::new(json.brx, json.bry, json.brw, json.brh),
        }
    }

    pub const fn for_json(&self) -> CaptureTransformJson {
        CaptureTransformJson {
            crx: self.capture_roi.x,
            cry: self.capture_roi.y,
            crw: self.capture_roi.width,
            crh: self.capture_roi.height,
            brx: self.bg_roi.x,
            bry: self.bg_roi.y,
            brw: self.bg_roi.width,
            brh: self.bg_roi.height,
        }
    }

    pub fn transform_bg(&self, mat: &Mat) -> Result<Mat> {
        crop(mat, self.bg_roi.x, self.bg_roi.y, self.bg_roi.width, self.bg_roi.height)
    }

    pub fn transform_capture(&self, mat: &Mat) -> Result<Mat> {
        let mut grayscale = Mat::default();
        cvt_color(mat, &mut grayscale, COLOR_BGR2GRAY, 0)?;
        let grayscale = gray_float(grayscale)?;
        let cropped = crop(&grayscale, self.capture_roi.x, self.capture_roi.y, self.capture_roi.width, self.capture_roi.height)?;
        scale_to(&cropped, self.bg_roi.width, self.bg_roi.height)
    }
}

#[derive(Debug, Clone)]
pub struct CaptureImage(Mat);

impl CaptureImage {
    pub fn new(mat: Mat) -> Result<Self> {
        if mat.typ() != CV_8UC3 {
            bail!("Capture image must be 8-bit true color");
        }

        Ok(Self(mat))
    }

    pub fn find_transform(&self, background: &Mat, mask: &Mat) -> Result<CaptureTransform> {
        // crop any black bars to find the actual game display within the capture
        let mut y_min = self.0.rows() - 1;
        let mut y_max = 0;
        let mut x_min = self.0.cols() - 1;
        let mut x_max = 0;

        for x in 0..self.0.cols() {
            for y in 0..self.0.rows() {
                let pixel = self.0.at_2d(y, x)?;
                if !is_black(pixel) {
                    y_min = y_min.min(y);
                    y_max = y_max.max(y);
                    x_min = x_min.min(x);
                    x_max = x_max.max(x);
                }
            }
        }

        let capture_roi = Rect::new(x_min, y_min, x_max - x_min, y_max - y_min);
        let cropped = self.0.roi(capture_roi)?;
        let mut grayscale = Mat::default();
        cvt_color(&cropped, &mut grayscale, COLOR_BGR2GRAY, 0)?;
        let grayscale = gray_float(grayscale)?;

        // the background may be slightly cut off in the capture, so we'll try different
        // combinations of cropping off up to a few percent around the edges and see which one
        // yields the best match
        let mut best_match = (-1.0, Rect::default());
        for width in MIN_SEARCH_WIDTH..=BACKGROUND_WIDTH {
            for height in MIN_SEARCH_HEIGHT..=BACKGROUND_HEIGHT {
                let capture = scale_to(&grayscale, width, height)?;
                let x_max = BACKGROUND_WIDTH - width;
                let y_max = BACKGROUND_HEIGHT - height;

                for x in 0..=x_max {
                    for y in 0..=y_max {
                        let background = crop(background, x, y, width, height)?;
                        let mask = crop(mask, x, y, width, height)?;

                        let score = zncc(&capture, &background, &mask)?;
                        if score > best_match.0 {
                            best_match = (score, Rect::new(x, y, width, height));
                        }
                    }
                }
            }
        }

        if best_match.0 < MATCH_THRESHOLD {
            bail!("Capture did not match reference image");
        }

        Ok(CaptureTransform::new(capture_roi, best_match.1))
    }

    pub fn transform(&self, transform: &CaptureTransform) -> Result<Mat> {
        transform.transform_capture(&self.0)
    }
}