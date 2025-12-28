use core::{constants::ItemFlags, types::Character};
use std::sync::{OnceLock, RwLock};

use crate::{god::God, repository::Repository, state::State};

#[derive(Clone, Copy)]
struct Destination {
    x: u32,
    y: u32,
}

#[derive(Clone, Copy)]
struct Riddle {
    question: &'static str,
    answer_1: &'static str,
    answer_2: &'static str,
    answer_3: &'static str,
}

#[derive(Clone, Copy)]
struct SwitchQuestions {
    should_be_true: bool,
    question: &'static str,
}

#[derive(Clone, Copy)]
struct Bank {
    x1: i32,
    y1: i32,
    temp: i32,
    doorx: i32,
    doory: i32,
}

// Switch questions data - matches C++ switch_questions[BANKS][BANK_QUESTIONS]
const SWITCH_QUESTIONS: [[SwitchQuestions; 8]; 5] = [
    // Bank 1
    [
        SwitchQuestions {
            should_be_true: true,
            question: "Jefferson gives the quest for repair.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "Steven asks you to bring him a greater healing potion.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "Gunthar lives in Rose Street.",
        },
        SwitchQuestions {
            should_be_true: false,
            question:
                "The golems in the Pentagram Quest say \"Dusdra gur, Hu-Har!\" when attacking.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "Argha is a Master Sergeant.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "There are exactly 17 rooms in the Dungeon of Doors.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "Serena is a templar.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "There is a purple flower growing under the tree beside the Magic Shop.",
        },
    ],
    // Bank 2
    [
        SwitchQuestions {
            should_be_true: false,
            question: "Ingrid gives the quest for recall.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "Nasir asks you to bring him a potion of life.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "Serena lives in Temple Street.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "If asked, Robin tells you about Lord Azrael of Aston.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "The barkeeper in the Tavern of the Blue is a First Lieutenant.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "In Stevens house is a hole, that leads into the Underground.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "Kira is a Staff Sergeant.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "Leopold is an old man of about 70 years.",
        },
    ],
    // Bank 3
    [
        SwitchQuestions {
            should_be_true: true,
            question: "Manfred gives the quest for sense magic.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "Leopold wants you to bring him a Ratling Fighters Eye.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "The priest in the Temple of the Purple One is a Master Sergeant.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "21 ghosts roam the Haunted Castle.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "Garna runs her shop right at the entrance to the mines.",
        },
        SwitchQuestions {
            should_be_true: false,
            question:
                "A golden ring adorned with a huge ruby raises you Intuition by 24 if activated.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "Jefferson is a Second Lieutenant.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "There is a green flower growing behind the Leather Armor Shop.",
        },
    ],
    // Bank 4
    [
        SwitchQuestions {
            should_be_true: false,
            question: "Sirjan gives the quest for identify.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "Cirrus wants you to bring him the Amulet of Resistance.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "There are three Ratling Counts to be found in the Underground.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "The ghosts in the Haunted Castle praise Damor when they die.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "Malte is a Corporal.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "Clara is wielding a golden dagger.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "Hagen is running the Golden Armor Shop.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "Nasir's left eye looks as if it was made of glass.",
        },
    ],
    // Bank 5
    [
        SwitchQuestions {
            should_be_true: false,
            question: "Shiva is a Baron of Astonia.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "Ursel is wearing bronze armor.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "The Lizard Archmages carry 93 silver pieces.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "The pentagram 134 is worth 2566 points.",
        },
        SwitchQuestions {
            should_be_true: true,
            question: "The Greenling Prince is a Captain.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "Antonia runs the leather armor shop.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "The pentagram 139 is worth 2766 points.",
        },
        SwitchQuestions {
            should_be_true: false,
            question: "Cirrus is a fat old man.",
        },
    ],
];

