"""
This script generates the background images, background masks, and room metadata needed by the console autosplitter. It
uses my Galerians mod SDK "galsdk" (https://github.com/descawed/galsdk) and expects to be run from the galsdk virtual
environment. The script takes two CLI arguments: the path to the galsdk project directory and the path to the output
directory.
"""

import json
import sys
from enum import IntEnum
from pathlib import Path

from PIL import Image, ImageEnhance

from galsdk.game import STAGE_MAPS, Stage
from galsdk.module import Entrance, RoomModule
from galsdk.project import Project
from galsdk.tim import TimFormat
from psx.tim import Transparency


class Map(IntEnum):
    HOSPITAL_15F = 0
    HOSPITAL_14F = 1
    HOSPITAL_13F = 2
    YOUR_HOUSE_1F = 3
    YOUR_HOUSE_2F = 4
    HOTEL_1F = 5
    HOTEL_2F = 6
    HOTEL_3F = 7
    MUSHROOM_TOWER = 8


class RoomModules:
    def __init__(self, module_project: Project):
        self.manifest = module_project.get_modules()
        self.version = module_project.version.id
        self.loaded_modules = {}

    def __getitem__(self, module_index: int) -> RoomModule:
        if module_index not in self.loaded_modules:
            self.loaded_modules[module_index] = self.manifest.load_file(
                module_index,
                lambda path: RoomModule.load_with_metadata(path, self.version),
            ).obj
        return self.loaded_modules[module_index]



type Point = tuple[int, int]

def orient(a: Point, b: Point, c: Point) -> int:
    """2D cross-product (b - a) x (c - a). >0: left turn; <0: right turn; =0: collinear."""
    return (b[0] - a[0])*(c[1] - a[1]) - (b[1] - a[1])*(c[0] - a[0])

def point_in_convex_quad(p: Point, quad: tuple[Point, Point, Point, Point]) -> bool:
    """
    True if point p is inside a convex quadrilateral.
    Quad must be ordered either clockwise or counterclockwise: [A, B, C, D].
    """
    sgn = 0
    n = 4
    for i in range(n):
        a, b = quad[i], quad[(i+1) % n]
        o = orient(a, b, p)
        if o == 0:
            return False
        if sgn == 0:
            sgn = 1 if o > 0 else -1
        elif (o > 0 > sgn) or (o < 0 < sgn):
            return False
    return True  # all on same side (or on edges)


def get_room_name(room_module: RoomModule, room_module_index: int) -> str:
    # use the original unique names for the rooms with duplicate names
    match room_module_index:
        case 13:
            return 'A15RA'
        case 14:
            return 'A15RB'
        case _:
            return room_module.name


project = Project.open(sys.argv[1])
output_dir = Path(sys.argv[2])

