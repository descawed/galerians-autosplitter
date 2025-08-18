use crate::game::{Item, Map, Stage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Room(Map, u16),
    // there are two rooms in the game (A1401 and A1310) that are mapped twice. I'm not sure if the
    // game actually uses both mappings, but we'll check for both just to cover our bases.
    // actually, there's also a third room, A14RH, which is mapped four times, but that room is
    // unused, so we won't worry about it.
    Room2((Map, u16), (Map, u16)),
    Flag(Stage, u32),
    Item(Item),
}

macro_rules! room {
    ($map:ident $room:expr) => {Event::Room(Map::$map, $room)};
}

macro_rules! room2 {
    ($map:ident $room:expr, $map2:ident $room2:expr) => {Event::Room2((Map::$map, $room), (Map::$map2, $room2))};
}

macro_rules! flag {
    ($stage:ident $flag:expr) => {Event::Flag(Stage::$stage, $flag)};
}

macro_rules! item {
    ($name:ident) => {Event::Item(Item::$name)};
}

pub const KEY_EVENT_SPLITS: [Event; 45] = [
    // Stage A
    item!(SecurityCard),
    item!(FreezerRoomKey),
    item!(PpecStorageKey),
    item!(Fuse),
    item!(LiquidExplosive),
    item!(SpecialPpecOfficeKey),
    item!(SecurityCardReformatted),
    item!(PhotoOfParents), // skipping control room key since you get it in the same room
    item!(TestLabKey),
    item!(ResearchLabKey),
    item!(TwoHeadedSnake),
    item!(TwoHeadedMonkey),
    item!(TwoHeadedWolf),
    item!(TwoHeadedEagle),
    // Lem is hardly a boss fight because you just press a button and he dies, so I decided to just
    // make the Lem split the whole end part of Stage A
    // Event::Room(Map::Hospital14F, 4), // A1405 (Lem)
    room!(YourHouse1F 11), // B0112; end of Stage A
    // Stage B
    item!(BackdoorKey),
    item!(SecondFloorKey),
    item!(DoorKnob),
    item!(BedroomKey),
    item!(MothersRing),
    item!(FathersRing),
    item!(ThreeBall),
    item!(NineBall),
    item!(ShedKey),
    item!(LiliasDoll),
    // because the Birdman fight happens almost immediately after getting Lilia's Doll, I don't
    // think we need a separate split
    // Event::Room(Map::YourHouse1F, 10), // B0111 (Birdman)
    room!(Hotel1F 0), // C0101; end of Stage B
    // Stage C
    flag!(C 5), // learned secret knock
    flag!(C 17), // successfully performed secret knock
    flag!(C 10), // Crovic
    flag!(C 144), // Priest
    flag!(C 143), // bomb guy
    flag!(C 54), // defeat enemy in 3F hall
    flag!(C 145), // Suzan
    flag!(C 142), // gun guy
    flag!(C 47), // defeat enemies in room 305
    flag!(C 35), // defeat enemies in room 301
    flag!(C 95), // take phone call in room 205
    flag!(C 23), // defeat enemies in 2F hall
    flag!(C 11), // defeat enemy in room 202
    room!(Hotel3F 4), // C0305 (Rainheart)
    room!(Hotel3F 6), // C0307 (post-Rainheart)
    room!(Hotel1F 5), // C1101 (Rita)
    room!(MushroomTower 0), // D0001; end of Stage C
    // Stage D
    room!(MushroomTower 4), // D1001 (Cain)
    room!(MushroomTower 7), // D1004 (Dorothy)
];