// Riddles data - matches C++ riddles[RIDDLEGIVERS][MAX_RIDDLES]
const RIDDLES: [[Riddle; 11]; 5] = [
    // Riddler 1
    [
        Riddle { question: "This marvelous thing\nThough it sounds absurd\nContains all our letters\nBut is only a word\nWhat ist it?\n", answer_1: "ALPHABET", answer_2: "", answer_3: "" },
        Riddle { question: "Like dogs shouting at the moon\nOr armor worn by the trees\nLike a sharply spoken command\nOr a tiny vessel upon the seas\nWhat is it?\n", answer_1: "BARK", answer_2: "", answer_3: "" },
        Riddle { question: "All about the house\nWith his Lady he dances\nYet he always works\nAnd never romances\nWhat ist it?\n", answer_1: "BROOM", answer_2: "", answer_3: "" },
        Riddle { question: "This engulfing thing is strange indeed\nThe greater it grows the less you see\nWhat ist it?\n", answer_1: "DARKNESS", answer_2: "DARK", answer_3: "" },
        Riddle { question: "I can not be seen only heard\nand I will not speak unless spoken to.\nWhat am I?\n", answer_1: "ECHO", answer_2: "", answer_3: "" },
        Riddle { question: "Power enough to smash ships and crush roofs\nYet it still must fear the sun\nWhat is it?\n", answer_1: "ICE", answer_2: "", answer_3: "" },
        Riddle { question: "What is it that you must give before you can keep it?\n", answer_1: "PROMISE", answer_2: "", answer_3: "" },
        Riddle { question: "Silently he stalks me\nRunning as I run\nCreeping as I creep\nDressed in black\nHe disappears at night\nOnly to return with the sun\nWhat is it?\n", answer_1: "SHADOW", answer_2: "", answer_3: "" },
        Riddle { question: "It flies without wings\nDrops without fear\nBut held in warm hands\nIt will soon disappear\nWhat is it?\n", answer_1: "SNOWFLAKE", answer_2: "SNOW", answer_3: "" },
        Riddle { question: "It never was before\nIt is not now\nFools wait for it forever\nWhat is it?\n", answer_1: "FUTURE", answer_2: "TOMORROW", answer_3: "" },
        Riddle { question: "I am emeralds and diamonds,\nLost by the moon.\nI am found by the sun\nAnd picked up soon.\nWhat am I?\n", answer_1: "DEW", answer_2: "", answer_3: "" },
    ],
    // Riddler 2
    [
        Riddle { question: "Come up and let us go;\ngo down and here we stay.\nWhat is it?\n", answer_1: "ANCHOR", answer_2: "", answer_3: "" },
        Riddle { question: "This sparkling globe can float on water and weighs not more than a feather\nYet despite its weight ten giants could never pick it up\nWhat is it?\n", answer_1: "BUBBLE", answer_2: "", answer_3: "" },
        Riddle { question: "The one who made it didnt want it\nThe one who bought it didnt need it\nThe one who used it never saw it\nWhat is it?\n", answer_1: "COFFIN", answer_2: "GRAVE", answer_3: "TOMB" },
        Riddle { question: "It can hold you\nBut you cannot hold it\nAnd the more you remove\nThe bigger it will get\nWhat is it?\n", answer_1: "HOLE", answer_2: "", answer_3: "" },
        Riddle { question: "Tear one off and scratch my head,\nwhat once was red is black instead.\nWhat am I?\n", answer_1: "MATCH", answer_2: "", answer_3: "" },
        Riddle { question: "Always old, sometimes new, never sad,\nsometimes blue. Never empty, sometimes full,\nnever pushes, always pulls.\nWhat is it?\n", answer_1: "MOON", answer_2: "", answer_3: "" },
        Riddle { question: "Drapes the hills all in white,\nswallows not but hard it bites.\nWhat is it?\n", answer_1: "FROST", answer_2: "", answer_3: "" },
        Riddle { question: "What can bring back the dead; make us cry,\nmake us laugh, make us young;\nborn in an instant yet lasts a life time.\nWhat is it?\n", answer_1: "MEMORY", answer_2: "MEMORIES", answer_3: "" },
        Riddle { question: "You tie these things\nBefore you go\nAnd untie them\nAfter you stop\nWhat is it?\n", answer_1: "SHOES", answer_2: "SHOE", answer_3: "" },
        Riddle { question: "The language of men can be mastered\nBut what word is always pronounced wrong?\n", answer_1: "WRONG", answer_2: "", answer_3: "" },
        Riddle { question: "Its tail is round and hollow\nSeems to get chewed a bit\nBut youll rarely see this thing\nUnless the other end is lit\nWhat is it?\n", answer_1: "PIPE", answer_2: "", answer_3: "" },
    ],
    // Riddler 3
    [
        Riddle { question: "After the final fire the winds will blow \nAnd these which are already dead will cover the ones who have yet to die \nWhat are these?\n", answer_1: "ASHES", answer_2: "", answer_3: "" },
        Riddle { question: "When I live I cry, if you don't kill me I'll die.\nWhat am I? \n", answer_1: "CANDLE", answer_2: "", answer_3: "" },
        Riddle { question: "Twins on either side of a ridge that smells\nThey shall never see each other directly\nWhat are they?\n", answer_1: "EYES", answer_2: "EYE", answer_3: "" },
        Riddle { question: "What is it the more you take, \nthe more you leave behind? \n", answer_1: "FOOTSTEPS", answer_2: "STEPS", answer_3: "STEP" },
        Riddle { question: "You see me oft\nIn woods and town\nWith my roots above\nI must grow down\nWhat am I?\n", answer_1: "ICICLE", answer_2: "ICICLES", answer_3: "" },
        Riddle { question: "Passed from father to son\nAnd shared between brothers\nIts importance is unquestioned\nThough it is used more by others\nWhat is it?\n", answer_1: "NAME", answer_2: "NAMES", answer_3: "" },
        Riddle { question: "Walk on the living, we don't even mumble.\nWalk on the dead, we mutter and grumble.\nWhat are we?\n", answer_1: "LEAVES", answer_2: "LEAF", answer_3: "" },
        Riddle { question: "She has tasteful friends\nAnd tasteless enemies\nTears are often shed on her behalf\nYet never has she broken a heart\nWhat is it?\n", answer_1: "ONION", answer_2: "ONIONS", answer_3: "" },
        Riddle { question: "This odd thing seems to get wetter\nThe more it dries\nWhat is it?\n", answer_1: "TOWEL", answer_2: "", answer_3: "" },
        Riddle { question: "He got it in the woods and brought it home in his hand because he couldn't find it\nThe more he looked for it the more he felt it When he finally found it he threw it away\nWhat was it?\n", answer_1: "THORN", answer_2: "PRICK", answer_3: "SLIVER" },
        Riddle { question: "Four legs in front two behind \nIts steely armor scratched and dented by rocks and sticks\nStill it toils as it helps feed the hungry\nWhat is it?\n", answer_1: "PLOW", answer_2: "", answer_3: "" },
    ],
    // Riddler 4
    [
        Riddle { question: "Black when bought \nRed when used \nGrey when thrown away\nWhat is it?\n", answer_1: "COALS", answer_2: "COAL", answer_3: "" },
        Riddle { question: "As I walked along the path \nI saw something with four fingers and one thumb, \nbut it was not flesh, fish, bone or fowl. \nWhat was it?\n", answer_1: "GLOVE", answer_2: "GLOVES", answer_3: "" },
        Riddle { question: "Look in my face I am somebody \nLook at my back I am nobody\nWhat am I?\n", answer_1: "MIRROR", answer_2: "", answer_3: "" },
        Riddle { question: "A shimmering field that reaches far \nYet it has no tracks \nAnd is crossed without paths \nWhat is it?\n", answer_1: "OCEAN", answer_2: "SEA", answer_3: "" },
        Riddle { question: "A precious gift this \nYet it has no end or beginning \nAnd in the middle nothing \nWhat is it?\n", answer_1: "RING", answer_2: "", answer_3: "" },
        Riddle { question: "An untiring servant it is carrying loads across muddy earth\nBut one thing that cannot be forced is a return to the place of its birth\nWhat is it?\n", answer_1: "RIVER", answer_2: "", answer_3: "" },
        Riddle { question: "It can pierce the best armor \nAnd make swords crumble with a rub \nYet for all its power \nIt can't harm a club\nWhat is it?\n", answer_1: "RUST", answer_2: "", answer_3: "" },
        Riddle { question: "No sooner spoken than broken. \nWhat is it? \n", answer_1: "SILENCE", answer_2: "", answer_3: "" },
        Riddle { question: "One pace to the North\nTwo paces to the East\nTwo paces to the South\nTwo paces to the West\nOne pace to the North\nWhat is it?\n", answer_1: "SQUARE", answer_2: "", answer_3: "" },
        Riddle { question: "This great thing can be swallowed\nBut can also swallow us\nWhat is it?\n", answer_1: "WATER", answer_2: "", answer_3: "" },
        Riddle { question: "Holes at the top \nHoles at the bottom \nHoles in the middle \nBut still it holds water\nWhat is it?\n", answer_1: "SPONGE", answer_2: "SPONGES", answer_3: "" },
    ],
    // Riddler 5
    [
        Riddle { question: "Feed me and I live, give me a drink and I die.\nWhat am I? \n", answer_1: "FIRE", answer_2: "", answer_3: "" },
        Riddle { question: "What goes through a door \nbut never goes in\nand never comes out? \n", answer_1: "KEYHOLE", answer_2: "", answer_3: "" },
        Riddle { question: "Some will use me, while others will not, \nsome have remembered, while others have forgot.\nFor profit or gain, I'm used expertly, \nI can't be picked off the ground or tossed into the sea.\nOnly gained from patience and time, \ncan you unravel my rhyme? \n", answer_1: "KNOWLEDGE", answer_2: "WISDOM", answer_3: "" },
        Riddle { question: "We love it more than life\nWe fear it more than death\nThe wealthy want for it\nThe poor have it in plenty\nWhat is it?\n", answer_1: "NOTHING", answer_2: "", answer_3: "" },
        Riddle { question: "If you have it, you want to share it. \nIf you share it, you don't have it. \nWhat is it? \n", answer_1: "SECRET", answer_2: "", answer_3: "" },
        Riddle { question: "Up and down they go but never move\nWhat are they?\n", answer_1: "STAIRS", answer_2: "STAIR", answer_3: "STEPS" },
        Riddle { question: "You must keep this thing \nIts loss will affect your brothers\nFor once yours is lost\nIt will soon be lost by others\nWhat is it?\n", answer_1: "TEMPER", answer_2: "", answer_3: "" },
        Riddle { question: "An open-ended barrel, it is shaped like a hive.\nIt is filled with flesh, and the flesh is alive! \nWhat is it?\n", answer_1: "THIMBLE", answer_2: "", answer_3: "" },
        Riddle { question: "Mountains will crumble and temples will fall,\nand no man can survive its endless call. \nWhat is it? \n", answer_1: "TIME", answer_2: "ETERNITY", answer_3: "" },
        Riddle { question: "This old one runs forever\nBut never moves at all\nHe has not lungs nor throat\nStill a mighty roaring call\nWhat is it?\n", answer_1: "WATERFALL", answer_2: "FALLS", answer_3: "" },
        Riddle { question: "You can hear me. \nYou can see what I do. \nMe, you cannot see. \nWhat am I? \n", answer_1: "WIND", answer_2: "STORM", answer_3: "" },
    ],
];

