//! Tables of the ESHL, SOBJ, ITEM and BOX families (enemy shots, special objects, pickup objects and crates:
//! `families/eshl*`, `sobj`, `items*` and `boxes` of the game): `data/objects.json`.
//!
//! Same form as the tables of the effects ([`crate::effects`]): each table is named, has its address in the program
//! ROM (its key), the space it is read in (data, or program space for the tables that the code reads PC-relative),
//! the size of its elements and its values. The list below gives the reader (`eshl/shot1`: the module of the game),
//! the address and the meaning of each table; the extraction (`extract/src/objects.rs`) reads each table up to its
//! end, the next table or the next code. The game reads a table at a byte offset, like the original; a read past the
//! extracted bytes falls back to the residue (counted, docs/fin.md).

use crate::effects::tables;

tables! {
    /// The tables of the ESHL, SOBJ, ITEM and BOX families, one field per table.
    struct ObjectTables;

    /// `eshl/shot1`: Speed table (16 bytes per variant: VX AX VZ AZ VY AY).
    shot1_speeds: prog 2 @ 0x9dfe2;
    /// `eshl/shot3`: Per variant, 8 bytes: offset (X, Z) from the owner, then speeds (VX, VY).
    shot3_fan: prog 2 @ 0x9e280;
    /// `eshl/shot7`: Per variant, offset (X, Y) from the owner.
    shot7_offsets: prog 2 @ 0x9ed64;
    /// `eshl/shot9`: Limit heights (words) of the rise of the breath, by value of `+0x3e`.
    shot9_rise_limits: prog 2 @ 0x9f14c;
    /// `eshl/shot10`: Placement (Z, X offset); the row is the low word of D5 left by the previous handler (glitch).
    shot10_placement: prog 2 @ 0x9f3c8;
    /// `eshl/shot11`: Per variant, offset (X, Y) from the owner.
    shot11_offsets: prog 2 @ 0x9f524;
    /// `eshl/shot12`: Animation per variant.
    shot12_anims_of_variant: prog 1 @ 0x9f5e0;
    /// `eshl/shot12`: Per variant, offset (X, Z) from the owner.
    shot12_offsets: prog 2 @ 0x9f654;
    /// `eshl/shot13`: Per variant, 8 bytes: offset X, Y, fractional Y, Z from the owner.
    shot13_offsets: prog 2 @ 0x9f776;
    /// `eshl/shot14`: Animation per variant.
    shot14_anims_of_variant: prog 1 @ 0x9f7dc;
    /// `eshl/shot14`: Per variant, 8 bytes: offset X, Y, fractional Y, Z from the owner.
    shot14_offsets: prog 2 @ 0x9fd50;
    /// `eshl/shot14`: Byte table (data space) of the edge shot animations.
    shot14_breath_anims: data 1 @ 0x9fe00;
    /// `eshl_b/shot15`: Speeds (VX VZ AZ AX), one 8-byte row per range (8, 0x10, 0x18).
    shot15_speeds: prog 2 @ 0xa034e;
    /// `eshl_b/shot17`: Spawn offset (Z, X) by `+0x42` (4 bytes per row).
    shot17_spawn_offsets: prog 2 @ 0xa0670;
    /// `eshl_b/shot17`: `+0x2d` according to the angle (Y) for mode 0 and mode > 0.
    shot17_aim_b: prog 1 @ 0xa07ac;
    /// `eshl_b/shot17`: `+0x2d` according to the angle (from 8) for mode < 0.
    shot17_aim_a: prog 1 @ 0xa0892;
    /// `eshl_b/shot17`: Vertical and horizontal speed of mode < 0 (4 bytes per row).
    shot17_drop_speeds: prog 2 @ 0xa0962;
    /// `eshl_b/shot18`: Speed table number (byte) according to A5+0x7f92.
    shot18_speed_tables: prog 1 @ 0xa0a60;
    /// `eshl_b/shot19`: Per angle step, (value of `+0x2d`, animation): 16 bytes facing left, then 16 facing right.
    shot19_steps: data 1 @ 0xa0bda .. 0xa0bfa;
    /// `eshl_b/shot19`: Offsets (X, Z) of the sprite per angle step.
    shot19_offsets: data 2 @ 0xa0bfa;
    /// `eshl_b/shot20`: Wait animation by a 0..7 draw.
    shot20_idle_anims: prog 1 @ 0xa0d60;
    /// `eshl_b/shot20`: Wait (frames) of variant 1 from a 0..7 draw.
    shot20_delays: prog 2 @ 0xa0e2c;
    /// `eshl_b/shot20`: Attachment to the owner per variant (8 bytes: X, integer Y, signed fractional Y, Z).
    shot20_attach: prog 2 @ 0xa0f86;
    /// `eshl_b/shot21`: Attachment to the owner per variant (8 bytes: X, integer Y, signed fractional Y, Z).
    shot21_attach: prog 2 @ 0xa113a;
    /// `eshl_b/shot22`: Offsets (X, Z) according to `+0x2f` (4 bytes per row).
    shot22_offsets: prog 2 @ 0xa1194;
    /// `eshl_b/shot22`: Speeds (VX+AX, VZ+AZ) according to the variant (8 bytes per row).
    shot22_speeds: prog 4 @ 0xa123a;
    /// `eshl_b/shot24`: Offsets (X, Z, Y) of the 16 effects (8 bytes per row).
    shot24_offsets: prog 2 @ 0xa1438;
    /// `eshl_b/shot24`: Number of enemies by (A5+0x7f92 / 2) * 4 + A5+0x35.
    shot24_group_sizes: prog 1 @ 0xa15ae;
    /// `eshl_b/shot24`: Relative positions (X, Y) of the enemies of a group: 2 pairs per row, the row drawn at random.
    shot24_group_offsets: data 2 @ 0xa15ee;
    /// `eshl_creators`: Number of bursts of type 0x19 minus one, per rank / 2 (0xa1670).
    bursts_19_counts: prog 1 @ 0xa16ac;
    /// `eshl_creators`: Number of bursts of type 0x1a minus one, per rank / 2 (0xa17d0).
    bursts_1a_counts: prog 1 @ 0xa184c .. 0xa185c;
    /// `eshl_creators`: Directions (`+0x2d`) of the bursts of type 0x1a: four tables of 0x20 bytes (kind 1, kind 2,
    /// target on the left, on the right), 8 bytes per random draw, read one after the other by address.
    bursts_1a_directions: data 1 @ 0xa185c .. 0xa18dc;
    /// `eshl_b/shot26`: Height per `+0x60` of the source.
    shot26_heights: prog 2 @ 0xa1968;
    /// `eshl_b/shot26`: Speed table number drawn at random (four words).
    shot26_speed_tables: prog 2 @ 0xa1972;
    /// `eshl_b/shot28`: Height per `+0x60 & 0xf` of the source.
    shot28_heights: prog 2 @ 0xa1d18;
    /// `eshl_b/shot28`: Angle per variant (`& 3`).
    shot28_angles: prog 1 @ 0xa1d22;
    /// `eshl_b/shot29`: X shift of the attachment by `+0x33` of the source, for the attacks 0x16 and 0x18.
    shot29_attach_shift: prog 2 @ 0xa1fac;
    /// `eshl_b/shot35`: Direction (`+0x2d`) per variant.
    shot35_directions: prog 1 @ 0xa2934;
    /// `eshl_b/shot36`: Vertical speed drawn at random (16 words).
    shot36_speeds: prog 2 @ 0xa2aa4;
    /// `eshl_b/shot36`: X offset drawn at random (16 words).
    shot36_x_offsets: prog 2 @ 0xa2b0a;
    /// `eshl_b/shot36`: Y offset drawn at random (16 words).
    shot36_y_offsets: prog 2 @ 0xa2b2a;
    /// `eshl_b/shot37`: Vertical speed drawn at random (16 words).
    shot37_speeds: prog 2 @ 0xa2c7e;
    /// `eshl_b/shot37`: Starting animation drawn at random (8 bytes).
    shot37_start_anims: prog 1 @ 0xa2c9e;
    /// `eshl_b/shot37`: X offset drawn at random (16 words).
    shot37_x_offsets: prog 2 @ 0xa2ce0;
    /// `eshl_b/shot37`: Y offset drawn at random (16 words).
    shot37_y_offsets: prog 2 @ 0xa2d00;
    /// `eshl_b/shot38`: Sprite per direction (16 bytes).
    shot38_sprites: prog 1 @ 0xa2e10;
    /// `eshl_b/shot38`: Speed rows per `+0x2f` (16 bytes: speed table, VX, AX, VZ, AZ, VY, AY): 6 rows for variant 0,
    /// then the rows of the other variants.
    shot38_speed_rows: data 2 @ 0xa2ed4;
    /// `eshl_b/shot39`: Direction (`+0x2d`) per variant.
    shot39_directions: prog 1 @ 0xa3148;
    /// `eshl_b/shot39`: Starting animation per variant.
    shot39_start_anims: prog 1 @ 0xa3158;
    /// `eshl_b/shot39`: Sprite frame of the 8-step cycle (`+0x47`); also read as the palettes (`+0x2b`) of shot 40.
    shot39_frames: prog 1 @ 0xa31ba;
    /// `eshl_c/shot40`: Animation drawn at random (16 bytes).
    shot40_random_anims: prog 1 @ 0xa329e;
    /// `eshl_c/shot41`: Z speed (word) per 32-pixel slice of distance.
    shot41_fall_speeds: prog 2 @ 0xa3492;
    /// `eshl_c/shot42`: Animation of the attack according to `+0x33` of the host (byte table indexed by 7 bits).
    shot42_anims_of_host_frame: prog 1 @ 0xa3600;
    /// `eshl_c/shot43`: Animation added per direction (`+0x2d / 2`).
    shot43_anims_of_direction: prog 1 @ 0xa37ba;
    /// `eshl_c/debris45`: Animation of a burst piece, drawn at random (4 bytes).
    debris45_burst_anims: prog 1 @ 0xa3c3a;
    /// `eshl_c/debris45`: Vertical speed of a hop, drawn at random (4 words).
    debris45_hop_vz: prog 2 @ 0xa3c94;
    /// `eshl_c/debris45`: Mirror of `+0x2f` for the flipped object.
    debris45_mirror: prog 1 @ 0xa3d10;
    /// `eshl_c/debris45`: Offsets of variant 1 by `+0x2f`: X, then Z by a read shifted by two bytes and by X.
    debris45_place_offsets: prog 2 @ 0xa3dc2;
    /// `eshl_c/debris45`: Rows of 4 words (VX, AX, VZ, VY) for variant 0 (data space).
    debris45_burst_speeds: data 2 @ 0xa3e30;
    /// `eshl_c/spawn47`: Tables of 16 words of offsets: in X (inside the band / outside the band), in Y (same), delays.
    spawn47_x_inside: prog 2 @ 0xa3fd4;
    /// `eshl_c/spawn47`: X offset drawn when the shot is left of the screen (negated on the right).
    spawn47_x_outside: prog 2 @ 0xa3ff4;
    /// `eshl_c/spawn47`: Y offset drawn when the shot is inside the screen band.
    spawn47_y_inside: prog 2 @ 0xa4014;
    /// `eshl_c/spawn47`: Y offset drawn when the shot is above the screen band (negated below).
    spawn47_y_outside: prog 2 @ 0xa4034;
    /// `eshl_c/spawn47`: Delay drawn at random (16 words).
    spawn47_delays: prog 2 @ 0xa4068;
    /// `eshl_c/bullet50`: Duration of the animation phases (bytes).
    bullet50_phase_lengths: prog 1 @ 0xa44aa;
    /// `eshl_creators`: Offsets (dx, dy) of the three chasers of type 0x33 around their target (0xa44bc).
    chasers_offsets: prog 2 @ 0xa4516 .. 0xa4522;
    /// `eshl_c/finale52`: Sub-types of the three elements launched by variant 0.
    finale52_orb_subtypes: prog 1 @ 0xa46c2;
    /// `eshl_c/finale52`: Animation per sub-type of variant 0 (bytes).
    finale52_anims_of_subtype: prog 1 @ 0xa473e;
    /// `eshl_c/finale52`: Stage 0x2e: pairs (start, length) of the safe zones in X, terminated by 0.
    finale52_safe_ranges: data 2 @ 0xa4bca .. 0xa4bd4;
    /// `eshl_c/finale52`: Stage 0x69: safe rectangles (X, width, Y, height), one per bit of `0xff2064`.
    finale52_safe_rects: data 2 @ 0xa4bd4 .. 0xa4be4;
    /// `eshl_c/finale52`: Per character class, address of its body boxes (collision boxes of sprites/tables.json).
    finale52_player_boxes: data 4 @ 0xa4cba .. 0xa4cd2;
    /// `eshl_c/finale52`: Attack boxes, rows of 16 bytes per `+0x3e` - 1: (offset, half size) in X, Z, Y.
    finale52_attack_boxes: data 2 @ 0xa4cd2;
    /// `eshl_c/aimed53`: For each direction: (animation, orientation).
    aimed53_directions: prog 1 @ 0xa4e16;
    /// `eshl_c/burst54`: Rows of 9 words: DX, DZ, DY, VX, AX, VZ, AZ, VY, AY.
    burst54_rows: prog 2 @ 0xa4fe6;
    /// `eshl_c/burst54`: Rows of 10 words: the same, then the animation.
    burst54_rows_anim: prog 2 @ 0xa51b4;
    /// `eshl_c/pickup56`: Delays (16 words) drawn at random for variants 6 and 7.
    pickup56_delays: prog 2 @ 0xa55b2;
    /// `eshl_c/shard58`: Per variant, 8 bytes: timer (word), sub-type, speed index, animation, flip, speed table
    /// (word).
    shard58_rows: prog 1 @ 0xa5bd6;
    /// `eshl_c/ring59`: Offsets (X, Y) per variant for sub-type 1 (4 bytes per row).
    ring59_offsets: prog 2 @ 0xa5db2;
    /// `eshl_c/ring59`: Value of `+0x2b` (palette (h)) by `+0x47`.
    ring59_palettes: prog 1 @ 0xa5f30;
    /// `eshl_c/platform60`: Starting positions of variant 2 per sub-type (4 bytes per row).
    platform60_start_positions: prog 2 @ 0xa6046;
    /// `eshl_c/marker61`: Offsets (X, Y) per `+0x2f`, 20 bytes per variant, for a creator of variant 0.
    marker61_offsets_0: prog 2 @ 0xa6538;
    /// `eshl_c/marker61`: The same for a creator of variant 1.
    marker61_offsets_1: prog 2 @ 0xa65de;
    /// `eshl_c/marker61`: The same for a creator of variant 2.
    marker61_offsets_2: prog 2 @ 0xa6670;
    /// `eshl_c/meteor62`: Delay before the fall (bytes) per variant (32 entries).
    meteor62_delays: prog 1 @ 0xa679e;
    /// `eshl_c/hand63`: Per number of players, address of 32 delays (words, `hand63_delays`).
    hand63_delay_tables: data 4 @ 0xa6b0e .. 0xa6b22;
    /// `eshl_c/hand63`: The lists of 32 delays (words) designated by `hand63_delay_tables`.
    hand63_delays: data 2 @ 0xa6b22;
    /// `eshl_c/hand63`: Per number of players, address of 32-bit masks indexed by the rank (`hand63_gates`).
    hand63_gate_tables: data 4 @ 0xa6c54 .. 0xa6c68;
    /// `eshl_c/hand63`: The masks designated by `hand63_gate_tables`.
    hand63_gates: data 4 @ 0xa6c68;
    /// `eshl_d/rocks65`: Waits per rank (bytes) of variant 0.
    rocks65_delays_0: prog 1 @ 0xa739a;
    /// `eshl_d/rocks65`: Waits per rank (bytes) of variant 1.
    rocks65_delays_1: prog 1 @ 0xa74b6;
    /// `eshl_creators`: Number of fragments of type 0x42 per variant (0xa7570).
    fragments_42_counts: prog 1 @ 0xa75b6;
    /// `eshl_d/shard66`: Animation per variant, at creation then at impact (bytes, program ROM).
    shard66_start_anims: prog 1 @ 0xa760e;
    /// `eshl_d/shard66`: Landing animation per variant.
    shard66_land_anims: prog 1 @ 0xa777a;
    /// `eshl_d/shard66`: Offsets (X, depth) per rank (four bytes), one table per variant: variants 0 and 1 at 0, then
    /// 0x20, 0x3c, 0x5c, 0x74.
    shard66_offsets: data 2 @ 0xa7834;
    /// `eshl_creators`: Bursts of boss 11 (0xa7b0a): pairs (number, absent one) drawn by an even random index.
    boss11_shard_counts: prog 1 @ 0xa7b64 .. 0xa7b74;
    /// `eshl_d/stone68`: Abscissa per rank (words, program ROM), direction per rank (bytes, program ROM).
    stone68_columns: prog 2 @ 0xa7be4;
    /// `eshl_d/stone68`: Direction per rank (`& 7`).
    stone68_directions: prog 1 @ 0xa7c7c;
    /// `eshl_d/stone68`: Animations according to the height on the screen: count then pairs (animation, threshold)
    /// (words, in the clear).
    stone68_height_anims: data 2 @ 0xa7d50;
    /// `eshl_creators`: Number of fragments of type 0x45 of each row (0xa7da2).
    fragments_45_count: prog 1 @ 0xa7d92 .. 0xa7d94;
    /// `eshl_d/bouncer69`: Per rank: wait then animation (words, program ROM).
    bouncer69_ranks: prog 2 @ 0xa7eb4;
    /// `eshl_d/splinter72`: Per rank, address of its sheets (`splinter72_sheets`).
    splinter72_rows: data 4 @ 0xa84f4 .. 0xa850c;
    /// `eshl_d/splinter72`: Sheets of 18 bytes (offsets X, Y, Z, then the speeds), one per variant from the address of
    /// the rank.
    splinter72_sheets: data 2 @ 0xa850c;
    /// `boxes/chest`: Per loot number, (type, variant) of the created ITEM (also in data/boxes.json, `loot`).
    chest_loots: data 1 @ 0xda28a .. 0xda398;
    /// `boxes/chest`: Per class of the carrying player, address of the offsets (X, Z) of the carried crate per pose
    /// (`chest_taken_offsets`).
    chest_taken: data 4 @ 0xda398 .. 0xda3c8;
    /// `boxes/chest`: Offsets (X, Z) of the carried crate per pose, designated by `chest_taken`.
    chest_taken_offsets: data 2 @ 0xda3c8;
    /// `boxes`: Barrier: position given to its SET per mode (8 bytes) and class of the hitter.
    barrier_positions: prog 1 @ 0xda72c;
    /// `boxes/pushes`: Offset of the player against the pushed block, per character (words), one row of 16 bytes per
    /// push direction: high, low, left and right (variant 1), left and right (variant 0).
    pushes_against: data 2 @ 0xdab52 .. 0xdabb2;
    /// `items/treasure`: Parameters per variant (8 bytes): read in program space.
    item_treasure_params: prog 1 @ 0xdadb6;
    /// `items/consumable`: Per variant, (sparkle `+0x54`, pickup index `+0x5d`).
    item_consumable_params: prog 1 @ 0xdaedc;
    /// `items_b`: Per character, non-zero: the piece of slot 4 is forbidden to it.
    equipment_b_slot_4_forbidden: prog 1 @ 0xdb482;
    /// `items_b`: Sheet of each variant of the equipment of type 2 (8 bytes: slot, identifier, sparkle, hit counter,
    /// collection index, announce).
    equipment_b_sheets: data 1 @ 0xdb522;
    /// `items`: Equipment that drops when a player is hit: a mask per value (0 to 5) of the hit counter left, for the
    /// special weapon, the shield and the ring (also in data/boxes.json, `wear_chances`); the piece falls if the bit
    /// `rng & 15` of the mask is set.
    wear_chances: data 2 @ 0xdb7b8 .. 0xdb7dc;
    /// `items/equipment`: Per class, 8 identifiers that the first slot cannot replace.
    item_equipment_locked: data 1 @ 0xdbc5c;
    /// `items/equipment`: Sheets (data); bytes 0 to 5: authorized classes, 6: identifier, 7: group.
    item_equipment_sheets: data 1 @ 0xdbc8c;
    /// `items/equipment`: Pickup index per variant.
    item_equipment_indices: data 1 @ 0xdbdcc;
    /// `items/inventory`: Sparkle per variant; pickup index per variant.
    item_inventory_shimmers: data 1 @ 0xdbf04;
    /// `items/inventory`: Index in the collectible objects per variant.
    item_inventory_indices: data 1 @ 0xdbf16;
    /// `items/inventory`: Per player class, 8 identifiers of objects that cannot be replaced.
    item_inventory_locked: data 1 @ 0xdc1ca;
    /// `items/inventory`: Per variant, 8 bytes: object identifier then one prohibition byte per class.
    item_inventory_sheets: data 1 @ 0xdc1fa;
    /// `items/inventory`: Per object identifier: (type, variant) of the ITEM recreated when it is replaced.
    item_inventory_recreated: prog 1 @ 0xdc2d6;
    /// `items/bonus`: Per mode: vx, vz, az, vy (words), read in program space.
    item_bonus_speeds: prog 2 @ 0xdc42c;
    /// `items/scene`: Icon: animation per mode drawn at random (4 bytes).
    item_scene_icon_animations: prog 1 @ 0xdc69c;
    /// `items/scene`: Icon: sound (envelope) per mode, two sets of four longs.
    item_scene_icon_sounds: prog 4 @ 0xdc6b2;
    /// `items/scene`: Icon leaving the scene: per mode, sound (envelope) played at departure (longs, 0 = none).
    item_scene_start_routines: prog 4 @ 0xdca50 .. 0xdca60;
    /// `items/scene`: Icon leaving the scene: departure animation per mode.
    item_scene_start_animations: prog 1 @ 0xdca60;
    /// `items/scene`: Icon leaving the scene: stop animation per mode.
    item_scene_stop_animations: prog 1 @ 0xdca9c .. 0xdcaa0;
    /// `items/scene`: Icon leaving the scene: per mode, non-zero to play the stop sound.
    item_scene_stop_sounds: prog 1 @ 0xdcaa0;
    /// `items/scene`: Guide: (animation, mode) per level.
    item_scene_guide_animations: prog 1 @ 0xdcb6e;
    /// `items/scene`: Guide: arrival animation per mode.
    item_scene_guide_animations_arrival: prog 1 @ 0xdcbbc;
    /// `items/scene`: Guide 3: its 16-color palette, copied to the palette page 0x91c300.
    item_scene_guide_3_palette: data 4 @ 0xdccec .. 0xdcd0c;
    /// `items/scene`: X offset from the player, facing right then left.
    item_scene_offsets_x: prog 2 @ 0xdcdd8;
    /// `items/reserve`: Per character (program space): is the object usable?
    item_reserve_characters: prog 1 @ 0xdd01e;
    /// `items/reserve`: Per class (data): reserve accepted?
    item_reserve_classes: data 1 @ 0xdd122;
    /// `items/contact`: Variant 0: starting animation per mode.
    item_contact_v0_animations: prog 1 @ 0xdd180;
    /// `items/contact`: Variant 0: animation by mode (`+0x2f`) when the key is used.
    item_contact_v0_animations_key: prog 1 @ 0xdd1d2;
    /// `items/contact`: Variant 0: animation by mode when its SET is created.
    item_contact_v0_animations_set: prog 1 @ 0xdd22a;
    /// `items/contact`: Variant 0: animation by mode at the end.
    item_contact_v0_animations_end: prog 1 @ 0xdd246;
    /// `items/contact`: Per class of the player, address of its body boxes (collision boxes of sprites/tables.json).
    item_contact_boxes_player: data 4 @ 0xdd578 .. 0xdd590;
    /// `items/contact`: Contact boxes: one per type (`+0x39`) for the object, one per character and per `+0x38` for the
    /// player.
    item_contact_boxes_object: data 1 @ 0xdd590;
    /// `items/escort`: Palette per variant.
    item_escort_palettes: prog 1 @ 0xdd5de;
    /// `sobj/scenery`: Per variant, the initialization routine (13 routines).
    sobj_scenery_init_kind: prog 1 @ 0xdd6a4;
    /// `sobj/scenery`: Variant -> follow routine (23 routines).
    sobj_scenery_step_kind: prog 1 @ 0xdd8ea;
    /// `sobj/fixture`: Variant -> shape of the behavior at creation (0: none, 1: saves X/Y).
    sobj_fixture_init_kind: prog 1 @ 0xddc6e;
    /// `sobj/fixture`: Variant -> shape of the per-frame behavior (0: screen exit, 1: scrolling, 2: following).
    sobj_fixture_step_kind: prog 1 @ 0xddd16;
    /// `sobj/scene`: Variant 0: per value of `+0x2f`, 8 bytes: animation set (long), flip indicator, number (word at
    /// 6).
    sobj_scene_v0_entries: prog 4 @ 0xddf6c;
    /// `sobj/scene`: Variant 3: lists of decorative SET objects (one byte per object, 0 ends): `A5+0x38` zero, then
    /// non-zero (offset 8).
    sobj_scene_v3_lists: data 1 @ 0xde1e0 .. 0xde1f0;
    /// `sobj/scene`: Variant 5: offsets (X, Z) of the variants 0 it creates (two at most).
    sobj_scene_v5_offsets: prog 2 @ 0xde386;
    /// `sobj/scene`: Variant 6: per variant of the letter, (X, stop height).
    sobj_scene_v6_table: prog 2 @ 0xde3ee;
    /// `sobj/scene`: Variant 7: addresses of the scene scripts by `+0x2f`, then the same for two players' cabinets.
    sobj_scene_v7_scripts: prog 4 @ 0xde4d2 .. 0xde4e2;
    /// `sobj/scene`: Variant 7: the scene scripts (two header words copied to `A5+0x5ca` and `A5+0x5cc`; the rest is
    /// read by the scene script from `A5+0x5c6`).
    sobj_scene_v7_texts: data 2 @ 0xde4e2;
    /// `sobj/scene/later`: Variant 14: badge, 8 bytes per number (`+0x46`, or the variant of the object it designates):
    /// X, Z, sprite.
    sobj_scene_v14_table: prog 2 @ 0xdef1e;
    /// `sobj/scene/later`: Variant 15: row of the pennant palette (from 0x28f960) per value of `+0x2f`.
    sobj_scene_v15_palette_select: prog 1 @ 0xdefa4;
    /// `sobj/scene/later`: Variant 15: position (X, Z) per value of `+0x2f`.
    sobj_scene_v15_positions: prog 2 @ 0xdf01a;
    /// `sobj/scene/later`: Variant 19: per value of `+0x2f`, (row of the palette from 0x28f3a0, flip).
    sobj_scene_v19_lines: prog 1 @ 0xdf182;
    /// `sobj/scene/later`: Variant 19: position (X, Z) per value of `+0x2f`.
    sobj_scene_v19_positions: prog 2 @ 0xdf234;
    /// `sobj/scene/later`: Variants 20 and 22: abscissa drawn at random (8 words).
    sobj_scene_v20_x: prog 2 @ 0xdf2a8;
    /// `sobj/scene/later`: Variants 20 and 22: (vertical acceleration, animation) drawn at random (4 pairs).
    sobj_scene_v20_fall: prog 2 @ 0xdf2ea;
    /// `sobj/scene/later`: Variant 21: row of the pennant palette (from 0x28f960) per value of `+0x2f / 2`.
    sobj_scene_v21_palette_select: prog 1 @ 0xdf332;
    /// `sobj/scene/later`: Variant 21: position (X, Z) per value of `+0x2f`.
    sobj_scene_v21_positions: prog 2 @ 0xdf396;
    /// `sobj/frame`: Animation per variant (words).
    sobj_frame_anims_by_variant: prog 2 @ 0xdf80a;
    /// `sobj/frame`: Offset (X, Z) per variant.
    sobj_frame_offsets: prog 2 @ 0xdf87a;
    /// `sobj/arrow`: Animation per advice (words, from advice 0x118).
    sobj_arrow_anims_by_advice: prog 2 @ 0xdf9ae;
    /// `sobj/arrow`: Event code per advice (words, from advice 0x118).
    sobj_arrow_events_by_advice: prog 2 @ 0xdfa0e;
    /// `sobj/stage_title`: Animation of the title per stage (words).
    sobj_stage_title_anims_by_stage: prog 2 @ 0xdff0c;
    /// `sobj/target`: Rectangles of the target (4 words), one per variant group: variant 0x25, the others.
    sobj_target_rects: data 2 @ 0xe02c6 .. 0xe02d6;
    /// `boxes/chest`: Per kind of crate (`+3`), address of its 0x2a-byte sheet (`chest_sheet_data`; also in
    /// data/boxes.json, `sheets`).
    chest_sheets: data 4 @ 0x1051ae .. 0x105396;
    /// `boxes/chest`: The sheets of the crates (0xda082): 10 bytes copied from `+0x60`, 16 random loots, 16 random
    /// special effects (also in data/boxes.json, `sheets/a` and `sheets/b`).
    chest_sheet_data: data 1 @ 0x105396 .. 0x1067a2;
    /// `eshl/shot9`: Frame lists of the breath stuck to an enemy: two arrays of 7 offsets (words, relative to their
    /// array) by height, while it burns (offset 0) and at its end (offset 0xe), then the lists of (frame, frame data)
    /// longs, one pair per value of `+0x3e`.
    shot9_frames: data 2 @ 0x16d676 .. 0x16da84;
    /// `eshl_b/shot34`: Four palettes of 8 longs copied to the palette pages (excerpt of the palette library,
    /// palettes.png).
    shot34_palettes: data 4 @ 0x2538c0 .. 0x253940;
    /// `eshl_b/shot17`: Palette of 8 longs copied to 0x91c300 (excerpt of the palette library, palettes.png).
    shot17_palette: data 4 @ 0x25a660 .. 0x25a680;
    /// `eshl_c/shot41`: Palette copied (ROM to palette RAM): 8 longs.
    shot41_palette: data 4 @ 0x25a6e0 .. 0x25a700;
    /// `sobj`: Palettes copied by the special objects (excerpt of the palette library, palettes.png): scene elements,
    /// pennants, stage titles and their fades, hint and panel.
    sobj_palettes: data 4 @ 0x28f340 .. 0x292260;
}
