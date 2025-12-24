pub fn sub_door_driver(cn: usize, item_idx: usize) -> i32 {}

pub fn use_door(cn: usize, item_idx: usize) -> i32 {}

pub fn use_create_item(cn: usize, item_idx: usize) -> i32 {}

pub fn use_create_gold(cn: usize, item_idx: usize) -> i32 {}

pub fn use_create_item2(cn: usize, item_idx: usize) -> i32 {}

pub fn use_create_item3(cn: usize, item_idx: usize) -> i32 {}

pub fn use_mix_potion(cn: usize, item_idx: usize) -> i32 {}

pub fn use_chain(cn: usize, item_idx: usize) -> i32 {}

pub fn stone_sword(cn: usize, item_idx: usize) -> i32 {}

pub fn finish_laby_teleport(cn: usize, nr: usize, exp: usize) -> i32 {}

pub fn is_nolab_item(item_idx: usize) -> bool {}

pub fn teleport(cn: usize, item_idx: usize) -> i32 {}

pub fn teleport2(cn: usize, item_idx: usize) -> i32 {}

pub fn use_labyrinth(cn: usize, item_idx: usize) -> i32 {}

pub fn use_ladder(cn: usize, item_idx: usize) -> i32 {}

pub fn use_bag(cn: usize, item_idx: usize) -> i32 {}

pub fn use_scroll(cn: usize, item_idx: usize) -> i32 {}

pub fn use_scroll2(cn: usize, item_idx: usize) -> i32 {}

pub fn use_scroll3(cn: usize, item_idx: usize) -> i32 {}

pub fn use_scroll4(cn: usize, item_idx: usize) -> i32 {}

pub fn use_scroll5(cn: usize, item_idx: usize) -> i32 {}

pub fn use_crystal_sub(cn: usize, item_idx: usize) -> i32 {}

pub fn use_crystal(cn: usize, item_idx: usize) -> i32 {}

pub fn use_mine_respawn(cn: usize, item_idx: usize) -> i32 {}

pub fn rat_eye(cn: usize, item_idx: usize) -> i32 {}

pub fn skua_protect(cn: usize, item_idx: usize) -> i32 {}

pub fn purple_protect(cn: usize, item_idx: usize) -> i32 {}

pub fn use_lever(cn: usize, item_idx: usize) -> i32 {}

pub fn use_spawn(cn: usize, item_idx: usize) -> i32 {}

pub fn use_pile(cn: usize, item_idx: usize) -> i32 {}

pub fn use_grave(cn: usize, item_idx: usize) -> i32 {}

pub fn mine_wall(cn: usize, item_idx: usize) -> i32 {}

pub fn mine_state(cn: usize, item_idx: usize) -> i32 {}

pub fn use_mine(cn: usize, item_idx: usize) -> i32 {}

pub fn use_mine_fast(cn: usize, item_idx: usize) -> i32 {}

pub fn build_ring(cn: usize, item_idx: usize) -> i32 {}

pub fn build_amulet(cn: usize, item_idx: usize) -> i32 {}

pub fn use_gargoyle(cn: usize, item_idx: usize) -> i32 {}

pub fn use_grolm(cn: usize, item_idx: usize) -> i32 {}

pub fn boost_char(cn: usize, divi: usize) -> i32 {}

pub fn spawn_penta_enemy(item_idx: usize) -> i32 {}

pub fn solved_pentagram(cn: usize, item_idx: usize) -> i32 {}

pub fn is_in_pentagram_quest(cn: usize) -> bool {}

pub fn use_pentagram(cn: usize, item_idx: usize) -> i32 {}

pub fn use_shrine(cn: usize, item_idx: usize) -> i32 {}

pub fn use_kill_undead(cn: usize, item_idx: usize) -> i32 {}

pub fn teleport3(cn: usize, item_idx: usize) -> i32 {}

pub fn use_seyan_shrine(cn: usize, item_idx: usize) -> i32 {}

pub fn use_seyan_door(cn: usize, item_idx: usize) -> i32 {}

pub fn use_seyan_portal(cn: usize, item_idx: usize) -> i32 {}

pub fn spell_scroll(cn: usize, item_idx: usize) -> i32 {}

pub fn use_blook_pentagram(cn: usize, item_idx: usize) -> i32 {}

pub fn use_create_npc(cn: usize, item_idx: usize) -> i32 {}

pub fn use_rotate(cn: usize, item_idx: usize) -> i32 {}

pub fn use_lab8_key(cn: usize, item_idx: usize) -> i32 {}

pub fn use_lab8_shrine(cn: usize, item_idx: usize) -> i32 {}

pub fn use_lab8_moneyshrine(cn: usize, item_idx: usize) -> i32 {}

pub fn change_to_archtemplar(cn: usize) {}

pub fn change_to_archharakim(cn: usize) {}

pub fn change_to_warrior(cn: usize) {}

pub fn change_to_sorcerer(cn: usize) {}

pub fn shrine_of_change(cn: usize, item_idx: usize) -> i32 {}

pub fn explorer_point(cn: usize, item_idx: usize) -> i32 {}

pub fn use_garbage(cn: usize, item_idx: usize) -> i32 {}

pub fn use_driver(cn: usize, item_idx: usize, carried: bool) {}

pub fn item_age(item_idx: usize) -> i32 {}

pub fn item_damage_worn(cn: usize, n: usize, damage: i32) {}

pub fn item_damage_citem(cn: usize, damage: i32) {}

pub fn item_damage_armor(cn: usize, damage: i32) {}

pub fn item_damage_weapon(cn: usize, damage: i32) {}

pub fn lightage(item_idx: usize, multi: i32) {}

pub fn age_message(cn: usize, item_idx: usize, where_is: &str) {}

pub fn char_item_expire(cn: usize) {}

pub fn may_deactivate(item_idx: usize) -> bool {}

pub fn pentagram(item_idx: usize) {}

pub fn spiderweb(item_idx: usize) {}

pub fn greenlingball(item_idx: usize) {}

pub fn expire_blood_penta(item_idx: usize) {}

pub fn expire_driver(item_idx: usize) {}

pub fn item_tick_expire() {}

pub fn item_tick_gc() {}

pub fn item_tick() {}

pub fn trap1(cn: usize, item_idx: usize) {}

pub fn trap2(cn: usize, item_idx: usize) {}

pub fn start_trap(cn: usize, item_idx: usize) {}

pub fn step_trap(cn: usize, item_idx: usize) -> i32 {}

pub fn step_trap_remove(cn: usize, item_idx: usize) {}

pub fn step_portal1_lab13(cn: usize, item_idx: usize) -> i32 {}

pub fn step_portal2_lab13(cn: usize, item_idx: usize) -> i32 {}

pub fn step_portal_arena(cn: usize, item_idx: usize) -> i32 {}

pub fn step_teleport(cn: usize, item_idx: usize) -> i32 {}

pub fn step_firefloor(cn: usize, item_idx: usize) -> i32 {}

pub fn step_firefloor_remove(cn: usize, item_idx: usize) {}

pub fn step_driver(cn: usize, item_idx: usize) -> i32 {}

pub fn step_driver_remove(cn: usize, item_idx: usize) {}
