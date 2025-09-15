use std::thread;
use std::time::Duration;

use anyhow::Result;

use super::{Game, GameState, Item, Stage};
use crate::platform::{Emulator, PlatformInterface, PlatformRef};
use crate::splits::Event;

const SEARCH_STRING: &[u8] = b"GALERIANS";
const NEW_GAME_MENU_STATE: i32 = 99;
const TRAILER_MENU_STATE: i32 = 200;
const GAME_END_FLAGS: [u32; 4] = [37, 38, 39, 80];
const FLAG_BANK_SIZE: u32 = 4 * 8;
const MAX_ITEMS: usize = 41;
const EMULATOR_RETRY_DURATION: Duration = Duration::from_millis(5000);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GameCheck {
    Same,
    Changed,
    Unknown,
}

impl GameCheck {
    pub const fn is_valid(&self) -> bool {
        !matches!(self, GameCheck::Unknown)
    }
}

#[derive(Debug, Clone)]
pub struct GameVersion {
    name: &'static str,
    search_string_address: u32,
    main_menu_state_address: u32,
    menu_module_id_address: u32,
    main_menu_module_id: i16,
    map_id_address: u32,
    room_id_address: u32,
    flag_banks_address: u32,
    inventory_address: u32,
    inventory_count_address: u32,
}

impl GameVersion {
    pub const fn flag_bank_address(&self, stage: Stage, flag_index: u32) -> (u32, u64) {
        let (bank_offset, bit_index) = if flag_index >= 128 {
            (FLAG_BANK_SIZE * 2, flag_index - 128)
        } else if flag_index >= 64 {
            (FLAG_BANK_SIZE, flag_index - 64)
        } else {
            (0, flag_index)
        };

        let stage_offset = stage as u32 * 8;

        (self.flag_banks_address + bank_offset + stage_offset, 1u64 << bit_index)
    }

    pub fn detect(emulator: &Emulator) -> Option<&'static Self> {
        for version in &GAME_VERSIONS {
            if version.validate(emulator) {
                log::info!("Detected game version: {}", version.name);
                return Some(version);
            }
        }

        None
    }

    pub fn validate(&self, emulator: &Emulator) -> bool {
        let mut compare_value = [0u8; SEARCH_STRING.len()];
        emulator.read_into(self.search_string_address, &mut compare_value);
        compare_value == SEARCH_STRING
    }
}

const GAME_VERSIONS: [GameVersion; 2] = [
    GameVersion {
        name: "NTSC-U",
        search_string_address: 0x8011AE40,
        main_menu_state_address: 0x801FCF00,
        menu_module_id_address: 0x80190E9C,
        main_menu_module_id: 111,
        map_id_address: 0x801912DC,
        room_id_address: 0x801912DE,
        flag_banks_address: 0x801AF9A0,
        inventory_address: 0x801AFAAC,
        inventory_count_address: 0x801AFAFE,
    },
    GameVersion {
        name: "NTSC-J",
        search_string_address: 0x80193830,
        main_menu_state_address: 0x801FE2E0,
        menu_module_id_address: 0x80190E08,
        main_menu_module_id: 112,
        map_id_address: 0x801912B4,
        room_id_address: 0x801912B6,
        flag_banks_address: 0x801AFFA0,
        inventory_address: 0x801B00AC,
        inventory_count_address: 0x801B00FE,
    },
];

fn wait_for_emulator(platform: &PlatformRef) -> Emulator {
    log::info!("Waiting for emulator...");
    loop {
        if let Some(emulator) = platform.search_for_emulator() {
            return emulator;
        }

        thread::sleep(EMULATOR_RETRY_DURATION);
    }
}

fn wait_for_version(emulator: &mut Emulator, platform: &PlatformRef) -> &'static GameVersion {
    log::info!("Waiting for game to be loaded...");
    loop {
        // make sure we don't lose the emulator while we're waiting for the game
        if !emulator.check_pulse() {
            log::warn!("Lost emulator");
            *emulator = wait_for_emulator(platform);
            log::info!("Waiting for game to be loaded...");
        }

        if let Some(version) = GameVersion::detect(emulator) {
            return version;
        }

        thread::sleep(EMULATOR_RETRY_DURATION);
    }
}

