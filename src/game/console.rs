use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use opencv::core::CV_32F;
use opencv::prelude::*;
use opencv::imgcodecs::{IMREAD_GRAYSCALE, imread};
use opencv::videoio::VideoCapture;

use super::{Game, GameState, Item, Map, Stage};
use crate::image::{
    BACKGROUND_WIDTH, BACKGROUND_HEIGHT,
    CaptureImage, CaptureTransform, CaptureTransformJson, MaskImage, MaskedImage, ReferenceImage,
    gray_float,
};
use crate::platform::PlatformRef;

const DEVICE_SETTINGS_PATH: &str = "device.json";
const BACKGROUND_PATH: &str = "assets/backgrounds/";
const CALIBRATION_IMAGE_PATH: &str = "assets/backgrounds/A1501_4.png";
const HUD_MASK_PATH: &str = "assets/backgrounds/hud_mask.png";
const BG_MAP_PATH: &str = "assets/backgrounds/bg_map.json";
const FINAL_BOSS_ROOM: (Map, u16) = (Map::MushroomTower, 7);

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

fn load_gray(path: impl AsRef<str>) -> Result<Mat> {
    let path = path.as_ref();
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

fn all_black(transform: &CaptureTransform, mask: &MaskImage) -> Result<MaskedImage> {
    let black_screen = Mat::zeros(BACKGROUND_HEIGHT, BACKGROUND_WIDTH, CV_32F)?.to_mat()?;
    let black_screen = transform.transform_bg(&black_screen)?;
    mask.mask(&black_screen)
}

#[derive(Debug)]
pub struct ConsoleGame {
    capture_device: VideoCapture,
    transform: CaptureTransform,
    hud_mask: MaskImage,
    bg_map: BackgroundMap,
    black_screen: ReferenceImage,
    current_map: Map,
    current_room: u16,
    current_links: Vec<(Map, u16, ReferenceImage)>,
    has_defeated_final_boss: bool,
}

impl ConsoleGame {
    pub const fn new(
        capture_device: VideoCapture,
        transform: CaptureTransform,
        hud_mask: MaskImage,
        bg_map: BackgroundMap,
        black_screen: ReferenceImage,
    ) -> Self {
        Self {
            capture_device,
            transform,
            hud_mask,
            bg_map,
            black_screen,
            current_map: Map::Hospital15F,
            current_room: 0,
            current_links: Vec::new(),
            has_defeated_final_boss: false,
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

        let hud_mask = MaskImage::new(transform.transform_bg(&hud_mask)?)?;

        let black_screen = all_black(&transform, &hud_mask)?;
        let black_screen = ReferenceImage::new(black_screen)?;

        Ok(Self::new(capture_device, transform, hud_mask, bg_map, black_screen))
    }

    fn set_room(&mut self, map: Map, room: u16) -> Result<()> {
        if map == self.current_map && room == self.current_room && !self.current_links.is_empty() {
            return Ok(());
        }

        self.current_map = map;
        self.current_room = room;
        self.has_defeated_final_boss = false;

        let Some(links) = self.bg_map.get(&(self.current_map, self.current_room)) else {
            bail!("No room links for room {} {}", self.current_map as u16, self.current_room);
        };

        self.current_links.clear();
        for (dest_map, dest_room, bg_path) in links {
            let bg_image = load_gray(bg_path.to_string_lossy())?;
            let bg_image = self.transform.transform_bg(&bg_image)?;
            let bg_image = self.hud_mask.mask(&bg_image)?;
            let reference_image = ReferenceImage::new(bg_image)?;
            self.current_links.push((*dest_map, *dest_room, reference_image));
        }

        Ok(())
    }

    fn check_frame(&mut self) -> Result<()> {
        let mut frame = Mat::default();
        self.capture_device.read(&mut frame)?;

        let capture_image = CaptureImage::new(frame)?;
        let capture = capture_image.transform(&self.transform, &self.hud_mask)?;

        for (dest_map, dest_room, reference_image) in &self.current_links {
            if reference_image.is_match_to(&capture)? {
                self.set_room(*dest_map, *dest_room)?;
                return Ok(());
            }
        }

        // if the player is in the final boss room, check for game completion by detecting the fade
        // to black
        if (self.current_map, self.current_room) == FINAL_BOSS_ROOM && !self.has_defeated_final_boss {
            // TODO: make sure this doesn't trigger too early, as this room is pretty dark to begin with
            if self.black_screen.is_match_to(&capture)? {
                self.has_defeated_final_boss = true;
            }
        }

        Ok(())
    }
}

impl Game for ConsoleGame {
    fn update(&mut self) -> GameState {
        match self.check_frame() {
            Ok(_) => GameState::Connected,
            Err(e) => {
                log::error!("Failed to check next capture frame: {e}");
                GameState::Disconnected
            }
        }
    }

    fn reconnect(&mut self, _platform: &PlatformRef) -> Result<()> {
        bail!("Video capture reconnect is not implemented");
    }

    fn map_id(&self) -> u16 {
        self.current_map as u16
    }

    fn room_id(&self) -> u16 {
        self.current_room
    }

    fn flag(&self, _stage: Stage, _flag_index: u32) -> bool {
        panic!("Flag check is not possible for console autosplitter");
    }

    fn has_item(&self, _item_id: Item) -> bool {
        panic!("Item check is not implemented for console autosplitter");
    }

    fn has_defeated_final_boss(&self) -> bool {
        self.has_defeated_final_boss
    }
}