use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use opencv::core::min as cv_min;
use opencv::prelude::*;
use opencv::imgcodecs::{IMREAD_GRAYSCALE, imread};
use opencv::videoio::VideoCapture;

use super::{Game, GameState, Item, Map, Stage};
use crate::image::{
    MATCH_THRESHOLD,
    CaptureImage, CaptureTransform, CaptureTransformJson, MaskImage, MaskedImage, ReferenceImage,
    gray_float, is_fade_out,
};
use crate::platform::PlatformRef;
use crate::splits::Event;

const DEVICE_SETTINGS_PATH: &str = "device.json";
const BACKGROUND_PATH: &str = "assets/backgrounds/";
const CALIBRATION_IMAGE_PATH: &str = "assets/backgrounds/A1501_4_0.png";
const HUD_MASK_PATH: &str = "assets/backgrounds/hud_mask.png";
const MAIN_MENU_PATH: &str = "assets/backgrounds/main_menu.png";
const BG_MAP_PATH: &str = "assets/backgrounds/bg_map.json";
const FINAL_BOSS_ROOM: (Map, u16) = (Map::MushroomTower, 7);
const MAIN_MENU_MATCH_THRESHOLD: f64 = 0.7;
const MAIN_MENU_FADE_MAX: f64 = 0.05;
const GAME_END_FADE_MAX: f64 = 0.01;

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
    println!(concat!(
        "Before starting a run, we must first calibrate the video capture. ",
        "Please start a new game, wait until you gain control of Rion in the first room, and then press enter.",
    ));
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
    hud_mask: MaskImage,
    main_menu: ReferenceImage,
    bg_map: BackgroundMap,
    current_map: Map,
    current_room: u16,
    current_links: Vec<(Map, u16, ReferenceImage)>,
    has_defeated_final_boss: bool,
    is_at_main_menu: bool,
    is_new_game_start: bool,
}

impl ConsoleGame {
    pub const fn new(
        capture_device: VideoCapture,
        transform: CaptureTransform,
        hud_mask: MaskImage,
        main_menu: ReferenceImage,
        bg_map: BackgroundMap,
    ) -> Self {
        Self {
            capture_device,
            transform,
            hud_mask,
            main_menu,
            bg_map,
            current_map: Map::Hospital15F,
            current_room: 0,
            current_links: Vec::new(),
            has_defeated_final_boss: false,
            is_at_main_menu: false,
            is_new_game_start: false,
        }
    }

    pub fn connect(device_index: i32, force_calibrate: bool) -> Result<Self> {
        let mut capture_device = VideoCapture::new_def(device_index)?;
        let hud_mask = load_gray(HUD_MASK_PATH)?;
        let main_menu = load_gray(MAIN_MENU_PATH)?;
        let bg_map = load_bg_map()?;

        let mut settings = load_device_settings()?;
        let transform = if force_calibrate || !settings.contains_key(&device_index) {
            let transform = calibrate(&mut capture_device, &hud_mask)?;
            println!("Calibration complete. Transform: {transform:?}");
            settings.insert(device_index, transform.clone());
            save_device_settings(&settings)?;
            transform
        } else {
            settings.get(&device_index).unwrap().clone()
        };

        let hud_mask = MaskImage::new(transform.transform_bg(&hud_mask)?)?;

        let main_menu = transform.transform_bg(&main_menu)?;
        let main_menu = MaskedImage::unmasked(main_menu);
        let main_menu = ReferenceImage::new(main_menu)?;

        Ok(Self::new(capture_device, transform, hud_mask, main_menu, bg_map))
    }

    fn is_in_final_boss_room(&self) -> bool {
        (self.current_map, self.current_room) == FINAL_BOSS_ROOM
    }

    fn set_room(&mut self, map: Map, room: u16) -> Result<()> {
        if map == self.current_map && room == self.current_room && !self.current_links.is_empty() {
            return Ok(());
        }

        log::debug!("Room {map:?} {room}");

        self.current_map = map;
        self.current_room = room;
        self.has_defeated_final_boss = false;
        self.is_at_main_menu = false;
        self.is_new_game_start = false;

        self.current_links.clear();
        let Some(links) = self.bg_map.get(&(self.current_map, self.current_room)) else {
            if self.is_in_final_boss_room() {
                // don't expect any rooms after the final boss
                return Ok(());
            }
            bail!("No room links for room {} {}", self.current_map as u16, self.current_room);
        };

        for (dest_map, dest_room, bg_path) in links {
            let bg_image = load_gray(bg_path.to_string_lossy())?;
            let bg_image = self.transform.transform_bg(&bg_image)?;
            let bg_image = self.hud_mask.mask(&bg_image)?;
            let reference_image = ReferenceImage::new(bg_image)?;
            self.current_links.push((*dest_map, *dest_room, reference_image));
        }

        Ok(())
    }

