use crate::game::{Item, Map, Stage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Room(Map, u16),
    Flag(Stage, u32),
    Item(Item),
}

// FIXME: might need edits for better pacing
pub const KEY_EVENT_SPLITS: [Event; 46] = [
    // Stage A
    Event::Item(Item::SecurityCard),
    Event::Item(Item::FreezerRoomKey),
    Event::Item(Item::PpecStorageKey),
    Event::Item(Item::Fuse),
    Event::Item(Item::LiquidExplosive),
    Event::Item(Item::SpecialPpecOfficeKey),
    Event::Item(Item::SecurityCardReformatted),
    Event::Item(Item::PhotoOfParents), // skipping control room key since you get it in the same room
    Event::Item(Item::TestLabKey),
    Event::Item(Item::ResearchLabKey),
    Event::Item(Item::TwoHeadedSnake),
    Event::Item(Item::TwoHeadedMonkey),
    Event::Item(Item::TwoHeadedWolf),
    Event::Item(Item::TwoHeadedEagle),
    Event::Room(Map::Hospital14F, 4), // A1405 (Lem)
    // Stage B
    Event::Room(Map::YourHouseFirstFloor, 3), // B0104
    Event::Item(Item::BackdoorKey),
    Event::Item(Item::SecondFloorKey),
    Event::Item(Item::DoorKnob),
    Event::Item(Item::BedroomKey),
    Event::Item(Item::MothersRing),
    Event::Item(Item::FathersRing),
    Event::Item(Item::ThreeBall),
    Event::Item(Item::NineBall),
    Event::Item(Item::ShedKey),
    Event::Item(Item::LiliasDoll),
    Event::Room(Map::YourHouseFirstFloor, 10), // B0111 (Birdman)
    // Stage C
    Event::Room(Map::HotelLower, 0), // C0101
    Event::Flag(Stage::C, 5), // learned secret knock
    Event::Flag(Stage::C, 17), // successfully performed secret knock
    Event::Flag(Stage::C, 10), // Crovic
    Event::Flag(Stage::C, 143), // bomb guy
    Event::Flag(Stage::C, 54), // defeat enemy in 3F hall
    Event::Flag(Stage::C, 44), // Suzan
    Event::Flag(Stage::C, 142), // gun guy
    Event::Flag(Stage::C, 47), // defeat enemies in room 305
    Event::Flag(Stage::C, 35), // defeat enemies in room 301
    Event::Flag(Stage::C, 95), // take phone call in room 205
    Event::Flag(Stage::C, 23), // defeat enemies in 2F hall
    Event::Flag(Stage::C, 11), // defeat enemy in room 202
    Event::Room(Map::Hotel3F, 4), // C0305 (Rainheart)
    Event::Room(Map::Hotel3F, 6), // C0307 (post-Rainheart)
    Event::Room(Map::HotelLower, 5), // C1101 (Rita)
    // Stage D
    Event::Room(Map::MushroomTower, 0), // D0001
    Event::Room(Map::MushroomTower, 4), // D1001 (Cain)
    Event::Room(Map::MushroomTower, 7), // D1004 (Dorothy)
];