#[derive(Debug)]
pub struct EmulatorGame {
    version: &'static GameVersion,
    emulator: Emulator,
}

impl EmulatorGame {
    pub const fn new(version: &'static GameVersion, emulator: Emulator) -> Self {
        Self { version, emulator }
    }

    pub fn connect(platform: &PlatformRef) -> Self {
        let mut emulator = wait_for_emulator(platform);
        let version = wait_for_version(&mut emulator, platform);
        Self::new(version, emulator)
    }

    pub fn main_menu_state(&self) -> i32 {
        let menu_module_id: i16 = self.emulator.read_num(self.version.menu_module_id_address);
        if menu_module_id != self.version.main_menu_module_id {
            return -1;
        }

        self.emulator.read_num(self.version.main_menu_state_address)
    }

    /// Check that the emulator providing the game memory is still running
    pub fn check_emulator(&self) -> bool {
        self.emulator.check_pulse()
    }

    /// Check to make sure a different game hasn't been loaded in the emulator since we started watching
    pub fn check_version(&mut self) -> GameCheck {
        if self.version.validate(&self.emulator) {
            return GameCheck::Same;
        }

        self.search_for_game()
    }

    /// Search for the next game version to be loaded since the last one was unloaded
    pub fn search_for_game(&mut self) -> GameCheck {
        // the game has changed. see if it's another game we know about or a complete unknown.
        match GameVersion::detect(&self.emulator) {
            Some(new_version) => {
                self.version = new_version;
                GameCheck::Changed
            }
            None => GameCheck::Unknown,
        }
    }
}

impl Game for EmulatorGame {
    fn update(&mut self, _route_hint: Option<&Event>) -> GameState {
        if !self.check_emulator() {
            return GameState::Disconnected;
        }
        
        match self.check_version() {
            GameCheck::Same => GameState::Connected,
            GameCheck::Changed => GameState::GameChanged,
            GameCheck::Unknown => GameState::Disconnected,
        }
    }
    
    fn reconnect(&mut self, platform: &PlatformRef) -> Result<()> {
        if !self.check_emulator() {
            self.emulator = wait_for_emulator(platform);
        }
        
        if !self.search_for_game().is_valid() {
            self.version = wait_for_version(&mut self.emulator, platform);
        }
        
        Ok(())
    }

    fn is_at_main_menu(&self) -> bool {
        (0..NEW_GAME_MENU_STATE).contains(&self.main_menu_state())
    }

    fn is_new_game_start(&self) -> bool {
        (NEW_GAME_MENU_STATE..TRAILER_MENU_STATE).contains(&self.main_menu_state())
    }

    fn map_id(&self) -> u16 {
        self.emulator.read_num(self.version.map_id_address)
    }

    fn room_id(&self) -> u16 {
        self.emulator.read_num(self.version.room_id_address)
    }

    fn flag(&self, stage: Stage, flag_index: u32) -> bool {
        let (bank_address, bit_value) = self.version.flag_bank_address(stage, flag_index);
        let bank: u64 = self.emulator.read_num(bank_address);
        bank & bit_value != 0
    }

    fn has_defeated_final_boss(&self) -> bool {
        for &flag in &GAME_END_FLAGS {
            if !self.flag(Stage::D, flag) {
                return false;
            }
        }

        true
    }

    fn has_item(&self, item_id: Item) -> bool {
        let num_items: u16 = self.emulator.read_num(self.version.inventory_count_address);
        let items: [i16; MAX_ITEMS] = self.emulator.read_nums(self.version.inventory_address);
        items[..num_items as usize].contains(&(item_id as i16))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flag_bank_address_low() {
        let version = &GAME_VERSIONS[0];
        let (bank_address, bit_value) = version.flag_bank_address(Stage::D, GAME_END_FLAGS[1]);
        assert_eq!(bank_address, 0x801af9b8);
        assert_eq!(bit_value, 0x4000000000);
    }

    #[test]
    fn test_flag_bank_address_high() {
        let version = &GAME_VERSIONS[1];
        let (bank_address, bit_value) = version.flag_bank_address(Stage::D, GAME_END_FLAGS[3]);
        assert_eq!(bank_address, 0x801affd8);
        assert_eq!(bit_value, 0x10000);
    }
}