use crate::repository::Repository;

pub struct Area {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub name: &'static str,
    pub flag: i32,
}

impl Area {
    /// Returns true when the coordinate (`x`,`y`) lies within this area.
    ///
    /// Port of simple bounding-box containment check from the original C
    /// sources. Used by area lookup helpers to determine whether a position
    /// belongs to a named region.
    ///
    /// # Arguments
    /// * `x` - X coordinate to test
    /// * `y` - Y coordinate to test
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x1 && y >= self.y1 && x <= self.x2 && y <= self.y2
    }
}

const AREAS: &[Area] = &[
    Area {
        x1: 481,
        y1: 407,
        x2: 633,
        y2: 596,
        name: "Aston",
        flag: 0,
    },
    Area {
        x1: 497,
        y1: 205,
        x2: 537,
        y2: 233,
        name: "Lizard Temple",
        flag: 1,
    },
    Area {
        x1: 480,
        y1: 234,
        x2: 634,
        y2: 405,
        name: "Strange Forest",
        flag: 1,
    },
    Area {
        x1: 840,
        y1: 0,
        x2: 1024,
        y2: 157,
        name: "Underground I",
        flag: 0,
    },
    Area {
        x1: 491,
        y1: 504,
        x2: 520,
        y2: 520,
        name: "Temple of Skua",
        flag: 1,
    },
    Area {
        x1: 520,
        y1: 524,
        x2: 529,
        y2: 534,
        name: "Leather Armor Shop",
        flag: 1,
    },
    Area {
        x1: 539,
        y1: 525,
        x2: 548,
        y2: 532,
        name: "Jamil's House",
        flag: 0,
    },
    Area {
        x1: 545,
        y1: 536,
        x2: 564,
        y2: 548,
        name: "Temple of the Purple One",
        flag: 1,
    },
    Area {
        x1: 499,
        y1: 539,
        x2: 528,
        y2: 550,
        name: "Thieves House I",
        flag: 1,
    },
    Area {
        x1: 20,
        y1: 20,
        x2: 49,
        y2: 69,
        name: "Thieves House I",
        flag: 1,
    },
    Area {
        x1: 499,
        y1: 525,
        x2: 514,
        y2: 538,
        name: "Thieves House II",
        flag: 1,
    },
    Area {
        x1: 532,
        y1: 439,
        x2: 536,
        y2: 550,
        name: "Temple Street",
        flag: 2,
    },
    Area {
        x1: 532,
        y1: 551,
        x2: 578,
        y2: 555,
        name: "Rose Street",
        flag: 2,
    },
    Area {
        x1: 538,
        y1: 559,
        x2: 575,
        y2: 588,
        name: "Cursed Tomb",
        flag: 1,
    },
    Area {
        x1: 574,
        y1: 496,
        x2: 578,
        y2: 550,
        name: "Castle Way",
        flag: 2,
    },
    Area {
        x1: 588,
        y1: 530,
        x2: 623,
        y2: 554,
        name: "Haunted Castle",
        flag: 1,
    },
    Area {
        x1: 562,
        y1: 525,
        x2: 570,
        y2: 531,
        name: "Inga's House",
        flag: 0,
    },
    Area {
        x1: 582,
        y1: 519,
        x2: 587,
        y2: 524,
        name: "Jefferson's House",
        flag: 0,
    },
    Area {
        x1: 582,
        y1: 510,
        x2: 591,
        y2: 514,
        name: "Steel Armor Shop",
        flag: 1,
    },
    Area {
        x1: 554,
        y1: 509,
        x2: 570,
        y2: 520,
        name: "Joe's House",
        flag: 0,
    },
    Area {
        x1: 582,
        y1: 498,
        x2: 588,
        y2: 505,
        name: "Bronze Armor Shop",
        flag: 1,
    },
    Area {
        x1: 569,
        y1: 468,
        x2: 582,
        y2: 487,
        name: "Damor's Magic Shop",
        flag: 0,
    },
    Area {
        x1: 558,
        y1: 481,
        x2: 567,
        y2: 487,
        name: "Brunhild's Shop",
        flag: 0,
    },
    Area {
        x1: 555,
        y1: 499,
        x2: 563,
        y2: 504,
        name: "Sirjan's House",
        flag: 0,
    },
    Area {
        x1: 541,
        y1: 499,
        x2: 549,
        y2: 503,
        name: "Cloth Armor Shop",
        flag: 1,
    },
    Area {
        x1: 540,
        y1: 479,
        x2: 553,
        y2: 487,
        name: "Weapon Shop",
        flag: 1,
    },
    Area {
        x1: 507,
        y1: 482,
        x2: 514,
        y2: 487,
        name: "Tavern of the Blue Ogre",
        flag: 1,
    },
    Area {
        x1: 515,
        y1: 436,
        x2: 522,
        y2: 444,
        name: "Bank",
        flag: 1,
    },
    Area {
        x1: 540,
        y1: 442,
        x2: 546,
        y2: 451,
        name: "Cirrus' House",
        flag: 0,
    },
    Area {
        x1: 521,
        y1: 450,
        x2: 528,
        y2: 459,
        name: "Serena's House",
        flag: 0,
    },
    Area {
        x1: 540,
        y1: 456,
        x2: 564,
        y2: 474,
        name: "Magic Maze",
        flag: 1,
    },
    Area {
        x1: 512,
        y1: 465,
        x2: 528,
        y2: 471,
        name: "Steven's House",
        flag: 0,
    },
    Area {
        x1: 519,
        y1: 477,
        x2: 527,
        y2: 484,
        name: "Golden Armor Shop",
        flag: 1,
    },
    Area {
        x1: 537,
        y1: 434,
        x2: 616,
        y2: 438,
        name: "New Street",
        flag: 2,
    },
    Area {
        x1: 559,
        y1: 442,
        x2: 565,
        y2: 448,
        name: "Gordon's House",
        flag: 0,
    },
    Area {
        x1: 571,
        y1: 442,
        x2: 577,
        y2: 448,
        name: "Nasir's House",
        flag: 0,
    },
    Area {
        x1: 582,
        y1: 442,
        x2: 610,
        y2: 462,
        name: "Templar Outlaws",
        flag: 3,
    },
    Area {
        x1: 614,
        y1: 434,
        x2: 618,
        y2: 495,
        name: "South End",
        flag: 2,
    },
    Area {
        x1: 590,
        y1: 467,
        x2: 610,
        y2: 488,
        name: "Skeleton Lord",
        flag: 3,
    },
    Area {
        x1: 537,
        y1: 491,
        x2: 613,
        y2: 495,
        name: "Merchant's Way",
        flag: 2,
    },
    Area {
        x1: 593,
        y1: 498,
        x2: 601,
        y2: 505,
        name: "Ingrid's House",
        flag: 0,
    },
    Area {
        x1: 501,
        y1: 400,
        x2: 526,
        y2: 432,
        name: "Abandoned Kwai Clan Hall",
        flag: 1,
    },
    Area {
        x1: 500,
        y1: 558,
        x2: 525,
        y2: 590,
        name: "Abandoned Gorn Clan Hall",
        flag: 1,
    },
    Area {
        x1: 493,
        y1: 448,
        x2: 519,
        y2: 476,
        name: "Arena",
        flag: 1,
    },
    Area {
        x1: 411,
        y1: 331,
        x2: 478,
        y2: 394,
        name: "Black Stronghold",
        flag: 1,
    },
    Area {
        x1: 561,
        y1: 410,
        x2: 604,
        y2: 430,
        name: "Dungeon of Doors",
        flag: 1,
    },
    Area {
        x1: 125,
        y1: 57,
        x2: 197,
        y2: 131,
        name: "Random Dungeon",
        flag: 1,
    },
    Area {
        x1: 411,
        y1: 460,
        x2: 479,
        y2: 529,
        name: "Mine, 1st Level",
        flag: 1,
    },
    Area {
        x1: 771,
        y1: 20,
        x2: 839,
        y2: 89,
        name: "Mine, 2nd Level",
        flag: 1,
    },
    Area {
        x1: 700,
        y1: 20,
        x2: 768,
        y2: 89,
        name: "Mine, 3rd Level",
        flag: 1,
    },
    Area {
        x1: 52,
        y1: 52,
        x2: 105,
        y2: 104,
        name: "Labyrinth, Grolm Gorge",
        flag: 1,
    },
    Area {
        x1: 58,
        y1: 158,
        x2: 154,
        y2: 212,
        name: "Labyrinth, Lizard Gorge",
        flag: 1,
    },
    Area {
        x1: 30,
        y1: 236,
        x2: 151,
        y2: 307,
        name: "Labyrinth, Spellcaster Gorge",
        flag: 1,
    },
    Area {
        x1: 25,
        y1: 330,
        x2: 110,
        y2: 375,
        name: "Labyrinth, Knight Gorge",
        flag: 1,
    },
    Area {
        x1: 16,
        y1: 385,
        x2: 119,
        y2: 455,
        name: "Labyrinth, Undead Gorge",
        flag: 1,
    },
    Area {
        x1: 16,
        y1: 459,
        x2: 56,
        y2: 487,
        name: "Labyrinth, Undead Gorge",
        flag: 1,
    },
    Area {
        x1: 16,
        y1: 489,
        x2: 81,
        y2: 529,
        name: "Labyrinth, Light&Dark Gorge",
        flag: 1,
    },
    Area {
        x1: 16,
        y1: 529,
        x2: 81,
        y2: 591,
        name: "Labyrinth, Water Gorge",
        flag: 1,
    },
    Area {
        x1: 16,
        y1: 593,
        x2: 48,
        y2: 602,
        name: "Labyrinth, Final Entry",
        flag: 1,
    },
    Area {
        x1: 16,
        y1: 602,
        x2: 48,
        y2: 608,
        name: "Labyrinth, Final Preparations",
        flag: 1,
    },
    Area {
        x1: 38,
        y1: 593,
        x2: 48,
        y2: 602,
        name: "Labyrinth, Final Test",
        flag: 1,
    },
    Area {
        x1: 49,
        y1: 593,
        x2: 80,
        y2: 608,
        name: "Labyrinth, Final Reward",
        flag: 1,
    },
    Area {
        x1: 15,
        y1: 611,
        x2: 126,
        y2: 703,
        name: "Labyrinth, Forest Gorge",
        flag: 1,
    },
    Area {
        x1: 112,
        y1: 703,
        x2: 126,
        y2: 708,
        name: "Labyrinth, Forest Gorge",
        flag: 1,
    },
    Area {
        x1: 15,
        y1: 704,
        x2: 66,
        y2: 724,
        name: "Labyrinth, Riddle Gorge",
        flag: 1,
    },
    Area {
        x1: 15,
        y1: 724,
        x2: 48,
        y2: 804,
        name: "Labyrinth, Riddle Gorge",
        flag: 1,
    },
    Area {
        x1: 15,
        y1: 804,
        x2: 37,
        y2: 812,
        name: "Labyrinth, Riddle Gorge",
        flag: 1,
    },
    Area {
        x1: 210,
        y1: 300,
        x2: 410,
        y2: 600,
        name: "Pentagram Quest",
        flag: 1,
    },
    Area {
        x1: 330,
        y1: 246,
        x2: 408,
        y2: 299,
        name: "Ice Pentagram Quest",
        flag: 1,
    },
    Area {
        x1: 822,
        y1: 176,
        x2: 1020,
        y2: 333,
        name: "Underground II",
        flag: 1,
    },
    Area {
        x1: 792,
        y1: 796,
        x2: 813,
        y2: 811,
        name: "Elysium",
        flag: 1,
    },
    Area {
        x1: 367,
        y1: 227,
        x2: 410,
        y2: 244,
        name: "Gargoyle's Nest",
        flag: 1,
    },
    Area {
        x1: 410,
        y1: 227,
        x2: 480,
        y2: 329,
        name: "Gargoyle's Nest",
        flag: 1,
    },
    Area {
        x1: 1,
        y1: 1,
        x2: 20,
        y2: 20,
        name: "Aston",
        flag: 0,
    },
    Area {
        x1: 622,
        y1: 466,
        x2: 629,
        y2: 477,
        name: "Astonian Inn",
        flag: 1,
    },
    Area {
        x1: 630,
        y1: 466,
        x2: 633,
        y2: 481,
        name: "Astonian Inn",
        flag: 1,
    },
    Area {
        x1: 61,
        y1: 122,
        x2: 65,
        y2: 131,
        name: "Astonian Inn",
        flag: 1,
    },
    Area {
        x1: 53,
        y1: 118,
        x2: 60,
        y2: 124,
        name: "Astonian Inn",
        flag: 1,
    },
    Area {
        x1: 36,
        y1: 106,
        x2: 52,
        y2: 141,
        name: "Astonian Inn",
        flag: 1,
    },
    Area {
        x1: 29,
        y1: 137,
        x2: 38,
        y2: 144,
        name: "Astonian Inn",
        flag: 1,
    },
    Area {
        x1: 578,
        y1: 566,
        x2: 633,
        y2: 596,
        name: "Memorial Park",
        flag: 1,
    },
    Area {
        x1: 820,
        y1: 158,
        x2: 851,
        y2: 161,
        name: "Staffers Corner",
        flag: 1,
    },
    Area {
        x1: 807,
        y1: 151,
        x2: 819,
        y2: 168,
        name: "Staffers Corner",
        flag: 1,
    },
    Area {
        x1: 793,
        y1: 146,
        x2: 805,
        y2: 156,
        name: "Staffers Corner",
        flag: 1,
    },
    Area {
        x1: 799,
        y1: 158,
        x2: 805,
        y2: 168,
        name: "Staffers Corner",
        flag: 1,
    },
    Area {
        x1: 794,
        y1: 163,
        x2: 798,
        y2: 165,
        name: "Staffers Corner",
        flag: 1,
    },
    Area {
        x1: 632,
        y1: 240,
        x2: 647,
        y2: 257,
        name: "Tower I",
        flag: 1,
    },
    Area {
        x1: 822,
        y1: 337,
        x2: 837,
        y2: 352,
        name: "Tower II",
        flag: 1,
    },
    Area {
        x1: 822,
        y1: 354,
        x2: 837,
        y2: 367,
        name: "Tower III",
        flag: 1,
    },
    Area {
        x1: 822,
        y1: 369,
        x2: 837,
        y2: 384,
        name: "Tower IV",
        flag: 1,
    },
    Area {
        x1: 822,
        y1: 386,
        x2: 837,
        y2: 401,
        name: "Tower V",
        flag: 1,
    },
    Area {
        x1: 854,
        y1: 338,
        x2: 870,
        y2: 353,
        name: "Tower VI",
        flag: 1,
    },
    Area {
        x1: 854,
        y1: 355,
        x2: 870,
        y2: 368,
        name: "Tower VII",
        flag: 1,
    },
    Area {
        x1: 854,
        y1: 370,
        x2: 870,
        y2: 384,
        name: "Tower VIII",
        flag: 1,
    },
    Area {
        x1: 854,
        y1: 386,
        x2: 870,
        y2: 401,
        name: "Tower IX",
        flag: 1,
    },
    Area {
        x1: 854,
        y1: 403,
        x2: 870,
        y2: 419,
        name: "Tower X",
        flag: 1,
    },
    Area {
        x1: 854,
        y1: 422,
        x2: 870,
        y2: 437,
        name: "Tower XI",
        flag: 1,
    },
    Area {
        x1: 851,
        y1: 440,
        x2: 869,
        y2: 459,
        name: "Tower XII",
        flag: 1,
    },
    Area {
        x1: 849,
        y1: 463,
        x2: 866,
        y2: 478,
        name: "Tower XIII",
        flag: 1,
    },
    Area {
        x1: 848,
        y1: 479,
        x2: 866,
        y2: 495,
        name: "Tower XIV",
        flag: 1,
    },
    Area {
        x1: 847,
        y1: 498,
        x2: 863,
        y2: 522,
        name: "Tower XV",
        flag: 1,
    },
    Area {
        x1: 842,
        y1: 514,
        x2: 846,
        y2: 522,
        name: "Tower XV",
        flag: 1,
    },
    Area {
        x1: 839,
        y1: 523,
        x2: 863,
        y2: 545,
        name: "Tower XVI",
        flag: 1,
    },
    Area {
        x1: 839,
        y1: 546,
        x2: 855,
        y2: 564,
        name: "Tower XVI",
        flag: 1,
    },
    Area {
        x1: 839,
        y1: 570,
        x2: 858,
        y2: 590,
        name: "Tower XVII",
        flag: 1,
    },
    Area {
        x1: 839,
        y1: 594,
        x2: 858,
        y2: 613,
        name: "Tower XVIII",
        flag: 1,
    },
    Area {
        x1: 839,
        y1: 616,
        x2: 858,
        y2: 635,
        name: "Tower XIX",
        flag: 1,
    },
    Area {
        x1: 841,
        y1: 639,
        x2: 855,
        y2: 657,
        name: "Tower XX",
        flag: 1,
    },
    Area {
        x1: 839,
        y1: 658,
        x2: 857,
        y2: 683,
        name: "Tower XX",
        flag: 1,
    },
    Area {
        x1: 836,
        y1: 684,
        x2: 860,
        y2: 694,
        name: "Tower XX",
        flag: 1,
    },
    Area {
        x1: 411,
        y1: 436,
        x2: 476,
        y2: 456,
        name: "Pentagram Quest",
        flag: 1,
    },
    Area {
        x1: 597,
        y1: 600,
        x2: 619,
        y2: 619,
        name: "Aston Hall",
        flag: 0,
    },
    Area {
        x1: 529,
        y1: 198,
        x2: 537,
        y2: 204,
        name: "Lizard Temple Shrine",
        flag: 1,
    },
    Area {
        x1: 321,
        y1: 294,
        x2: 327,
        y2: 300,
        name: "Ice Pents Shrine",
        flag: 1,
    },
    Area {
        x1: 529,
        y1: 198,
        x2: 537,
        y2: 204,
        name: "Lizard Temple Shrine",
        flag: 1,
    },
    Area {
        x1: 577,
        y1: 597,
        x2: 596,
        y2: 628,
        name: "Aston Hall",
        flag: 1,
    },
    Area {
        x1: 597,
        y1: 620,
        x2: 619,
        y2: 628,
        name: "Aston Hall",
        flag: 1,
    },
    Area {
        x1: 620,
        y1: 597,
        x2: 633,
        y2: 628,
        name: "Aston Hall",
        flag: 1,
    },
    Area {
        x1: 597,
        y1: 597,
        x2: 619,
        y2: 599,
        name: "Aston Hall",
        flag: 1,
    },
    Area {
        x1: 836,
        y1: 151,
        x2: 846,
        y2: 154,
        name: "Underground I",
        flag: 1,
    },
    Area {
        x1: 479,
        y1: 360,
        x2: 479,
        y2: 363,
        name: "Black Stronghold",
        flag: 1,
    },
    Area {
        x1: 634,
        y1: 466,
        x2: 639,
        y2: 477,
        name: "Astonian Inn",
        flag: 1,
    },
    Area {
        x1: 532,
        y1: 406,
        x2: 536,
        y2: 406,
        name: "Aston",
        flag: 0,
    },
    Area {
        x1: 469,
        y1: 457,
        x2: 473,
        y2: 459,
        name: "Pentagram Quest",
        flag: 1,
    },
];

