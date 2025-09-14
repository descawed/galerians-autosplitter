use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use opencv::prelude::*;
use opencv::imgcodecs::{IMREAD_GRAYSCALE, imread};
use opencv::videoio::VideoCapture;

use super::{Game, Map};
use crate::image::{CaptureImage, CaptureTransform, CaptureTransformJson, ReferenceImage, gray_float};

const DEVICE_SETTINGS_PATH: &str = "device.json";
const BACKGROUND_PATH: &str = "assets/backgrounds/";
const CALIBRATION_IMAGE_PATH: &str = "assets/backgrounds/A1501_4.png";
const HUD_MASK_PATH: &str = "assets/backgrounds/hud_mask.png";
const BG_MAP_PATH: &str = "assets/backgrounds/bg_map.json";

type BackgroundMap = HashMap<(Map, u16), Vec<(Map, u16, PathBuf)>>;

fn load_device_settings() -> Result<HashMap<i32, CaptureTransform>> {
    let path = Path::new(DEVICE_SETTINGS_PATH);
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let file = File::open(path)?;
    let settings: HashMap<i32, CaptureTransformJson> = serde_json::from_reader(file)?;

    Ok(settings.into_iter().map(|(index, json)| (index, CaptureTransform::from_json(&json))).collect())
}

fn save_device_settings(settings: &HashMap<i32, CaptureTransform>) -> Result<()> {
    let json: HashMap<_, _> = settings.iter().map(|(index, transform)| (*index, transform.for_json())).collect();
    let file = File::create(DEVICE_SETTINGS_PATH)?;
    serde_json::to_writer(file, &json)?;
    Ok(())
}

fn load_gray(path: &str) -> Result<Mat> {
    let mat = imread(path, IMREAD_GRAYSCALE)?;
    if mat.empty() {
        bail!("Failed to load image {path}");
    }
    gray_float(mat)
}

fn calibrate(capture_device: &mut VideoCapture, hud_mask: &Mat) -> Result<CaptureTransform> {
    println!(
        "Before starting a run, we must first calibrate the video capture. "
        "Please start a new game, wait until you gain control of Rion in the first room, and then press enter."
    );
    std::io::stdin().read_line(&mut String::new())?;

    let mut frame = Mat::default();
    capture_device.read(&mut frame)?;

    let capture_image = CaptureImage::new(frame)?;
    let calibration_image = load_gray(CALIBRATION_IMAGE_PATH)?;
    capture_image.find_transform(&calibration_image, &hud_mask)
}

fn load_bg_map() -> Result<BackgroundMap> {
    let file = File::open(BG_MAP_PATH)?;
    let bg_list: Vec<((Map, u16, Map, u16), String)> = serde_json::from_reader(file)?;
    
    let bg_path = Path::new(BACKGROUND_PATH);
    let mut bg_map = HashMap::new();
    for ((source_map, source_room, dest_map, dest_room), filename) in bg_list {
        let links = bg_map.entry((source_map, source_room)).or_insert_with(Vec::new);
        links.push((dest_map, dest_room, bg_path.join(filename)));
    }
    
    Ok(bg_map)
}

#[derive(Debug)]
pub struct ConsoleGame {
    capture_device: VideoCapture,
    transform: CaptureTransform,
    hud_mask: Mat,
    bg_map: BackgroundMap,
    current_map: Map,
    current_room: u16,
    current_links: Vec<(Map, u16, ReferenceImage)>,
}

impl ConsoleGame {
    pub const fn new(capture_device: VideoCapture, transform: CaptureTransform, hud_mask: Mat, bg_map: BackgroundMap) -> Self {
        Self {
            capture_device,
            transform,
            hud_mask,
            bg_map,
            current_map: Map::Hospital15F,
            current_room: 0,
            current_links: Vec::new(),
        }
    }

    pub fn connect(device_index: i32, force_calibrate: bool) -> Result<Self> {
        let mut capture_device = VideoCapture::new_def(device_index)?;
        let hud_mask = load_gray(HUD_MASK_PATH)?;
        let bg_map = load_bg_map()?;

        let mut settings = load_device_settings()?;
        let transform = if force_calibrate || !settings.contains_key(&device_index) {
            let transform = calibrate(&mut capture_device, &hud_mask)?;
            println!("Calibration complete. Transform: {:?}", transform);
            settings.insert(device_index, transform.clone());
            save_device_settings(&settings)?;
            transform
        } else {
            settings.get(&device_index).unwrap().clone()
        };

        Ok(Self::new(capture_device, transform, hud_mask, bg_map))
    }
}