//! Tables of the players (`player/`, `combat/` and the player shots and effects `families/player/` of the game):
//! `data/player_tables.json`.
//!
//! Same form as the tables of the effects ([`crate::effects`]): each table is named, has its address in the program
//! ROM (its key), the space it is read in (data, or program space for the tables that the code reads PC-relative),
//! the size of its elements and its values. The list below gives the reader (`hurt`: the module of the game under
//! `player/`, `combat/` or `families/player/`), the address and the meaning of each table; the extraction
//! (`extract/src/player_tables.rs`) reads each table up to its end, the next table or the next code. The tables that
//! `data/players.json` and `data/combat.json` already describe are read there. The game reads a table at a byte
//! offset, like the original; a read past the extracted bytes falls back to the residue (counted, docs/fin.md).

use crate::effects::tables;

tables! {
    /// The tables of the players, one field per table.
    struct PlayerTables;

    /// `combat/damage`: 0x3936 / 0x3954 (PC): thief's loot: probability per level, draw masks.
    damage_loot_level: prog 1 @ 0x3936;
    /// `combat/damage`: Damage loot mask (h).
    damage_loot_mask: prog 2 @ 0x3954;
    /// `combat/damage`: 0x3be4 (PC): random variation added to the damage, signed bytes, 32 per row (row = min(base damage - 1, 4)).
    damage_roll: prog 1 @ 0x3be4;
    /// `combat/damage`: 0x3c84 (PC): for a critical hit, according to (low HP, 4-bit random): row number of 0x3ca2.
    damage_crit_row: prog 1 @ 0x3c84;
    /// `combat/damage`: 0x3ca2 (PC): 16-bit masks; the critical hit happens if the drawn bit (4-bit random) is 1.
    damage_crit_mask: prog 2 @ 0x3ca2;
    /// `combat/damage`: 0x3e4e and 0x3ef8 (PC): tables of the magician's critical hit (class 4).
    damage_mage_row: prog 1 @ 0x3e4e;
    /// `combat/damage`: Damage mage shift (h).
    damage_mage_shift: prog 1 @ 0x3ef8;
    /// `combat/damage`: 0x406d (PC): number of 1/32 slices removed according to protection (`+0x108`).
    damage_protection: prog 1 @ 0x406d;
    /// `update`: Predefined names (in clear): per character (8 bytes, second series starting at 12 for version `A5+0x71`), or designated by a negative first byte.
    score_names: data 2 @ 0x1829e;
    /// `scripted`: Cinematics by scene: one routine address (long) per scene number.
    scripted_by_scene: prog 4 @ 0x23826;
    /// `scripted`: Waiting for the controls: routine addresses, indexed by `+6`.
    scripted_wait_steps: prog 4 @ 0x23d66;
    /// `scripted`: Scripted wait delays (h).
    scripted_wait_delays: prog 2 @ 0x23dac;
    /// `scripted`: End cinematics by scene.
    scripted_end_by_scene: prog 4 @ 0x23f4e;
    /// `scripted`: Scripted scene1 wait (h).
    scripted_scene1_wait: prog 2 @ 0x24f38;
    /// `moves/climb`: Attack animation from the climb, per character (second table).
    climb_attacks: prog 1 @ 0x35eee;
    /// `moves/climb`: Exit palette slots (h).
    exit_palette_slots: data 2 @ 0x36626;
    /// `checks`: Words (code, 0x37784): for each 4-bit draw, the mask of the draws that trigger the effect.
    curse_masks: prog 2 @ 0x37784;
    /// `effects`: Offset (code words, 0x378aa) of each character's palette block in the palette RAM and in the source blocks.
    aura_palette_offsets: prog 2 @ 0x378aa;
    /// `overlays`: Animation scripts by weapon number (pointers).
    overlay_animations: data 4 @ 0x396e8;
    /// `moves/leaps`: Counterattack after a parry: attack sub-action per class (code bytes).
    riposte_kinds: prog 1 @ 0x3a0a2;
    /// `hurt`: Reaction animations (sets of long entries per character, read as data).
    hurt_animations: data 4 @ 0x3bb52;
    /// `elf/spells`: Bytes (code): variant number of the gesture of spell 0x18 per level.
    elf_spark_gestures: prog 1 @ 0x43476;
    /// `magic_user/spells`: Bytes (code): variant number of the gesture of spell 0x18 per level.
    magic_user_spark_gestures: prog 1 @ 0x46a46;
    /// `peff_ring`: Ring0 initial angle (h).
    ring0_initial_angle: prog 1 @ 0x49f0e;
    /// `peff_ring`: Ring0 next angle (h).
    ring0_next_angle: prog 1 @ 0x4a172;
    /// `peff_ring`: Ring1 angle c (h).
    ring1_angle_c: prog 1 @ 0x4a28c;
    /// `peff_ring`: Ring1 angle b (h).
    ring1_angle_b: prog 1 @ 0x4a290;
    /// `peff_ring`: Ring1 angle a (h).
    ring1_angle_a: prog 1 @ 0x4a296;
    /// `peff_ring`: Ring1 next c (h).
    ring1_next_c: prog 1 @ 0x4a6ee;
    /// `peff_ring`: Ring1 next b (h).
    ring1_next_b: prog 1 @ 0x4a6f2;
    /// `peff_ring`: Ring1 next a (h).
    ring1_next_a: prog 1 @ 0x4a6f8;
    /// `peff_ring`: Ring1 next2 b (h).
    ring1_next2_b: prog 1 @ 0x4a7ae;
    /// `peff_ring`: Ring1 next2 a (h).
    ring1_next2_a: prog 1 @ 0x4a7b4;
    /// `peff_c`: Cure spark vz (h).
    cure_spark_vz: prog 2 @ 0x4adc8;
    /// `peff_c`: Cure spark times (h).
    cure_spark_times: prog 1 @ 0x4add8;
    /// `peff_c`: Holy script by player (h).
    holy_script_by_player: prog 1 @ 0x4b0b2;
    /// `peff_c`: Holy scripts (h).
    holy_scripts: data 4 @ 0x4b0da;
    /// `peff_c`: Holy shake table (h).
    holy_shake_table: prog 4 @ 0x4b172;
    /// `peff_d`: Ray shot counts (h).
    ray_shot_counts: prog 1 @ 0x4b75a;
    /// `peff_b`: Hail speed tables (h).
    hail_speed_tables: prog 4 @ 0x4baf6;
    /// `peff_e`: Ice delays (h).
    ice_delays: prog 1 @ 0x4bd98;
    /// `peff_e`: The code bytes around `ice_delays` that its index reaches (original glitch: the high byte of the
    /// random draw enters the index, -0x8000..0x7f0f).
    ice_delays_reach: prog 1 @ 0x43d98 .. 0x53ca8;
    /// `peff_e`: Bounce angle bytes (up, down, left, right...).
    ice_bounce_left: data 1 @ 0x4be70;
    /// `peff_e`: Ice bounce right (h).
    ice_bounce_right: data 1 @ 0x4be78;
    /// `peff_e`: Ice bounce top (h).
    ice_bounce_top: data 1 @ 0x4be80;
    /// `peff_e`: Ice bounce bottom (h).
    ice_bounce_bottom: data 1 @ 0x4be88;
    /// `peff_e`: Ice scatter x (h).
    ice_scatter_x: prog 2 @ 0x4c016;
    /// `peff_e`: Ice scatter y (h).
    ice_scatter_y: prog 2 @ 0x4c036;
    /// `peff_e`: Ice spread (h).
    ice_spread: prog 2 @ 0x4c0ae;
    /// `peff_e`: Banish alt bank (h).
    banish_alt_bank: prog 1 @ 0x4c3a4;
    /// `peff_e`: Banish frames a (h).
    banish_frames_a: prog 2 @ 0x4c3a8;
    /// `peff_e`: Banish frames b (h).
    banish_frames_b: prog 2 @ 0x4c3d6;
    /// `peff_f`: Trail colors (h).
    trail_colors: prog 1 @ 0x4c66e;
    /// `peff_f`: Trail poses (h).
    trail_poses: data 4 @ 0x4c6de;
    /// `peff_f`: Bubble offsets (h).
    bubble_offsets: prog 2 @ 0x4c9e0;
    /// `peff_f`: Flash flips (h).
    flash_flips: prog 1 @ 0x4ceae;
    /// `peff_f`: Flash corners (h).
    flash_corners: prog 2 @ 0x4ceb2;
    /// `peff_f`: Flash delays (h).
    flash_delays: prog 2 @ 0x4cec2;
    /// `peff_b`: Witch spread (h).
    witch_spread: prog 2 @ 0x4d1bc;
    /// `peff_g`: Prayer durations (h).
    prayer_durations: prog 2 @ 0x4d45e;
    /// `peff_g`: Prayer angles (h).
    prayer_angles: prog 1 @ 0x4d5e6;
    /// `peff_o`: Per-character tables (pointers) giving, for each piece of equipment, 0 (nothing), 1 (sold `+0x148` times), ≥ 2 (sold once) or negative (exchanged for nothing).
    sell_tables: prog 4 @ 0x4d884;
    /// `peff_h`: Ring offsets (h).
    ring_offsets: prog 2 @ 0x4e412;
    /// `peff_h`: Moon offsets (h).
    moon_offsets: prog 2 @ 0x4e578;
    /// `peff_i`: Storm offsets a (h).
    storm_offsets_a: prog 2 @ 0x4e8ee;
    /// `peff_i`: Storm offsets b (h).
    storm_offsets_b: prog 2 @ 0x4eae6;
    /// `peff_i`: Whirl offsets (h).
    whirl_offsets: prog 2 @ 0x4ed06;
    /// `peff_i`: Whirl flips (h).
    whirl_flips: prog 1 @ 0x4eda6;
    /// `peff_i`: Whirl spawn table (h).
    whirl_spawn_table: prog 1 @ 0x4eebe;
    /// `peff_i`: Whirl power (h).
    whirl_power: prog 1 @ 0x4efea;
    /// `peff_i`: Target offsets (h).
    target_offsets: prog 2 @ 0x4f0b0;
    /// `peff_b`: Spark throws (h).
    spark_throws: prog 2 @ 0x4f1f6;
    /// `peff_m`: Offset (X, Z) per step of the caster's animation.
    peff_m_offsets: prog 2 @ 0x4f3ea;
    /// `peff_j`: Chip offsets (h).
    chip_offsets: prog 2 @ 0x4ff9a;
    /// `peff_n`: Peff n offsets (h).
    peff_n_offsets: prog 2 @ 0x5043a;
    /// `peff_shop`: Item messages (h).
    item_messages: prog 2 @ 0x50c78;
    /// `peff_shop`: Item prices (h).
    item_prices: prog 4 @ 0x50c90;
    /// `peff_shop`: Item animations (h).
    item_animations: data 2 @ 0x50cc0;
    /// `peff_shop`: Item purchases (h).
    item_purchases: data 2 @ 0x50cd8;
    /// `peff_shop`: Sales zone (four words, program space) and per-variant tables.
    item_zone_sizes: prog 2 @ 0x50cf0;
    /// `peff`: Per scene: dialogue script of each zone (longs, program ROM).
    shop_scripts_a: prog 4 @ 0x51106;
    /// `peff`: Shop scripts b (h).
    shop_scripts_b: prog 4 @ 0x5113e;
    /// `peff`: Column offsets per variant (words, program ROM).
    badge_columns: prog 2 @ 0x5184e;
    /// `peff_k`: Lamp offsets x (h).
    lamp_offsets_x: prog 2 @ 0x51982;
    /// `peff_k`: Lamp offsets z (h).
    lamp_offsets_z: prog 2 @ 0x51996;
    /// `peff_j`: Rain offsets (h).
    rain_offsets: prog 2 @ 0x51b44;
    /// `peff_j`: Rain delays (h).
    rain_delays: prog 2 @ 0x51ba8;
    /// `pshl`: Thrown offsets (h).
    thrown_offsets: data 2 @ 0x51e16;
    /// `pshl`: Flung offsets (h).
    flung_offsets: data 2 @ 0x520f6;
    /// `pshl`: Arc offsets (h).
    arc_offsets: data 2 @ 0x522c0;
    /// `pshl`: Bouncer offsets (h).
    bouncer_offsets: data 2 @ 0x52464;
    /// `pshl`: Burst shot counts (h).
    burst_shot_counts: prog 1 @ 0x5263c;
    /// `pshl`: Burst offset tables (h).
    burst_offset_tables: data 4 @ 0x52732;
    /// `pshl_e`: Orbit offsets (h).
    orbit_offsets: prog 2 @ 0x52802;
    /// `pshl_e`: Orbit delays (h).
    orbit_delays: prog 1 @ 0x528c8;
    /// `pshl_e`: Orbit power a (h).
    orbit_power_a: prog 1 @ 0x528d0;
    /// `pshl_e`: Orbit power b (h).
    orbit_power_b: prog 1 @ 0x528e8;
    /// `pshl_e`: Orbit target angles (h).
    orbit_target_angles: prog 1 @ 0x52a76;
    /// `pshl_e`: Orbit turn steps (h).
    orbit_turn_steps: prog 1 @ 0x52aae;
    /// `pshl_b`: Ground pose a (h).
    ground_pose_a: prog 2 @ 0x52d6a;
    /// `pshl_b`: Ground pose b (h).
    ground_pose_b: prog 2 @ 0x52dd8;
    /// `pshl_b`: Ground colors (h).
    ground_colors: prog 1 @ 0x52e1a;
    /// `pshl_b`: Shadow pick (h).
    shadow_pick: prog 1 @ 0x52ed4;
    /// `pshl_b`: Shadow angles (h).
    shadow_angles: prog 1 @ 0x52eda;
    /// `pshl_c`: Jet offsets (h).
    jet_offsets: data 2 @ 0x53150;
    /// `pshl_c`: Item offsets (h).
    item_offsets: data 2 @ 0x53314;
    /// `pshl_c`: Volley offsets (h).
    volley_offsets: prog 2 @ 0x53580;
    /// `pshl_c`: Volley lifetimes (h).
    volley_lifetimes: prog 1 @ 0x535a4;
    /// `pshl_c`: Star colors (h).
    star_colors: prog 1 @ 0x53734;
    /// `spawn`: Spawn slots (h).
    spawn_slots: prog 1 @ 0x5394e;
    /// `pshl_e`: Fountain turn (h).
    fountain_turn: prog 1 @ 0x53c7c;
    /// `pshl_d`: Wall pick (h).
    wall_pick: prog 1 @ 0x5438e;
    /// `pshl_d`: Fireball offsets (h).
    fireball_offsets: prog 2 @ 0x5460e;
    /// `pshl_d`: Bolt offsets (h).
    bolt_offsets: prog 2 @ 0x547ec;
    /// `init`: Init base level (h).
    init_base_level: prog 1 @ 0xcf17c;
    /// `init`: Init minima (h).
    init_minima: prog 4 @ 0xcf1b0;
    /// `hit_reactions`: By attacker class (address, in the code): table of stagger chances according to the hit.
    hit_reactions_stun_class: prog 4 @ 0xf1cac;
    /// `hit_reactions`: 16-bit masks: a draw of 0..15 succeeds if its bit is set.
    hit_reactions_stun_masks: data 2 @ 0xf1d2e .. 0xf1f2e;
    /// `hit_reactions`: Stagger durations.
    hit_reactions_stun_times: data 1 @ 0xf1f2e .. 0xf24fe;
    /// `hit_reactions`: Stun thresholds: 8 bytes per victim type, one per eighth of the random counter.
    hit_reactions_threshold_enemy: data 1 @ 0xf24fe;
    /// `hit_reactions`: Second table, indexed by the threshold of the first, for victims that are not enemies.
    hit_reactions_threshold_other: data 1 @ 0xf25ae;

    // ---- tables designated by their address (passed to a common reader, or read from another table) ----
    /// `characters/elf`: Ward duration by level (words).
    elf_ward_durations: prog 2 @ 0x43f70;
    /// `characters/elf`: Frost duration by level (words).
    elf_frost_durations: prog 2 @ 0x436c8;
    /// `characters/elf`: Quake duration by level (words).
    elf_quake_durations: prog 2 @ 0x4395a;
    /// `characters/magic_user`: Curse duration by level (words).
    magic_user_curse_durations: prog 2 @ 0x47c4e;
    /// `characters/magic_user`: Portal duration by level (words).
    magic_user_portal_durations: prog 2 @ 0x47ee8;
    /// `characters/magic_user`: Quake duration by level (words).
    magic_user_quake_durations: prog 2 @ 0x46fd0;
    /// `combos`: Input sequence of the recognizer `+0x1a0` (dash): pairs (mask, expected value), then a word, -1 at
    /// the end (code bytes read as data).
    combo_sequences: data 1 @ 0xf193a .. 0xf19c2;
    /// `hit_reactions`: By attacker class (pointers) then victim type (pointers): reaction rows, four bytes per hit
    /// identifier (normal reaction, airborne, stunned, addition to the counter); ordinary enemies.
    reaction_rows_enemy: data 4 @ 0xf2656;
    /// `hit_reactions`: Same, for the victims that are not ordinary enemies (bosses...).
    reaction_rows_other: data 4 @ 0xf266e;
    /// `hit_reactions`: The per-class pointer tables and the reaction rows they designate.
    reaction_rows: data 1 @ 0xf2686 .. 0x101166;
    /// `hurt`: Reactions of the hit state: one offset (from the table) per action `+6`.
    hurt_actions: prog 2 @ 0x3a0f4;
    /// `hurt`: Reactions of the downed state: one offset per action `+6`.
    down_actions: prog 2 @ 0x3b820;
    /// `hurt`: Step table of reaction 6 (offsets from the table, by step `+7`).
    hurt_steps_6: prog 2 @ 0x3a4d2;
    /// `hurt`: Step table of reaction 0x14.
    hurt_steps_14: prog 2 @ 0x3a858;
    /// `hurt`: Step table of reaction 0x26.
    hurt_steps_26: prog 2 @ 0x3aa9e;
    /// `hurt`: Step table of reaction 0x32.
    hurt_steps_32: prog 2 @ 0x3ad3a;
    /// `hurt`: Step table of downed reaction 6.
    down_steps_6: prog 2 @ 0x3b86e;
    /// `hurt`: Step table of downed reaction 0xa.
    down_steps_a: prog 2 @ 0x3b890;
    /// `moves/climb`: Palettes of the exit fade (three stages of 0x180 bytes, 0x20 per character).
    exit_palettes: data 2 @ 0x3663e .. 0x36abe;
    /// `moves/poses`: Dust puffs of the rare poses: 8 bytes per row (variant, setting, vx, height).
    pose_puffs: prog 2 @ 0x36d2c;
    /// `effects`: Aura palettes (second counter), 0x20 bytes per palette.
    aura_palettes_b: data 2 @ 0x378c2 .. 0x384c2;
    /// `effects`: Aura palettes (third counter).
    aura_palettes_c: data 2 @ 0x384c2 .. 0x390c2;
    /// `effects`: Normal palettes of the players, restored at the end of an aura.
    normal_palettes: data 2 @ 0x390c2 .. 0x395d4;
    /// `overlays`: Weapon frames held by the body, per character (pointers), 8 bytes per frame.
    overlay_body: data 4 @ 0x395d4;
    /// `overlays`: Raised shield frames, per character (pointers).
    overlay_shield_high: data 4 @ 0x39604;
    /// `overlays`: Frames designated by the per-character pointers of `overlay_body` and `overlay_shield_high`
    /// (8 bytes per frame: animation, dx, dz, flip, behind).
    overlay_frames: data 2 @ 0x1147ce .. 0x11bffa;
    /// `overlays`: Palettes of the power-up sparkle, 0x20 bytes per step.
    power_palettes: data 2 @ 0x397e0 .. 0x398e0;
    /// `scripted`: Steps of the revival.
    revive_steps: prog 2 @ 0x23b98;
    /// `scripted`: Steps of the walk-in of scene 1.
    scene1_steps: prog 2 @ 0x24eae;
    /// `scripted`: Steps of the walk-in.
    walk_in_steps: prog 2 @ 0x23a00;
    /// `scripted`: Sub-steps of the wait for the controls.
    wait_substeps: prog 2 @ 0x23d7a;
    /// `scripted`: Wait of the walk-in, per player (words).
    walk_in_wait: prog 2 @ 0x23a56;
    /// `scripted`: Sub-steps of the throw by a boss.
    thrown_substeps: prog 2 @ 0x23e0e;
    /// `scripted`: Rows of the throw by a boss, per player.
    thrown_rows: prog 2 @ 0x23e58;
    /// `scripted`: Offsets of the players around the frozen focus.
    frozen_offsets: prog 2 @ 0x23d04;
    /// `combat/damage`: Shield of the players at low hit points: by class (32 bytes) and hit points (difficulty < 6).
    damage_low_hp_shield: data 1 @ 0x40aa .. 0x416a;
    /// `combat/damage`: Same, at difficulty 6.
    damage_low_hp_shield_hard: data 1 @ 0x416a .. 0x422a;
    /// `combat/damage`: Damage rows of the enemies against the players, designated by `damage_enemy_scale`.
    damage_enemy_rows: data 1 @ 0x3f92 .. 0x3fea;
    /// `init`: Rows of minimum characteristics designated by `init_minima` (10 bytes per level).
    init_minimum_rows: data 1 @ 0xcf1c8 .. 0xcf3a8;
    /// `hit_reactions`: Stagger chances by hit, designated by `hit_reactions_stun_class`.
    stun_chances: data 1 @ 0xf1cc4 .. 0xf1d2e;
    /// `peff_h`: A word of the code of the lightning handler (0x4e038 + 0x168), that the original re-reads as the
    /// player's timer (original glitch).
    lightning_code_word: data 2 @ 0x4e1a0 .. 0x4e1a2;
    /// `combat/damage`: Party bonus by weapon (4 bytes per row).
    damage_party_bonus: data 1 @ 0x3d8e;
    /// `combat/damage`: Damage scale of the enemies by setting (words).
    damage_enemy_scale: prog 2 @ 0x3fea;
    /// `combat/damage`: Thief's loot: 8 bytes per loot kind (two entries of 4 bytes: item, weight...).
    damage_loot: data 1 @ 0x39d8;
    /// `peff`: Zones of the shop of each scene (first series, 8 bytes per scene).
    shop_zones_a: data 2 @ 0x51306;
    /// `peff`: Zones of the shop of each scene (second series).
    shop_zones_b: data 2 @ 0x51376 .. 0x51420;
    /// `peff_c`: Offsets (dx, dz) of the cure effect.
    cure_offsets: prog 2 @ 0x4a93a;
    /// `peff_c`: Offsets of the follower effect (first series).
    follow_offsets_a: prog 2 @ 0x4aa90;
    /// `peff_c`: Offsets of the follower effect (second series).
    follow_offsets_b: prog 2 @ 0x4ab2a;
    /// `peff_c`: Offsets of the prayer effect.
    prayer_offsets: prog 2 @ 0x4acb2;
    /// `peff_c`: Offsets of the prayer effect around the player.
    prayer_player_offsets: prog 2 @ 0x4ad38;
    /// `peff_c`: Offsets of the prayer sparks.
    prayer_spark_offsets: prog 2 @ 0x4ade0;
    /// `peff_d`: Offsets of the ray.
    ray_offsets: prog 2 @ 0x4b2dc;
    /// `peff_d`: Offsets of the ward (first series).
    ward_offsets_a: prog 2 @ 0x4b43c;
    /// `peff_d`: Offsets of the ward (second series).
    ward_offsets_b: prog 2 @ 0x4b4ae;
    /// `peff_d`: Offsets of the beam.
    beam_offsets: prog 2 @ 0x4b83e;
    /// `peff_e`: Offsets of the ice effect.
    ice_offsets: prog 2 @ 0x4bd0e;
    /// `peff_e`: Offsets of the crown (weapon 0x1a).
    crown_offsets_a: prog 2 @ 0x4c140;
    /// `peff_e`: Offsets of the crown (other weapons).
    crown_offsets_b: prog 2 @ 0x4c158;
    /// `peff_e`: Offsets of the banish effect.
    banish_offsets: prog 2 @ 0x4c226;
    /// `peff_h`: Offsets of the bolt (first series).
    bolt_effect_offsets_a: prog 2 @ 0x4dd84;
    /// `peff_h`: Offsets of the bolt (second series).
    bolt_effect_offsets_b: prog 2 @ 0x4de22;
    /// `peff_h`: Offsets of the bolt (third series).
    bolt_effect_offsets_c: prog 2 @ 0x4deac;
    /// `peff_h`: Speeds of the wide bolt.
    bolt_speeds_a: prog 2 @ 0x4df52;
    /// `peff_h`: Speeds of the narrow bolt.
    bolt_speeds_b: prog 2 @ 0x4dfc4;
    /// `peff_j`: Offsets of the chips (variant 0).
    chip_offsets_a: prog 2 @ 0x4fdd4;
    /// `peff_j`: Offsets of the chips (other variants).
    chip_offsets_b: prog 2 @ 0x4fdec;
    /// `peff_l`: Spots of the players (version 1).
    spots_player_a: data 2 @ 0x4cabe;
    /// `peff_l`: Spots of the slots (version 1).
    spots_slot_a: data 2 @ 0x4cae2;
    /// `peff_l`: Spots of the players (other versions).
    spots_player_b: data 2 @ 0x4caae;
    /// `peff_l`: Spots of the slots (other versions).
    spots_slot_b: data 2 @ 0x4caca;
    /// `peff_m`: Attack box of the creature (six bytes: dx, reach, dz, depth, dy, height).
    creature_box_attack: data 1 @ 0x4fbcc;
    /// `peff_m`: Bite box of the creature.
    creature_box_bite: data 1 @ 0x4fbd4;
    /// `peff_n`: Palette copied by the summoned creature (eight longs).
    summon_palette: data 4 @ 0x5036a;
    /// `peff_n`: Engage box of the summoned creature.
    summon_box_engage: data 1 @ 0x50b8a;
    /// `peff_n`: Strike box of the summoned creature.
    summon_box_strike: data 1 @ 0x50b92;
    /// `peff_o`: Spots of the players at the counter (version 1).
    counter_spots_a: data 2 @ 0x4d792;
    /// `peff_o`: Spots of the players at the counter (other versions).
    counter_spots_b: data 2 @ 0x4d782;
    /// `peff_o`: Prices of the items sold (4 bytes per item).
    counter_prices: data 1 @ 0x4db30;
    /// `peff_ring`: Offsets of the ring (first type).
    ring0_offsets: prog 2 @ 0x49f88;
    /// `peff_ring`: Speed tables of the ring by angle (first type).
    ring0_speeds: prog 4 @ 0x4a20a;
    /// `peff_ring`: Offsets of the ring (second type).
    ring1_offsets: prog 2 @ 0x4a3ac;
    /// `peff_ring`: Speed tables of the ring by angle (second type).
    ring1_speeds: prog 4 @ 0x4a84c;
    /// `pshl_e`: Turn rows of the fountain: (angle, animation) pairs.
    orbit_turn_table: data 1 @ 0x52b32;
    // ---- step and row tables of the cinematics and reactions, designated by their address (the key that the
    // dispatch passes around); each runs up to the next table or code ----
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_23ad6: prog 2 @ 0x23ad6;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_24138: prog 2 @ 0x24138;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_241b8: prog 2 @ 0x241b8;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_242dc: prog 2 @ 0x242dc;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_2436c: prog 2 @ 0x2436c;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_243f4: prog 2 @ 0x243f4;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_2449c: prog 2 @ 0x2449c;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_244fe: prog 2 @ 0x244fe;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_24590: prog 2 @ 0x24590;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_2460a: prog 2 @ 0x2460a;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_24660: prog 2 @ 0x24660;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_247d0: prog 2 @ 0x247d0;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_24864: prog 2 @ 0x24864;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_248fa: prog 2 @ 0x248fa;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_2497a: prog 2 @ 0x2497a;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_249fe: prog 2 @ 0x249fe;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_24a74: prog 2 @ 0x24a74;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h). Read up to 0x24aea: a player arriving with action 0x10 reads the word 0x6702 of the code (original glitch, `cinematics::STRAY_HEX_DUMP`).
    cinematic_24ad8: prog 2 @ 0x24ad8 .. 0x24aea;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_24b36: prog 2 @ 0x24b36;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_24bb0: prog 2 @ 0x24bb0;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_24c00: prog 2 @ 0x24c00;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_24c88: prog 2 @ 0x24c88;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_24d50: prog 2 @ 0x24d50;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_24e04: prog 2 @ 0x24e04;
    /// `cinematics`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_24e66: prog 2 @ 0x24e66;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_24ff0: prog 2 @ 0x24ff0;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25066: prog 2 @ 0x25066;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_250be: prog 2 @ 0x250be;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25154: prog 2 @ 0x25154;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_251ce: prog 2 @ 0x251ce;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_2523c: prog 2 @ 0x2523c;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_2528c: prog 2 @ 0x2528c;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_252ba: prog 2 @ 0x252ba;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25348: prog 2 @ 0x25348;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25350: prog 2 @ 0x25350;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_253ae: prog 2 @ 0x253ae;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25428: prog 2 @ 0x25428;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25492: prog 2 @ 0x25492;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25512: prog 2 @ 0x25512;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_2558e: prog 2 @ 0x2558e;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_255f8: prog 2 @ 0x255f8;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25658: prog 2 @ 0x25658;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25690: prog 2 @ 0x25690;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_256d0: prog 2 @ 0x256d0;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25732: prog 2 @ 0x25732;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_257b2: prog 2 @ 0x257b2;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25822: prog 2 @ 0x25822;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_2584c: prog 2 @ 0x2584c;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_2589a: prog 2 @ 0x2589a;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_258ec: prog 2 @ 0x258ec;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25962: prog 2 @ 0x25962;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_259b0: prog 2 @ 0x259b0;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25a6c: prog 2 @ 0x25a6c;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25ab0: prog 2 @ 0x25ab0;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25b4c: prog 2 @ 0x25b4c;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25bae: prog 2 @ 0x25bae;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25bf6: prog 2 @ 0x25bf6;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25c4a: prog 2 @ 0x25c4a;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25cac: prog 2 @ 0x25cac;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25d4e: prog 2 @ 0x25d4e;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25dc2: prog 2 @ 0x25dc2;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25e36: prog 2 @ 0x25e36;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25e82: prog 2 @ 0x25e82;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25f12: prog 2 @ 0x25f12;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_25f8c: prog 2 @ 0x25f8c;
    /// `cinematics/entries`: Step offsets or rows (positions, delays) of a scene script (h).
    cinematic_26004: prog 2 @ 0x26004;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3a1fc: prog 2 @ 0x3a1fc;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3a280: prog 2 @ 0x3a280;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3a584: prog 2 @ 0x3a584;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3a636: prog 2 @ 0x3a636;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3a802: prog 2 @ 0x3a802;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3a87a: prog 2 @ 0x3a87a;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3a89c: prog 2 @ 0x3a89c;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3a8be: prog 2 @ 0x3a8be;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3a9e6: prog 2 @ 0x3a9e6;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3aa04: prog 2 @ 0x3aa04;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3ab22: prog 2 @ 0x3ab22;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3ab5e: prog 2 @ 0x3ab5e;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3ada2: prog 2 @ 0x3ada2;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3ae96: prog 2 @ 0x3ae96;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3af50: prog 2 @ 0x3af50;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3b0d8: prog 2 @ 0x3b0d8;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3b16c: prog 2 @ 0x3b16c;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3b1f0: prog 2 @ 0x3b1f0;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3b31a: prog 2 @ 0x3b31a;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3b4d0: prog 2 @ 0x3b4d0;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3b57c: prog 2 @ 0x3b57c;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3b67a: prog 2 @ 0x3b67a;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3b738: prog 2 @ 0x3b738;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3b7c6: prog 2 @ 0x3b7c6;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3b8b2: prog 2 @ 0x3b8b2;
    /// `hurt`: Step offsets of a reaction (by step `+7`).
    hurt_steps_3b8d0: prog 2 @ 0x3b8d0;
    /// `combat/contact`: Weapon needed to parry, by enemy shot kind (h).
    shot_parry_weapons: prog 1 @ 0x27fd6;
    /// `combat/contact`: Damage reduction by (armor, class): words, 8 bytes per class.
    armor_reductions: prog 2 @ 0x27732;
    /// `combat/contact`: Damage of the crates: by crate type (16 words) and phase of the counter `TICK`.
    crate_damage: prog 2 @ 0x28334;
    /// `combat/contact`: Non-zero: the item kind can be picked up by contact (h).
    pickup_kinds: prog 1 @ 0x285e0;
    /// `combat/shots`: Non-zero: the player shot kind kills the small enemies at once.
    small_kill_shots: prog 1 @ 0x26856;
    /// `moves`: Offsets (dx, dz) of the object put down, per class.
    put_down_offsets: prog 2 @ 0x35ce2;
    /// `moves/poses`: Dust of the slow end (action 0x28): four words per class.
    slow_end_dust: prog 2 @ 0x36eea;
    /// `moves/leaps`: Animation of the leap attack per character.
    leap_attacks: prog 1 @ 0x376d6;
    /// `moves/leaps`: Animation of the leap attack per character, down key held.
    leap_attacks_down: prog 1 @ 0x376e2;
    /// `peff_ring`: Items shown by the second ring, per page (other characters).
    ring1_counts: prog 1 @ 0x4a470;
    /// `peff_ring`: Items shown by the second ring, per page (magic-user).
    ring1_counts_magic_user: prog 1 @ 0x4a474;
    /// `spawn`: Shots of a flame row of the wand, by level.
    rod_flame_counts: prog 1 @ 0x533f4;
    /// `spawn`: Wand balls alive at most, by level.
    rod_ball_limits: prog 1 @ 0x534b8;
    /// `spawn`: Shots of a star row of the wand, by level.
    rod_star_counts: prog 1 @ 0x5367c;
    /// `combos`: Code of the gate variants, whose `move.l` immediates (three longs at +0x16, +0x1a, +0x1e of each variant) are read as data.
    combo_gates: prog 2 @ 0xf19c2 .. 0xf1b26;
}