pub fn is_in_pentagram_quest(cn: usize) -> bool {
    if cn < 1 || cn >= crate::core::constants::MAXCHARS as usize {
        return false;
    }

    let coords = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
    let x = coords.0;
    let y = coords.1;

    let n: [usize; 5] = [67, 68, 110, 113, 123];
    for i in 0..5 {
        let idx = n[i];
        if AREAS.get(idx).map_or(false, |a| a.contains(x, y)) {
            return true;
        }
    }
    false
}

// Unused in original implementation as well
#[allow(dead_code)]
pub fn get_area(cn: usize, verbose: bool) -> String {
    if cn < 1 || cn >= crate::core::constants::MAXCHARS as usize {
        return String::new();
    }

    let (x, y) = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
    let mut buf = String::new();
    let mut first = true;

    for a in AREAS.iter() {
        if a.contains(x, y) {
            if verbose {
                if first {
                    buf.push_str("In ");
                    first = false;
                } else {
                    buf.push_str(", in ");
                }
                match a.flag {
                    1 => buf.push_str("the "),
                    2 => buf.push_str("on "),
                    3 => buf.push_str("at "),
                    _ => {}
                }
                buf.push_str(a.name);
            } else {
                if !first {
                    buf.push_str(", ");
                }
                first = false;
                buf.push_str(a.name);
            }
        }
    }

    buf
}

pub fn get_area_m(x: i32, y: i32, verbose: bool) -> String {
    let mut buf = String::new();
    let mut first = true;

    for a in AREAS.iter() {
        if a.contains(x, y) {
            if verbose {
                if first {
                    buf.push_str("in ");
                    first = false;
                } else {
                    buf.push_str(", in ");
                }
                match a.flag {
                    1 => buf.push_str("the "),
                    2 => buf.push_str("on "),
                    3 => buf.push_str("at "),
                    _ => {}
                }
                buf.push_str(a.name);
            } else {
                if !first {
                    buf.push_str(", ");
                }
                first = false;
                buf.push_str(a.name);
            }
        }
    }

    buf
}
