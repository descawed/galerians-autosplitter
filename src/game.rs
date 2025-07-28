use crate::shmem::GameMemory;

const SEARCH_STRING: &[u8] = b"GALERIANS";
const NEW_GAME_MENU_STATE: i32 = 99;
const STAGE_D_INDEX: u32 = 3;
const GAME_END_FLAGS: [u32; 4] = [37, 38, 39, 80];
const FLAG_BANK_SIZE: u32 = 4 * 8;

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
}

impl GameVersion {
    pub const fn flag_bank_address(&self, stage_index: u32, flag_index: u32) -> (u32, u64) {
        let (bank_offset, bit_index) = if flag_index >= 128 {
            (FLAG_BANK_SIZE * 2, flag_index - 128)
        } else if flag_index >= 64 {
            (FLAG_BANK_SIZE, flag_index - 64)
        } else {
            (0, flag_index)
        };

        let stage_offset = stage_index * 8;

        (self.flag_banks_address + bank_offset + stage_offset, 1u64 << bit_index)
    }

    pub fn detect(game_memory: &GameMemory) -> Option<&'static Self> {
        for version in &GAME_VERSIONS {
            let compare_value = game_memory.read_slice(version.search_string_address, SEARCH_STRING.len());
            if compare_value == SEARCH_STRING {
                log::info!("Detected game version: {}", version.name);
                return Some(version);
            }
        }

        None
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

    pub fn main_menu_state(&self) -> i32 {
        let menu_module_id: i16 = self.memory.read_num(self.version.menu_module_id_address);
        if menu_module_id != self.version.main_menu_module_id {
            return -1;
        }

        self.memory.read_num(self.version.main_menu_state_address)
    }

    pub fn is_new_game_start(&self) -> bool {
        self.main_menu_state() >= NEW_GAME_MENU_STATE
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

    pub fn flag(&self, stage_index: u32, flag_index: u32) -> bool {
        let (bank_address, bit_value) = self.version.flag_bank_address(stage_index, flag_index);
        let bank: u64 = self.memory.read_num(bank_address);
        bank & bit_value != 0
    }

    pub fn has_defeated_final_boss(&self) -> bool {
        for &flag in &GAME_END_FLAGS {
            if !self.flag(STAGE_D_INDEX, flag) {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flag_bank_address_low() {
        let version = &GAME_VERSIONS[0];
        let (bank_address, bit_value) = version.flag_bank_address(STAGE_D_INDEX, GAME_END_FLAGS[1]);
        assert_eq!(bank_address, 0x801af9b8);
        assert_eq!(bit_value, 0x4000000000);
    }

    #[test]
    fn test_flag_bank_address_high() {
        let version = &GAME_VERSIONS[1];
        let (bank_address, bit_value) = version.flag_bank_address(STAGE_D_INDEX, GAME_END_FLAGS[3]);
        assert_eq!(bank_address, 0x801affd8);
        assert_eq!(bit_value, 0x10000);
    }
}