const BANKS: [Bank; core::constants::BANKS] = [
    Bank {
        x1: 23,
        y1: 707,
        temp: 1047,
        doorx: 23,
        doory: 720,
    },
    Bank {
        x1: 23,
        y1: 727,
        temp: 1069,
        doorx: 23,
        doory: 741,
    },
    Bank {
        x1: 23,
        y1: 747,
        temp: 1076,
        doorx: 23,
        doory: 761,
    },
    Bank {
        x1: 23,
        y1: 767,
        temp: 1084,
        doorx: 31,
        doory: 781,
    },
    Bank {
        x1: 23,
        y1: 787,
        temp: 1088,
        doorx: 19,
        doory: 803,
    },
];

const DESTINATIONS: [Destination; core::constants::RIDDLEGIVERS] = [
    Destination { x: 39, y: 729 },
    Destination { x: 40, y: 749 },
    Destination { x: 40, y: 769 },
    Destination { x: 40, y: 789 },
    Destination { x: 34, y: 806 },
];

static LABYRINTH9: OnceLock<RwLock<Labyrinth9>> = OnceLock::new();

pub struct Labyrinth9 {
    guesser: [i32; core::constants::RIDDLEGIVERS],
    riddleno: [i32; core::constants::RIDDLEGIVERS],
    riddle_timeout: [i32; core::constants::RIDDLEGIVERS],
    riddle_attempts: [i32; core::constants::RIDDLEGIVERS],
    riddles: &'static [[Riddle; core::constants::MAX_RIDDLES]; core::constants::RIDDLEGIVERS],
    switch_questions:
        &'static [[SwitchQuestions; core::constants::BANK_QUESTIONS]; core::constants::BANKS],
    banks: &'static [Bank; core::constants::BANKS],
    questions: [[i32; core::constants::SWITCHES]; core::constants::BANKS],
}

