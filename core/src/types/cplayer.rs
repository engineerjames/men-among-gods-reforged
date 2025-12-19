/// Client-side player data
#[derive(Clone)]
pub struct CPlayer {
    // informative stuff
    pub name: [u8; 40],

    pub mode: i32, // 0 = slow, 1 = medium, 2 = fast

    // character stats
    // [0]=bare value, 0=unknown
    // [1]=preset modifier, is race/npc dependend
    // [2]=race specific maximum
    // [3]=race specific difficulty to raise (0=not raisable, 1=easy ... 10=hard)
    // [4]=dynamic modifier, depends on equipment and spells
    // [5]=total value
    pub attrib: [[u8; 6]; 5],

    pub hp: [u16; 6],
    pub end: [u16; 6],
    pub mana: [u16; 6],

    pub skill: [[u8; 6]; 100],

    // temporary attributes
    pub a_hp: i32,
    pub a_end: i32,
    pub a_mana: i32,

    pub points: i32,
    pub points_tot: i32,
    pub kindred: i32,

    // posessions
    pub gold: i32,

    // items carried
    pub item: [i32; 40],
    pub item_p: [i32; 40],

    // items worn
    pub worn: [i32; 20],
    pub worn_p: [i32; 20],

    pub spell: [i32; 20],
    pub active: [i8; 20],

    pub weapon: i32,
    pub armor: i32,

    pub citem: i32,
    pub citem_p: i32,

    pub attack_cn: i32,
    pub goto_x: i32,
    pub goto_y: i32,
    pub misc_action: i32,
    pub misc_target1: i32,
    pub misc_target2: i32,
    pub dir: i32,

    // server only:
    pub x: i32,
    pub y: i32,
}

impl Default for CPlayer {
    fn default() -> Self {
        Self {
            name: [0; 40],
            mode: 0,
            attrib: [[0; 6]; 5],
            hp: [0; 6],
            end: [0; 6],
            mana: [0; 6],
            skill: [[0; 6]; 100],
            a_hp: 0,
            a_end: 0,
            a_mana: 0,
            points: 0,
            points_tot: 0,
            kindred: 0,
            gold: 0,
            item: [0; 40],
            item_p: [0; 40],
            worn: [0; 20],
            worn_p: [0; 20],
            spell: [0; 20],
            active: [0; 20],
            weapon: 0,
            armor: 0,
            citem: 0,
            citem_p: 0,
            attack_cn: 0,
            goto_x: 0,
            goto_y: 0,
            misc_action: 0,
            misc_target1: 0,
            misc_target2: 0,
            dir: 0,
            x: 0,
            y: 0,
        }
    }
}
