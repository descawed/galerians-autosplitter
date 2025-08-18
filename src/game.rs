use anyhow::Result;

use crate::shmem::GameMemory;

const SEARCH_STRING: &[u8] = b"GALERIANS";
const NEW_GAME_MENU_STATE: i32 = 99;
const TRAILER_MENU_STATE: i32 = 200;
const GAME_END_FLAGS: [u32; 4] = [37, 38, 39, 80];
const FLAG_BANK_SIZE: u32 = 4 * 8;
const MAX_ITEMS: usize = 41;

// silencing "unused" warnings on these enums. even if all the possible values aren't used today,
// I still want them to be defined here both as a reference and for potential future use.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Stage {
    A = 0,
    B = 1,
    C = 2,
    D = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Map {
    Hospital15F = 0,
    Hospital14F = 1,
    Hospital13F = 2,
    YourHouse1F = 3,
    YourHouse2F = 4,
    Hotel1F = 5,
    Hotel2F = 6,
    Hotel3F = 7,
    MushroomTower = 8,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum Item {
    MemoryChip15F = 0,
    SecurityCard = 1,
    Beeject = 2,
    FreezerRoomKey = 3,
    PpecStorageKey = 4,
    Fuse = 5,
    LiquidExplosive = 6,
    MemoryChip14F = 7,
    SecurityCardReformatted = 8,
    SpecialPpecOfficeKey = 9,
    MemoryChip13F = 10,
    TestLabKey = 11,
    ControlRoomKey = 12,
    ResearchLabKey = 13,
    TwoHeadedSnake = 14,
    TwoHeadedMonkey = 15,
    TwoHeadedWolf = 16,
    TwoHeadedEagle = 17,
    YourHouseMemoryChip = 18,
    BackdoorKey = 19,
    DoorKnob = 20,
    NineBall = 21,
    MothersRing = 22,
    FathersRing = 23,
    LiliasDoll = 24,
    Metamorphosis = 25,
    BedroomKey = 26,
    SecondFloorKey = 27,
    MedicalStaffNotes = 28,
    GProjectReport = 29,
    PhotoOfParents = 30,
    RionsTestData = 31,
    DrLemsNotes = 32,
    NewReplicativeComputerTheory = 33,
    DrPascallesDiary = 34,
    LetterFromElsa = 35,
    Newspaper = 36,
    ThreeBall = 37,
    ShedKey = 38,
    LetterFromLilia = 39,
    DFelon = 40,
}

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

    pub fn detect(game_memory: &GameMemory) -> Option<&'static Self> {
        for version in &GAME_VERSIONS {
            if version.validate(game_memory) {
                log::info!("Detected game version: {}", version.name);
                return Some(version);
            }
        }

        None
    }

    pub fn validate(&self, game_memory: &GameMemory) -> bool {
        let compare_value = game_memory.read_slice(self.search_string_address, SEARCH_STRING.len());
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

#[derive(Debug)]
pub struct Game {
    version: &'static GameVersion,
    memory: GameMemory,
}

impl Game {
    pub const fn new(version: &'static GameVersion, memory: GameMemory) -> Self {
        Self { version, memory }
    }

    pub fn version_name(&self) -> &'static str {
        self.version.name
    }

    pub fn main_menu_state(&self) -> i32 {
        let menu_module_id: i16 = self.memory.read_num(self.version.menu_module_id_address);
        if menu_module_id != self.version.main_menu_module_id {
            return -1;
        }

        self.memory.read_num(self.version.main_menu_state_address)
    }

    pub fn is_new_game_start(&self) -> bool {
        (NEW_GAME_MENU_STATE..TRAILER_MENU_STATE).contains(&self.main_menu_state())
    }

    pub fn is_at_main_menu(&self) -> bool {
        (0..NEW_GAME_MENU_STATE).contains(&self.main_menu_state())
    }

    pub fn map_id(&self) -> u16 {
        self.memory.read_num(self.version.map_id_address)
    }

    pub fn room_id(&self) -> u16 {
        self.memory.read_num(self.version.room_id_address)
    }

    pub fn flag(&self, stage: Stage, flag_index: u32) -> bool {
        let (bank_address, bit_value) = self.version.flag_bank_address(stage, flag_index);
        let bank: u64 = self.memory.read_num(bank_address);
        bank & bit_value != 0
    }

    pub fn has_defeated_final_boss(&self) -> bool {
        for &flag in &GAME_END_FLAGS {
            if !self.flag(Stage::D, flag) {
                return false;
            }
        }

        true
    }

    pub fn has_item(&self, item_id: Item) -> bool {
        let num_items: u16 = self.memory.read_num(self.version.inventory_count_address);
        let items: [i16; MAX_ITEMS] = self.memory.read_nums(self.version.inventory_address);
        items[..num_items as usize].contains(&(item_id as i16))
    }

    /// Check that the emulator providing the game memory is still running
    pub fn check_emulator(&self) -> bool {
        self.memory.check_pulse()
    }

    /// Search for a new emulator process when we've lost our old one
    pub fn search_for_emulator(&mut self) -> Result<bool> {
        Ok(match GameMemory::discover()? {
            Some(memory) => {
                self.memory = memory;
                true
            }
            None => false,
        })
    }

    /// Check to make sure a different game hasn't been loaded in the emulator since we started watching
    pub fn check_version(&mut self) -> GameCheck {
        if self.version.validate(&self.memory) {
            return GameCheck::Same;
        }

        self.search_for_game()
    }

    /// Search for the next game version to be loaded since the last one was unloaded
    pub fn search_for_game(&mut self) -> GameCheck {
        // the game has changed. see if it's another game we know about or a complete unknown.
        match GameVersion::detect(&self.memory) {
            Some(new_version) => {
                self.version = new_version;
                GameCheck::Changed
            }
            None => GameCheck::Unknown,
        }
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