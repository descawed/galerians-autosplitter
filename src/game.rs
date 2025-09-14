use anyhow::Result;
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::platform::PlatformRef;

mod console;
mod emulator;

pub use emulator::EmulatorGame;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize_repr, Deserialize_repr)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Connected,
    GameChanged,
    Disconnected,
}

pub trait Game {
    /// Update our information on the game state from the connected game instance
    fn update(&mut self) -> GameState;
    
    fn reconnect(&mut self, platform: &PlatformRef) -> Result<()>;

    fn is_at_main_menu(&self) -> bool;

    fn is_new_game_start(&self) -> bool;
    
    // this returns u16 instead of Map because we can't guarantee that there will always be a valid
    // map value in emulator memory
    fn map_id(&self) -> u16;
    
    fn room_id(&self) -> u16;
    
    fn flag(&self, stage: Stage, flag_index: u32) -> bool;
    
    fn has_defeated_final_boss(&self) -> bool;
    
    fn has_item(&self, item_id: Item) -> bool;
}