maps = project.get_maps()
room_modules = RoomModules(project)
bg_map = {}
camera_images = {}
for stage in Stage:
    map_indexes = STAGE_MAPS[stage]
    bg_manifest = project.get_stage_backgrounds(stage)

    for current_map_index in map_indexes:
        current_map = maps[current_map_index]
        for map_room in current_map.rooms:
            current_room_index = map_room.room_index
            room = room_modules[map_room.module_index]

            room_name = get_room_name(room, map_room.module_index)

            if len(room.entrances) == 0:
                # some rooms have entrances we can't detect automatically, so we have to define them manually
                match room_name:
                    case 'D0001':
                        entrance_set = [
                            Entrance(5, 3018, 0, -2140, 0),
                            Entrance(8, 4920, 0, -8, 0),
                        ]
                    case 'D0002':
                        entrance_set = [
                            Entrance(0, 2748, 0, 16, 0),
                            Entrance(8, -332, 0, 2200, 0),
                        ]
                    case 'D0003':
                        entrance_set = [
                            Entrance(1, -706, 0, 1774, 0),
                            Entrance(8, -2530, 0, -555, 0),
                        ]
                    case 'D0004':
                        entrance_set = [
                            Entrance(2, 0, 0, 0, 0),
                            Entrance(8, -46, 0, 2256, 0),
                        ]
                    case 'D0101':
                        entrance_set = [
                            Entrance(0, 8, 0, -2656, 0),
                            Entrance(1, 0, 0, 0, 0),
                            Entrance(2, 0, 0, 0, 0),
                            Entrance(3, 0, 0, 0, 0),
                        ]
                    case _:
                        print(f"WARNING: no entrances found for {room_name}; skipping")
                        continue
            else:
                entrance_set = room.entrances[0].entrances

            bg_set_index = 0
            if len(room.backgrounds) > 1 and room_name != 'C0102':
                # most rooms in the game only have a single background set, but the rooms in the game where the lights
                # can turn off have two. we generally want the one with the lights on, which is the second set for all
                # except C0102.
                bg_set_index = 1

            # each entrance in the room corresponds to an origin room and a background image
            for entrance_index, entrance in enumerate(entrance_set):
                # first, identify the origin room
                if entrance.room_index < 0:
                    # debug entrance; ignore
                    continue

                origin_map_index = None
                origin_room_index = entrance.room_index
                if room_name == 'A15RC' and origin_room_index == 16:
                    # this seems like it should be the entrance for coming up the stairs behind the shutter, but
                    # neither the room index nor the position match, and there's no other entrance that seems to
                    # correspond to the actual spawn location
                    origin_room_index = 10

                # the entrance doesn't record the origin map, so we'll check the given room index in each map in the
                # stage to see if we can identify the one that links here
                for candidate_map_index in map_indexes:
                    candidate_map = maps[candidate_map_index]
                    if len(candidate_map.rooms) <= origin_room_index:
                        # can't be this map because the room index doesn't exist
                        continue
                    candidate_map_room = candidate_map.rooms[origin_room_index]
                    candidate_room = room_modules[candidate_map_room.module_index]
                    candidate_room_name = get_room_name(candidate_room, candidate_map_room.module_index)

                    for trigger in candidate_room.triggers.triggers:
                        if trigger.trigger_callback not in candidate_room.functions:
                            print(f"WARNING: trigger callback {trigger.trigger_callback:08X} not found in {candidate_room_name}; skipping")
                            continue

                        callback = candidate_room.functions[trigger.trigger_callback]
                        for call in callback.calls:
                            if call.name != 'GoToRoom':
                                continue

                            call_map = call.arguments[1].value
                            call_room = call.arguments[2].value
                            if call_map == current_map_index and call_room == current_room_index:
                                # this is a link to our room, so this is the correct map
                                origin_map_index = candidate_map_index
                                break

                        if origin_map_index is not None:
                            break

                    if origin_map_index is not None:
                        break
                else:
                    # some rooms and/or entrances have weird formats that prevent us from determining the origin map,
                    # so we just have to handle those cases manually
                    match (room_name, origin_room_index):
                        case ('B0112', 11):
                            origin_map_index = Map.HOSPITAL_14F
                            origin_room_index = 4
                        case ('C0101', 10):
                            origin_map_index = Map.YOUR_HOUSE_1F
                        case ('D0001', 5):
                            origin_map_index = Map.HOTEL_1F
                        case _:
                            print(f"WARNING: could not find map for entrance {entrance_index} to {room_name} from room index {origin_room_index}; assuming current map")
                            origin_map_index = current_map_index

                if origin_map_index == current_map_index and origin_room_index == current_room_index:
                    print(f"WARNING: ignoring self-entrance {entrance_index} in {room_name}")
                    continue

                # now that we know the origin room, we need to identify the camera angle corresponding to this entrance
                entrance_pos = (entrance.x, entrance.z)
                # seems like when cuts overlap, the last one wins?
                for cut in reversed(room.layout.cuts):
                    if point_in_convex_quad(entrance_pos, ((cut.x2, cut.z2), (cut.x4, cut.z4), (cut.x3, cut.z3), (cut.x1, cut.z1))):
                        camera_index = cut.index
                        break
                else:
                    print(f"WARNING: could not find camera for entrance {entrance_index} in {room_name}")
                    continue

                if room_name == 'D0003' and origin_room_index == 1:
                    # having trouble with overlapping cuts
                    camera_index = 6
                elif room_name == 'D1001' and origin_room_index in [3, 6]:
                    # you enter this room with a special cutscene camera angle, so we need to override the detected one
                    camera_index = 0
                elif room_name == 'A15RC' and origin_room_index == 10:
                    # entrance data seems bogus as mentioned above
                    camera_index = 7

                # A1310 has two map entries; normalize them to a single one
                if origin_map_index == Map.HOSPITAL_13F and origin_room_index == 10:
                    origin_room_index = 9
                if current_map_index == Map.HOSPITAL_13F and current_room_index == 10:
                    current_room_index = 9

                # with the camera angle, we can now identify the background image
                camera_key = (map_room.module_index, camera_index)
                if camera_key in camera_images:
                    bg_path = camera_images[camera_key]
                else:
                    db_index = 0
                    if (room_name, camera_index) in [('D0001', 5), ('D0002', 6), ('D0003', 5), ('D0004', 5)]:
                        # need to use the second image that has the light turned on
                        db_index = 1

                    bg_description = room.backgrounds[bg_set_index].backgrounds[camera_index]
                    bg_view_manifest = bg_manifest.get_manifest(bg_description.index)
                    bg_tim = bg_view_manifest.load_file(db_index, TimFormat).obj
                    bg_path = output_dir / f"{room_name}_{camera_index}.png"

                    if room_name == 'B0112' and camera_index == 1:
                        # this room has an animated sky texture that we need to underlay behind the main background
                        bg_image = bg_tim.to_image()
                        sky_tim = bg_view_manifest.load_file(2, TimFormat).obj
                        sky_image = sky_tim.to_image()
                        canvas = Image.new('RGBA', bg_image.size, (0, 0, 0, 0))
                        canvas.paste(sky_image, (0, 0), sky_image)
                        canvas.paste(bg_image, (0, 0), bg_image)
                        bg_image = canvas.convert('RGB')
                    else:
                        bg_image = bg_tim.to_image(transparency=Transparency.NONE)

                    if room_name == 'D1004':
                        # the platform in this room is an overlay, so we need to add it in
                        platform_tim = bg_view_manifest.load_file(1, TimFormat).obj
                        platform_image = platform_tim.to_image()
                        bg_image.paste(platform_image, (6, 164), platform_image)

                        # secondly, the unmodified image is only displayed during the lightning flashes; the normal
                        # background is a darkened version of this image. so, we need to darken ours too.
                        enhancer = ImageEnhance.Brightness(bg_image)
                        bg_image = enhancer.enhance(0.2)

                    bg_image.save(bg_path)
                    camera_images[camera_key] = bg_path
                bg_map[(origin_map_index, origin_room_index, current_map_index, current_room_index)] = bg_path.name

json_bg_map = list(bg_map.items())
with (output_dir / "bg_map.json").open('w') as f:
    json.dump(json_bg_map, f)