    fn check_frame(&mut self, route_hint: Option<&Event>) -> Result<()> {
        let mut frame = Mat::default();
        self.capture_device.read(&mut frame)?;

        let capture_image = CaptureImage::new(frame)?;
        let trans_capture = capture_image.transform(&self.transform)?;
        let capture = self.hud_mask.mask(&trans_capture)?;

        let mut best_match = None;
        for (dest_map, dest_room, reference_image) in &self.current_links {
            let score = if (*dest_map, *dest_room) == FINAL_BOSS_ROOM {
                // the background displayed in this room is a darkened version of the actual
                // background image, and our matching algorithm has trouble with very dark images
                // anyway, so we need to brighten the capture image for comparison
                let mut brightened = Mat::default();
                trans_capture.convert_to(&mut brightened, -1, 5.0, 0.0)?;
                let mut clipped = Mat::default();
                cv_min(&brightened, &1.0, &mut clipped)?;
                let masked = self.hud_mask.mask(&clipped)?;
                reference_image.match_score(&masked)?
            } else {
                reference_image.match_score(&capture)?
            };

            if score > MATCH_THRESHOLD {
                // if one of the matches is the expected next room, always take that one
                let route_match = match route_hint {
                    Some(Event::Room(route_map, route_room)) => (*route_map, *route_room) == (*dest_map, *dest_room),
                    Some(Event::Room2((route_map1, route_room1), (route_map2, route_room2))) => {
                        (*route_map1, *route_room1) == (*dest_map, *dest_room) || (*route_map2, *route_room2) == (*dest_map, *dest_room)
                    }
                    _ => false,
                };
                if route_match {
                    best_match = Some((1.0, *dest_map, *dest_room));
                    break;
                }
                // if not, take the match with the highest score
                let is_best = match best_match {
                    Some((best_score, _, _)) => score > best_score,
                    None => true,
                };
                if is_best {
                    best_match = Some((score, *dest_map, *dest_room));
                }
            }
        }

        if let Some((_, dest_map, dest_room)) = best_match {
            self.set_room(dest_map, dest_room)?;
            return Ok(());
        }

        // if the player is in the final boss room, check for game completion by detecting the fade
        // to black
        if self.is_in_final_boss_room() && !self.has_defeated_final_boss {
            // FIXME: this would also trigger if the player dies
            if is_fade_out(&trans_capture, GAME_END_FADE_MAX)? {
                self.has_defeated_final_boss = true;
                return Ok(());
            }
        }

        // if we're at the main menu, check for the start of a new game
        if self.is_at_main_menu && !self.is_new_game_start {
            // FIXME: this also triggers if the trailer starts playing
            if is_fade_out(&trans_capture, MAIN_MENU_FADE_MAX)? {
                self.is_at_main_menu = false;
                self.is_new_game_start = true;
                log::debug!("New game start");
                return Ok(());
            }
        }

        // lastly, check if the player is at the main menu
        if !self.is_at_main_menu {
            let capture = MaskedImage::unmasked(trans_capture);
            // the room 204 door triggers a false positive for the main menu with the normal match
            // threshold, so we use a slightly higher threshold here
            let score = self.main_menu.match_score(&capture)?;
            if score > MAIN_MENU_MATCH_THRESHOLD {
                self.set_room(Map::Hospital15F, 0)?;
                log::debug!("At main menu: {score}");
                self.is_at_main_menu = true;
                self.is_new_game_start = false;
            }
        }

        Ok(())
    }
}

impl Game for ConsoleGame {
    fn update(&mut self, route_hint: Option<&Event>) -> GameState {
        match self.check_frame(route_hint) {
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

    fn is_at_main_menu(&self) -> bool {
        self.is_at_main_menu
    }

    fn is_new_game_start(&self) -> bool {
        self.is_new_game_start
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

    fn has_defeated_final_boss(&self) -> bool {
        self.has_defeated_final_boss
    }

    fn has_item(&self, _item_id: Item) -> bool {
        panic!("Item check is not implemented for console autosplitter");
    }
}