pub const DOOR_SPLITS: [Event; 176] = [
    // Stage A
    room!(Hospital15F 1), // A1502 (security card)
    room!(Hospital15F 12), // A15RA
    room!(Hospital15F 2), // A1503 (use security card)
    room!(Hospital15F 11), // A1512
    room!(Hospital15F 3), // A1504 (freezer room key)
    room!(Hospital15F 11), // A1512
    room!(Hospital15F 13), // A15RB
    room!(Hospital15F 14), // A15RC
    room!(Hospital15F 4), // A1505 (PPEC storage key)
    room!(Hospital15F 14), // A15RC
    room!(Hospital15F 6), // A1507 (fuse)
    room!(Hospital15F 14), // A15RC (use fuse)
    room!(Hospital15F 0), // A1501 (liquid explosive)
    room!(Hospital15F 14), // A15RC (use liquid explosive)
    room!(Hospital14F 10), // A14RA
    room!(Hospital14F 1), // A1402
    room!(Hospital14F 0), // A1508
    room!(Hospital14F 8), // A1409 (special PPEC office key)
    room!(Hospital14F 0), // A1508
    room!(Hospital14F 1), // A1402
    room!(Hospital14F 10), // A14RA
    room!(Hospital15F 14), // A15RC
    room!(Hospital15F 13), // A15RB (use special PPEC office key)
    room2!(Hospital15F 7, Hospital14F 5), // A1401 (reformat security card)
    room!(Hospital15F 13), // A15RB
    room!(Hospital15F 14), // A15RC
    room!(Hospital14F 10), // A14RA (use reformatted security card)
    room!(Hospital14F 12), // A14RF
    room!(Hospital14F 7), // A1408
    room!(Hospital13F 15), // A13RA
    room!(Hospital13F 0), // A1301 (control room key, photo of parents)
    room!(Hospital13F 15), // A13RA
    room2!(Hospital13F 9, Hospital13F 10), // A1310
    room!(Hospital13F 16), // A13RB
    room!(Hospital13F 1), // A1302 (test lab key)
    room!(Hospital13F 16), // A13RB
    room2!(Hospital13F 9, Hospital13F 10), // A1310
    room!(Hospital14F 13), // A14RG
    room!(Hospital14F 2), // A1403 (research lab key)
    room!(Hospital14F 13), // A14RG
    room2!(Hospital13F 9, Hospital13F 10), // A1310 (use test lab key)
    room!(Hospital13F 5), // A1306 (two-headed snake)
    room2!(Hospital13F 9, Hospital13F 10), // A1310 (use control room key)
    room!(Hospital13F 7), // A1308 (unlock Clinic Chief's office, use research lab key)
    room!(Hospital13F 8), // A1309 (two-headed monkey)
    room!(Hospital13F 7), // A1308 (unlock armory)
    room2!(Hospital13F 9, Hospital13F 10), // A1310
    room!(Hospital13F 11), // A1312 (two-headed wolf)
    room2!(Hospital13F 9, Hospital13F 10), // A1310
    room!(Hospital13F 15), // A13RA
    room!(Hospital13F 2), // A1303 (two-headed eagle)
    room!(Hospital13F 3), // A1304
    room!(Hospital13F 17), // A13RC
    room!(Hospital13F 4), // A1305
    room!(Hospital13F 19), // A13RE
    room!(Hospital14F 18), // A14KD
    room!(Hospital14F 4), // A1405 (Lem)
    // Stage B
    room!(YourHouse1F 11), // B0112
    room!(YourHouse1F 9), // B0110 (backdoor key, use backdoor key)
    room!(YourHouse1F 3), // B0104
    room!(YourHouse1F 12), // B01RA
    room!(YourHouse1F 5), // B0106 (second floor key)
    room!(YourHouse1F 12), // B01RA
    room!(YourHouse1F 13), // B01RB
    room!(YourHouse1F 14), // B01RC
    room!(YourHouse1F 7), // B0108 (door knob)
    room!(YourHouse1F 14), // B01RC
    room!(YourHouse1F 13), // B01RB
    room!(YourHouse1F 12), // B01RA
    room!(YourHouse1F 3), // B0104 (use door knob)
    room!(YourHouse1F 0), // B0101
    room!(YourHouse2F 0), // B0201 (use second floor key)
    room!(YourHouse2F 9), // B02RA
    room!(YourHouse2F 1), // B0202 (bedroom key)
    room!(YourHouse2F 9), // B02RA
    room!(YourHouse2F 10), // B02RB
    room!(YourHouse2F 11), // B02RC (use bedroom key)
    room!(YourHouse2F 10), // B02RB
    room!(YourHouse1F 13), // B01RB
    room!(YourHouse1F 12), // B01RA
    room!(YourHouse1F 4), // B0105 (mother's ring)
    room!(YourHouse1F 12), // B01RA
    room!(YourHouse1F 3), // B0104
    room!(YourHouse1F 0), // B0101
    room!(YourHouse2F 0), // B0201
    room!(YourHouse2F 9), // B02RA
    room!(YourHouse2F 10), // B02RB
    room!(YourHouse2F 11), // B02RC
    room!(YourHouse2F 6), // B0207 (father's ring, use mother's ring)
    room!(YourHouse2F 11), // B02RC
    room!(YourHouse2F 10), // B02RB
    room!(YourHouse2F 9), // B02RA
    room!(YourHouse2F 3), // B0204 (use mother's ring, use father's ring)
    room!(YourHouse2F 2), // B0203 (3 ball)
    room!(YourHouse2F 3), // B0204
    room!(YourHouse2F 9), // B02RA
    room!(YourHouse2F 0), // B0201
    room!(YourHouse1F 0), // B0101
    room!(YourHouse1F 11), // B0112
    room!(YourHouse1F 9), // B0110 (9 ball)
    room!(YourHouse1F 3), // B0104
    room!(YourHouse1F 12), // B01RA
    room!(YourHouse1F 13), // B01RB
    room!(YourHouse1F 14), // B01RC
    room!(YourHouse1F 7), // B0108 (use 3 ball, use 9 ball)
    room!(YourHouse1F 15), // B0001 (shed key)
    room!(YourHouse1F 7), // B0108
    room!(YourHouse1F 14), // B01RC
    room!(YourHouse1F 13), // B01RB
    room!(YourHouse1F 12), // B01RA
    room!(YourHouse1F 3), // B0104
    room!(YourHouse1F 0), // B0101
    room!(YourHouse1F 11), // B0112
    room!(YourHouse1F 10), // B0111 (use shed key)
    room!(YourHouse1F 2), // B0103 (Lilia's doll)
    room!(YourHouse1F 10), // B0111 (Birdman)
    // Stage C
    room!(Hotel1F 0), // C0101 (front desk)
    room!(Hotel3F 6), // C0307
    room!(Hotel3F 1), // C0302 (302)
    room!(Hotel3F 6), // C0307
    // we have to be tricky at a few spots in the hotel, because a lot of the FMVs that show when
    // you first enter rooms don't actually take you to that room, they just play the FMV and then
    // reload the current room. we can't detect that with a load check, so we have to watch for
    // flags instead.
    flag!(C 50), // 306
    room!(Hotel1F 0), // C0101 (blood trail)
    room!(Hotel1F 1), // C0102 (staff room)
    room!(Hotel1F 0), // C0101 (learn knock)
    room!(Hotel2F 6), // C0207 (knock)
    room!(Hotel2F 3), // C0204 (Joule, D-Felon)
    room!(Hotel2F 6), // C0207
    room!(Hotel2F 0), // C0201 (Crovic)
    room!(Hotel2F 6), // C0207
    flag!(C 27), // 206
    room!(Hotel2F 5), // C0206 (priest)
    room!(Hotel2F 6), // C0207
    flag!(C 15), // 203
    room!(Hotel2F 2), // C0203 (bomb guy)
    room!(Hotel2F 6), // C0207
    room!(Hotel3F 6), // C0307 (3F rabbit)
    flag!(C 44), // 304
    room!(Hotel3F 3), // C0304 (Suzan)
    room!(Hotel3F 6), // C0307
    flag!(C 41), // 303
    room!(Hotel3F 2), // C0303
    room!(Hotel3F 6), // C0307
    room!(Hotel3F 4), // C0305 (rabbits)
    room!(Hotel3F 6), // C0307
    room!(Hotel3F 0), // C0301 (rabbits)
    room!(Hotel3F 6), // C0307
    room!(Hotel2F 6), // C0207
    room!(Hotel2F 4), // C0205 (phone call)
    room!(Hotel2F 6), // C0207 (rabbits)
    room!(Hotel2F 1), // C0202 (rabbit)
    room!(Hotel2F 6), // C0207 (Rainheart cutscene)
    room!(Hotel3F 6), // C0307
    room!(Hotel3F 4), // C0305 (Rainheart)
    room!(Hotel3F 6), // C0307
    room!(Hotel1F 0), // C0101
    room!(Hotel1F 1), // C0102 (circuit breaker)
    room!(Hotel1F 0), // C0101
    room!(Hotel1F 4), // C1001
    room!(Hotel1F 6), // C1102
    room!(Hotel1F 8), // C1104
    room!(Hotel1F 6), // C1102
    room!(Hotel1F 5), // C1101 (Rita)
    // Stage D
    room!(MushroomTower 0), // D0001
    room!(MushroomTower 8), // D0101
    room!(MushroomTower 0), // D0001
    room!(MushroomTower 1), // D0002
    room!(MushroomTower 8), // D0101
    room!(MushroomTower 1), // D0002
    room!(MushroomTower 2), // D0003
    room!(MushroomTower 8), // D0101
    room!(MushroomTower 2), // D0003
    room!(MushroomTower 3), // D0004
    room!(MushroomTower 8), // D0101
    room!(MushroomTower 3), // D0004
    // I think we might briefly go to D1003 here, but that's just a cutscene, so no need to make it
    // a split
    room!(MushroomTower 4), // D1001 (Cain)
    room!(MushroomTower 7), // D1004 (Dorothy)
];