impl Labyrinth9 {
    fn new() -> Self {
        Self {
            guesser: [0; core::constants::RIDDLEGIVERS],
            riddleno: [0; core::constants::RIDDLEGIVERS],
            riddle_timeout: [0; core::constants::RIDDLEGIVERS],
            riddle_attempts: [0; core::constants::RIDDLEGIVERS],
            riddles: &RIDDLES,
            switch_questions: &SWITCH_QUESTIONS,
            banks: &BANKS,
            questions: [[0; core::constants::SWITCHES]; core::constants::BANKS],
        }
    }

    pub fn initialize() -> Result<(), String> {
        let lab = Labyrinth9::new();
        LABYRINTH9
            .set(RwLock::new(lab))
            .map_err(|_| "Labyrinth9 already initialized".to_string())?;

        Labyrinth9::with_mut(|lab| {
            for i in 1..BANKS.len() + 1 {
                lab.lab9_reset_bank(i as i32, true);
            }
        });

        Ok(())
    }

    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&Labyrinth9) -> R,
    {
        let lab = LABYRINTH9
            .get()
            .expect("Labyrinth9 not initialized")
            .read()
            .unwrap();
        f(&*lab)
    }

    pub fn with_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Labyrinth9) -> R,
    {
        let mut lab = LABYRINTH9
            .get()
            .expect("Labyrinth9 not initialized")
            .write()
            .unwrap();
        f(&mut *lab)
    }

    pub fn get_guesser(&self, idx: usize) -> i32 {
        self.guesser[idx]
    }

    pub fn lab9_guesser_says(&mut self, character_id: usize, text: &str) -> bool {
        let is_player =
            Repository::with_characters(|characters| characters[character_id].is_player());

        if !is_player {
            log::warn!(
                "Non-player character {} attempted to answer a riddle.",
                character_id
            );
            return false;
        }

        Repository::with_characters_mut(|characters| {
            let riddler = characters[character_id].data[core::constants::CHD_RIDDLER];

            // Valid riddler?
            if !Character::is_sane_npc(riddler as usize, &characters[riddler as usize]) {
                log::warn!(
                    "Character {} attempted to answer a riddle from invalid riddler {}.",
                    character_id,
                    riddler
                );
                characters[character_id].data[core::constants::CHD_RIDDLER] = 0;
                return false;
            }

            // Certified riddler?
            let area_of_knowledge = characters[riddler as usize].data[72]; // Area of knowledge
            if area_of_knowledge < core::constants::RIDDLE_MIN_AREA
                || area_of_knowledge > core::constants::RIDDLE_MAX_AREA
            {
                log::warn!(
                    "Character {} attempted to answer a riddle from uncertified riddler {}.",
                    character_id,
                    riddler
                );
                characters[character_id].data[core::constants::CHD_RIDDLER] = 0;
                return false;
            }

            // Does the riddler remember the guesser?
            let guesser_index = area_of_knowledge - core::constants::RIDDLE_MIN_AREA;
            let guesser_match = self.guesser[guesser_index as usize] == character_id as i32;

            if !guesser_match {
                log::warn!(
                    "Character {} attempted to answer a riddle from riddler {} who does not remember them.",
                    character_id,
                    riddler
                );
                characters[character_id].data[core::constants::CHD_RIDDLER] = 0;
                return false;
            }

            // Does the player see the riddler?
            let can_see_riddler =
                State::with_mut(|state| state.do_char_can_see(character_id, riddler as usize));

            if can_see_riddler == 0 {
                log::warn!(
                    "Character {} attempted to answer a riddle from riddler {} who they cannot see.",
                    character_id,
                    riddler
                );
                characters[character_id].data[core::constants::CHD_RIDDLER] = 0;
                return false;
            }

            let riddle = {
                let riddleno = self.riddleno[guesser_index as usize];
                self.riddles[guesser_index as usize][riddleno as usize - 1]
            };

            let mut found = false;
            for word in text.split(' ') {
                if riddle.answer_1.eq_ignore_ascii_case(word.trim()) {
                    found = true;
                    break;
                }
            }

            if found {
                State::with(|state| {
                    state.do_sayx(
                        riddler as usize,
                        format!(
                            "That's absolutely correct, {}! \nFor solving my riddle, I will advance you in your quest. \nClose your eyes and...\n",
                            characters[character_id].get_name()
                        ).as_str(),
                    );
                });

                if God::transfer_char(
                    character_id,
                    DESTINATIONS[guesser_index as usize].x as usize,
                    DESTINATIONS[guesser_index as usize].y as usize,
                ) {
                    characters[character_id].data[core::constants::CHD_RIDDLER] = 0;
                    self.guesser[guesser_index as usize] = 0;
                } else {
                    log::error!(
                        "Failed to transfer character {} to destination after solving riddle.",
                        character_id
                    );
                    State::with(|state| {
                        state.do_sayx(
                            riddler as usize,
                            "Oops! Something went wrong. Please try again a bit later.\n",
                        );
                    });
                }
                return true;
            } else {
                let riddle_attempts = {
                    self.riddle_attempts[guesser_index as usize] -= 1;
                    self.riddle_attempts[guesser_index as usize]
                };

                if riddle_attempts > 0 {
                    State::with(|state| {
                        state.do_sayx(
                            riddler as usize,
                            format!(
                                "Sorry, that's not right. You have {} more attempt{}!\n",
                                riddle_attempts,
                                if riddle_attempts == 1 { "" } else { "s" }
                            )
                            .as_str(),
                        );
                    });
                } else {
                    State::with(|state| {
                        state.do_sayx(
                            riddler as usize,
                            "Sorry, that's not right. Now you'll have to bring me the book again to start over!\n",
                        );
                    });
                    characters[character_id].data[core::constants::CHD_RIDDLER] = 0;
                    self.guesser[guesser_index as usize] = 0;
                }
            }

            return true;
        })
    }

    pub fn lab9_pose_riddle(&mut self, riddler_id: usize, character_id: usize) {
        let riddle_index = Repository::with_characters(|characters| {
            characters[riddler_id].data[72] - core::constants::RIDDLE_MIN_AREA
        });

        let riddle_number = 1 + rand::random::<i32>() % (core::constants::MAX_RIDDLES as i32);
        let question = self.riddles[riddle_index as usize][riddle_number as usize - 1].question;
        self.guesser[riddle_index as usize] = character_id as i32;
        self.riddleno[riddle_index as usize] = riddle_number;
        self.riddle_timeout[riddle_index as usize] = core::constants::RIDDLE_TIMEOUT;
        self.riddle_attempts[riddle_index as usize] = core::constants::RIDDLE_ATTEMPTS;
        State::with_mut(|state| {
            state.do_sayx(
                    riddler_id,
                    format!(
                        "Here is a riddle. You have 3 minutes and {} attempts to say the correct answer.\n",
                        self.riddle_attempts[riddle_index as usize],
                    ).as_str(),
                );

            state.do_sayx(riddler_id, question);
        });

        Repository::with_characters_mut(|characters| {
            characters[character_id].data[core::constants::CHD_RIDDLER] = riddler_id as i32;
        });
    }

    pub fn lab9_check_door(&self, bankno: i32) -> bool {
        if bankno < 1 || bankno > core::constants::BANKS as i32 {
            log::error!("lab9_check_door: invalid bank number {}", bankno);
            return false;
        }
        let bank_index = bankno - 1;

        let x = BANKS[bank_index as usize].x1;
        let mut y = BANKS[bank_index as usize].y1;
        let t = BANKS[bank_index as usize].temp;

        let mut correct = true;
        let mut m = x + y * core::constants::SERVER_MAPX as i32;

        for n in 0..core::constants::SWITCHES {
            let item_number = Repository::with_map(|map| map[m as usize].it);

            if item_number == 0
                || Repository::with_items(|items| items[item_number as usize].temp) != t as u16
            {
                log::error!(
                    "lab9_check_door: switch {} in bank {} is not set correctly",
                    n + 1,
                    bankno
                );
                return false;
            }

            let question = self.questions[bank_index as usize][n];
            let switch_is_true =
                Repository::with_items(|items| items[item_number as usize].data[1] == 1);

            if switch_is_true
                != self.switch_questions[bank_index as usize][question as usize - 1].should_be_true
            {
                correct = false;
            }

            y += 1;
        }

        // Door logic
        m = self.banks[bank_index as usize].doorx
            + self.banks[bank_index as usize].doory * core::constants::SERVER_MAPX as i32;

        let item_number = Repository::with_map(|map| map[m as usize].it);

        if item_number == 0 {
            log::error!(
                "lab9_check_door: door in bank {} has no item assigned",
                bankno
            );
            return false;
        }

        if correct {
            // Open the door
            Repository::with_items_mut(|items| {
                if items[item_number as usize].active == 0 {
                    return;
                }

                items[item_number as usize].data[1] = 0;
                items[item_number as usize].active = items[item_number as usize].duration;
                items[item_number as usize].flags &=
                    !(ItemFlags::IF_MOVEBLOCK | ItemFlags::IF_SIGHTBLOCK).bits();

                State::do_area_sound(
                    0,
                    0,
                    items[item_number as usize].x as i32,
                    items[item_number as usize].y as i32,
                    10,
                );

                State::with_mut(|state| {
                    state.reset_go(
                        items[item_number as usize].x as i32,
                        items[item_number as usize].y as i32,
                    );

                    state.add_lights(
                        items[item_number as usize].x as i32,
                        items[item_number as usize].y as i32,
                    );
                });
            });
            return true;
        } else {
            // Close the door
            Repository::with_items_mut(|items| {
                if !correct && items[item_number as usize].active != 0 {
                    return;
                }

                items[item_number as usize].data[1] = 1;
                items[item_number as usize].active = 0;
                let temp = items[item_number as usize].temp;
                let flags = Repository::with_item_templates(|item_templates| {
                    item_templates[temp as usize].flags & ItemFlags::IF_SIGHTBLOCK.bits()
                });

                items[item_number as usize].flags |= ItemFlags::IF_MOVEBLOCK.bits() | flags;

                State::do_area_sound(
                    0,
                    0,
                    items[item_number as usize].x as i32,
                    items[item_number as usize].y as i32,
                    10,
                );
                State::with_mut(|state| {
                    state.reset_go(
                        items[item_number as usize].x as i32,
                        items[item_number as usize].y as i32,
                    );

                    state.add_lights(
                        items[item_number as usize].x as i32,
                        items[item_number as usize].y as i32,
                    );
                });
            });
            return false;
        }
    }

    /// Reset a given numbered switch bank
    /// Translates C++ function: void lab9_reset_bank(int bankno, int closedoor)
    pub fn lab9_reset_bank(&mut self, bankno: i32, closedoor: bool) {
        log::info!("lab9: reset bank #{}", bankno);

        if bankno < 1 || bankno > core::constants::BANKS as i32 {
            log::error!("lab9_reset_bank(): panic: bad bank number {}!!", bankno);
            return;
        }

        let bank_index = (bankno - 1) as usize;
        let bank = BANKS[bank_index];
        let x = bank.x1;
        let mut y = bank.y1;
        let t = bank.temp;

        // Reset switches and build description from random question
        for n in 0..core::constants::SWITCHES {
            let m = (x + y * core::constants::SERVER_MAPX) as usize;
            let item_number = Repository::with_map(|map| map[m].it);

            if item_number == 0
                || Repository::with_items(|items| items[item_number as usize].temp) != t as u16
            {
                log::error!("reset_bank_at(): panic: no switch at {}!!", m);
                return;
            }

            let bankidx = Repository::with_items(|items| {
                (items[item_number as usize].data[0] as i32 - 1) as usize
            });

            Repository::with_items_mut(|items| {
                items[item_number as usize].data[1] = 1;
                items[item_number as usize].active = 0;
            });

            let mut q: i32;
            let mut unique: bool;

            loop {
                q = 1 + (rand::random::<i32>().abs() % core::constants::BANK_QUESTIONS as i32);

                self.questions[bankidx][n] = q;

                unique = true;
                for j in 0..n {
                    let prev_q = self.questions[bankidx][j];
                    if prev_q == q {
                        unique = false;
                        break;
                    }
                }

                if unique {
                    break;
                }
            }

            // Set description on the switch
            let question_text = SWITCH_QUESTIONS[bankidx][q as usize - 1].question;
            let description = format!(
                "It looks like a switch. Attached near the bottom is a note that reads: {}\n",
                question_text
            );

            Repository::with_items_mut(|items| {
                let desc_bytes = description.as_bytes();
                let len = desc_bytes.len().min(200);
                items[item_number as usize].description[..len].copy_from_slice(&desc_bytes[..len]);
                // Null-terminate if there's space
                if len < 200 {
                    items[item_number as usize].description[len] = 0;
                }
            });

            y += 1;
        }

        // Handle door
        let door_m = (bank.doorx + bank.doory * core::constants::SERVER_MAPX) as usize;
        let door = Repository::with_map(|map| map[door_m].it);

        if closedoor && door != 0 {
            self.use_lab9_door(0, door as i32);
        }
    }

    /// Flip a switch in Lab 9
    /// Translates C++ function: int use_lab9_switch(int cn, int in)
    pub fn use_lab9_switch(&self, cn: usize, item_id: i32) -> bool {
        log::info!("Character {} flipped a switch.", cn);

        Repository::with_items_mut(|items| {
            items[item_id as usize].data[1] = if items[item_id as usize].data[1] == 0 {
                1
            } else {
                0
            };

            State::do_area_sound(
                0,
                0,
                items[item_id as usize].x as i32,
                items[item_id as usize].y as i32,
                10,
            );

            let bank_no = items[item_id as usize].data[0] as i32;

            if self.lab9_check_door(bank_no) {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        "You hear a door open nearby.\n",
                    );
                });
            }
        });

        true
    }

    /// One way door in lab 9
    /// Translates C++ function: int use_lab9_door(int cn, int in)
    /// data[3] = switch bank number (1..5)
    pub fn use_lab9_door(&mut self, cn: usize, item_id: i32) -> bool {
        let item_x = Repository::with_items(|items| items[item_id as usize].x);
        let item_y = Repository::with_items(|items| items[item_id as usize].y);
        let m = (item_x as i32 + item_y as i32 * core::constants::SERVER_MAPX) as usize;

        let ch_at_door = Repository::with_map(|map| map[m].ch);
        if ch_at_door != 0 {
            return false;
        }

        // This statement allows free movement southward.
        if cn == 0 {
            Repository::with_items_mut(|items| {
                items[item_id as usize].active = 1; // just so it will close for sure
            });
        } else {
            let is_active = Repository::with_items(|items| items[item_id as usize].active);
            let character_x = Repository::with_characters(|characters| characters[cn].x);
            let character_y = Repository::with_characters(|characters| characters[cn].y);

            if is_active == 0
                && ((character_x as i32) > (item_x as i32)
                    || (character_y as i32) < (item_y as i32))
            {
                State::with(|state| {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        "It's locked and no key will open it.\n",
                    );
                });
                return false;
            }
        }

        State::with_mut(|state| {
            state.reset_go(item_x as i32, item_y as i32);
            state.remove_lights(item_x as i32, item_y as i32);
        });

        State::do_area_sound(0, 0, item_x as i32, item_y as i32, 10);

        let is_active = Repository::with_items(|items| items[item_id as usize].active);

        if is_active == 0 {
            // open door
            Repository::with_items_mut(|items| {
                items[item_id as usize].flags &=
                    !(ItemFlags::IF_MOVEBLOCK | ItemFlags::IF_SIGHTBLOCK).bits();
                items[item_id as usize].data[1] = 0;
            });
        } else {
            // close door
            let temp = Repository::with_items(|items| items[item_id as usize].temp);
            let flags = Repository::with_item_templates(|item_templates| {
                item_templates[temp as usize].flags & ItemFlags::IF_SIGHTBLOCK.bits()
            });

            Repository::with_items_mut(|items| {
                items[item_id as usize].flags |= ItemFlags::IF_MOVEBLOCK.bits() | flags;
                items[item_id as usize].data[1] = 1;

                let bank_no = items[item_id as usize].data[3] as i32;
                self.lab9_reset_bank(bank_no, false);
            });
        }

        State::with_mut(|state| {
            state.reset_go(item_x as i32, item_y as i32);
            state.add_lights(item_x as i32, item_y as i32);

            let character_position = Repository::with_characters(|characters| {
                (characters[cn].x as i32, characters[cn].y as i32)
            });
            state.do_area_notify(
                cn as i32,
                0,
                character_position.0 as i32,
                character_position.1 as i32,
                core::constants::NT_SEE as i32,
                cn as i32,
                0,
                0,
                0,
            );
        });

        true
    }
}
