use core::constants::CT_COMPANION;

struct Know {
    word: [&'static str; 20],
    value: i32,
    area: i32,
    temp: i32,
    answer: &'static str,
    special: i32,
}

const HEALTH: i32 = 1;
const SHOP: i32 = 2;
const GREET: i32 = 3;
const WHOAMI: i32 = 4;
const WHERE: i32 = 5;
const STOP: i32 = 6;
const MOVE: i32 = 7;
const ATTACK: i32 = 8;
const WAIT: i32 = 9;
const FOLLOW: i32 = 10;
const TIME: i32 = 11;
const POINTS: i32 = 12;
const BUYGOLD: i32 = 13;
const BUYHEALTH: i32 = 14;
const BUYMANA: i32 = 15;
const BUYEXP: i32 = 16;
const TRANSFER: i32 = 17;
const SPELLINFO: i32 = 18;
const QUIET: i32 = 19;

const AR_GENERAL: i32 = 0;
const AR_THIEF: i32 = 1;
const AR_CASTLE: i32 = 2;
const AR_GROLM: i32 = 3;
const AR_ASTON: i32 = 4; // locations in aston
const AR_THIEF2: i32 = 5;
const AR_TOMB: i32 = 6;
const AR_JOE: i32 = 7;
const AR_PURPLE: i32 = 8;
const AR_SLORD: i32 = 9;
const AR_OUTLAW: i32 = 10;
const AR_MMAZE: i32 = 11;
const AR_LIZARD: i32 = 12;
const AR_UNDER_I: i32 = 13;
const AR_KNIGHT: i32 = 14;
const AR_MINE: i32 = 15;
const AR_STRONGHOLD: i32 = 16;
const AR_NEST: i32 = 17;

/* keep these in tune with RIDDLE_MIN_AREA/RIDDLE_MAX_AREA in Lab9.h! */
const AR_RIDDLE1: i32 = 21;
const AR_RIDDLE2: i32 = 22;
const AR_RIDDLE3: i32 = 23;
const AR_RIDDLE4: i32 = 24;
const AR_RIDDLE5: i32 = 25;
const RIDDLE_TEXT: &str = "If you bring me the right volume of the Book of Wisdom, I will tell you a riddle. To answer the riddle, just say the word the riddle asks for. If you answer correctly, I will bring you to the next stage of your quest.\n";

const AR_ALL: i32 = 12345;

const KNOW: [Know; 227] = [
    Know {
        word: ["!where", "!tavern", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The tavern lies in the north part of the city, on Temple Street.",
        special: 0,
    },
    Know {
        word: ["!where", "!thief", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The Thieves House is located at the east end of Temple Street.",
        special: 0,
    },
    Know {
        word: ["!where", "?haunted", "!castle", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The Haunted Castle is in the south-east corner of the city, on Castle Way.",
        special: 0,
    },
    Know {
        word: ["!where", "?cursed", "!tomb", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The Cursed Tomb is in the north-east corner of the city, on Rose Street.",
        special: 0,
    },
    Know {
        word: ["!where", "!joe", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "Joe's House is in the middle of Castle Way.",
        special: 0,
    },
    Know {
        word: ["!where", "?skeleton", "!lord", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The Skeleton Lord is in the dark, large building in the middle of Merchants Way.",
        special: 0,
    },
    Know {
        word: ["!where", "?templar", "!outlaw", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The Templar Outlaws live in a fortified building in the south-west corner of Aston.",
        special: 0,
    },
    Know {
        word: ["!where", "?magic", "!maze", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The Magic Maze is in the north-west corner of Aston.",
        special: 0,
    },
    Know {
        word: ["!where", "!labyrinth", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The entrance to the Labyrinth is in the middle of Merchants Way.",
        special: 0,
    },
    Know {
        word: ["!where", "!random", "?dungeon", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The entrance to the Random Dungeon is in the middle of Merchants Way.",
        special: 0,
    },
    Know {
        word: ["!where", "!bank", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The bank is in the north west corner of thy city, on Temple Street.",
        special: 0,
    },
    Know {
        word: ["!where", "!shop", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "Most shops are on Merchants Way.",
        special: 0,
    },
    Know {
        word: ["!where", "!buy", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "Most shops are on Merchants Way.",
        special: 0,
    },
    Know {
        word: ["!have", "!no", "!money", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "You might find some valuable stuff in the Donations Room in one of the temples.",
        special: 0,
    },
    Know {
        word: ["!where", "!temple", "!street", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The Temple Street is in the northern part of the city. It goes from the east to the west end of Aston.",
        special: 0,
    },
    Know {
        word: ["!where", "!castle", "!way", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The Temple Street is in the south-east corner of the city.",
        special: 0,
    },
    Know {
        word: ["!where", "!south", "!end", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "South End? That's a street in the south-west corner of Aston.",
        special: 0,
    },
    Know {
        word: ["!where", "!rose", "!street", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "Rose Street is on the eastern border of the city.",
        special: 0,
    },
    Know {
        word: ["!where", "!merchant", "!way", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The Merchants Way divides the city in the middle. It goes from north to south.",
        special: 0,
    },
    Know {
        word: ["!where", "!new", "!street", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "New Street is on the western border of Aston.",
        special: 0,
    },
    Know {
        word: ["?where", "?what", "!aston", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "This city is called Aston. But you should know that!",
        special: 0,
    },
    Know {
        word: ["!where", "!temple", "?skua", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The Temple of Skua is in the eastern part of Temple Street.",
        special: 0,
    },
    Know {
        word: ["!where", "!jamil", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "Jamil lives on Temple Street, a bit east of the Temple of Skua.",
        special: 0,
    },
    Know {
        word: ["!who", "!jamil", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "Jamil? He's a merchant. Rumor has it he has some connections to the thieves.",
        special: 0,
    },
    Know {
        word: ["!where", "!sirjan", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "Sirjan lives on Merchants Way. Fairly close to Damor's Shop.",
        special: 0,
    },
    Know {
        word: ["!where", "!damor", "?shop", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "Damor's Magic Shop? It's on Merchants Way.",
        special: 0,
    },
    Know {
        word: ["!who", "!damor", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 5,
        area: AR_ASTON,
        temp: 0,
        answer: "Damor came into Aston about 60 years ago. Noboby knows where he came from. He is very powerful, so no one dared to ask.",
        special: 0,
    },
    Know {
        word: ["!tell", "!damor", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 5,
        area: AR_ASTON,
        temp: 0,
        answer: "Damor came into Aston about 60 years ago. Noboby knows where he came from. He is very powerful, so no one dared to ask.",
        special: 0,
    },
    Know {
        word: ["!ratling", "?eye", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "You'd best ask Robin about that. He lives on South End.",
        special: 0,
    },
    Know {
        word: ["!underground", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "You'd best ask Robin about that. He lives on South End.",
        special: 0,
    },
    Know {
        word: ["!azrael", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "You'd best ask Robin about that. He lives on South End.",
        special: 0,
    },
    Know {
        word: ["!where", "!mine", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "The mine is north of the Temple of Skua.",
        special: 0,
    },
    Know {
        word: ["?what", "?where", "?good", "!start", "?place", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "A good place to start your adventurer's life? You'd best talk to Jamil.",
        special: 0,
    },
    Know {
        word: ["?tell", "?where", "?good", "!start", "?place", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_ASTON,
        temp: 0,
        answer: "A good place to start your adventurer's life? You'd best talk to Jamil.",
        special: 0,
    },
    Know {
        word: ["!where", "!thief", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_THIEF,
        temp: 25,
        answer: "The Thieves House is located at the east end of Temple Street.",
        special: 0,
    },
    Know {
        word: ["!where", "?thief", "!house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_THIEF,
        temp: 25,
        answer: "The Thieves House is located at the east end of Temple Street.",
        special: 0,
    },
    Know {
        word: ["!locked", "!door", "!thief", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF,
        temp: 25,
        answer: "There are two locked doors in the Thieves House. For the first, you'll easily find the key.",
        special: 0,
    },
    Know {
        word: ["!second", "!door", "!thief", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF,
        temp: 25,
        answer: "The second locked door in the Thieves House? Don't go there!.",
        special: 0,
    },
    Know {
        word: ["!danger", "!thief", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF,
        temp: 25,
        answer: "What would you expect in the Thieves' House? Thieves of course. They're very poor fighters.",
        special: 0,
    },
    Know {
        word: ["!tell", "!thief", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF,
        temp: 25,
        answer: "What would you expect in the Thieves' House? Thieves of course. They're very poor fighters.",
        special: 0,
    },
    Know {
        word: ["!who", "!thief", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF,
        temp: 25,
        answer: "What would you expect in the Thieves' House? Thieves of course. They're very poor fighters.",
        special: 0,
    },
    Know {
        word: ["!why", "", "?want", "!amulet", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF,
        temp: 25,
        answer: "The thieves stole my amulet. I want it back.",
        special: 0,
    },
    Know {
        word: ["!what", "!amulet", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF,
        temp: 25,
        answer: "It's just a small, golden amulet.",
        special: 0,
    },
    Know {
        word: ["!tell", "!amulet", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF,
        temp: 25,
        answer: "It's just a small, golden amulet.",
        special: 0,
    },
    Know {
        word: ["!where", "?haunted", "!castle", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_CASTLE,
        temp: 28,
        answer: "The Haunted Castle is in the south-east corner of the city, on Castle Way.",
        special: 0,
    },
    Know {
        word: ["!locked", "!door", "?haunted", "?castle", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 50,
        area: AR_CASTLE,
        temp: 28,
        answer: "My guess is that you'll have to find the keys for the locked doors in the Haunted Castle, %s.",
        special: 0,
    },
    Know {
        word: ["!key", "?haunted", "?castle", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 50,
        area: AR_CASTLE,
        temp: 28,
        answer: "Not all walls are what they seem to be in the Haunted Castle, %s.",
        special: 0,
    },
    Know {
        word: ["!danger", "?haunted", "?castle", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_CASTLE,
        temp: 28,
        answer: "There are ghosts in the Haunted Castle. They'll curse you! It takes a Corporal to survive that.",
        special: 0,
    },
    Know {
        word: ["!tell", "?haunted", "?castle", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_CASTLE,
        temp: 28,
        answer: "There are ghosts in the Haunted Castle. They'll curse you! It takes a Corporal to survive that.",
        special: 0,
    },
    Know {
        word: ["!who", "!in", "?haunted", "?castle", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_CASTLE,
        temp: 28,
        answer: "There are ghosts in the Haunted Castle. They'll curse you! It takes a Corporal to survive that.",
        special: 0,
    },
    Know {
        word: ["!why", "?want", "!belt", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_CASTLE,
        temp: 28,
        answer: "I need the belt for my research.",
        special: 0,
    },
    Know {
        word: ["!what", "!belt", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_CASTLE,
        temp: 28,
        answer: "It's a golden belt with a magical enchantment.",
        special: 0,
    },
    Know {
        word: ["!tell", "!belt", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_CASTLE,
        temp: 28,
        answer: "It's a golden belt with a magical enchantment.",
        special: 0,
    },
    Know {
        word: ["!where", "?cursed", "!tomb", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_TOMB,
        temp: 50,
        answer: "The Cursed Tomb is in the north-east corner of the city, on Rose Street.",
        special: 0,
    },
    Know {
        word: ["!where", "!ruby", "?cursed", "?tomb", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_TOMB,
        temp: 50,
        answer: "The ruby is in a hidden area in the Cursed Tomb.",
        special: 0,
    },
    Know {
        word: ["!how", "!hidden", "?cursed", "?tomb", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 50,
        area: AR_TOMB,
        temp: 50,
        answer: "There are some illusionary walls in the Cursed Tomb.",
        special: 0,
    },
    Know {
        word: ["!danger", "?cursed", "?tomb", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_TOMB,
        temp: 50,
        answer: "The Cursed Tomb is inhibitated by skeletons. They're poor fighters, but you need strong armor to survive their magic.",
        special: 0,
    },
    Know {
        word: ["!tell", "?cursed", "?tomb", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_TOMB,
        temp: 50,
        answer: "The Cursed Tomb is inhibitated by skeletons. They're poor fighters, but you need strong armor to survive their magic.",
        special: 0,
    },
    Know {
        word: ["!who", "!in", "?cursed", "?tomb", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_TOMB,
        temp: 50,
        answer: "The Cursed Tomb is inhibitated by skeletons. They're poor fighters, but you need strong armor to survive their magic.",
        special: 0,
    },
    Know {
        word: ["!why", "?want", "!ruby", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_TOMB,
        temp: 50,
        answer: "The ruby will help me with my magical experiments.",
        special: 0,
    },
    Know {
        word: ["!what", "!ruby", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_TOMB,
        temp: 50,
        answer: "It's a gem with magical properties. Rubies are used to build magical items.",
        special: 0,
    },
    Know {
        word: ["!tell", "!ruby", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_TOMB,
        temp: 50,
        answer: "It's a gem with magical properties. Rubies are used to build magical items.",
        special: 0,
    },
    Know {
        word: ["!where", "!joe", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_JOE,
        temp: 64,
        answer: "Joe's House is in the middle of Castle Way.",
        special: 0,
    },
    Know {
        word: ["!danger", "?joe", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_JOE,
        temp: 64,
        answer: "It might be wise to assure that Joe will not get any help when you attack him.",
        special: 0,
    },
    Know {
        word: ["!tell", "?joe", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_JOE,
        temp: 64,
        answer: "It might be wise to assure that Joe will not get any help when you attack him.",
        special: 0,
    },
    Know {
        word: ["!why", "?want", "?bronze", "!armor",         "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_JOE,
        temp: 64,
        answer: "The bronze armor was a gift from my father. Joe stole it.",
        special: 0,
    },
    Know {
        word: ["!what", "?bronze", "!armor",         "?", "",  "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_JOE,
        temp: 64,
        answer: "The bronze armor was a gift from my father. Joe stole it.",
        special: 0,
    },
    Know {
        word: ["!tell", "?bronze", "!armor",         "?", "",  "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_JOE,
        temp: 64,
        answer: "The bronze armor was a gift from my father. Joe stole it.",
        special: 0,
    },
    Know {
        word: ["?black", "!stronghold", "!coin", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_STRONGHOLD,
        temp: 72,
        answer: "Bring me one of the black candles from the Stronghold, and I'll give you the star part of the coin.",
        special: 0,
    },
    Know {
        word: ["!help", "?need", "?stronghold", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_STRONGHOLD,
        temp: 518,
        answer: "Every few hours a horde of monsters from the Black Stronghold is attacking our outpost or the city entrance. If you could help PROTECT these places, or - even better - enter the STRONGHOLD and stop the monsters, we'd reward you.",
        special: 0,
    },
    Know {
        word: ["!protect", "?outpost", "?city", "?entrance", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_STRONGHOLD,
        temp: 518,
        answer: "Wait close to one of these points till the guards there shout alert. Then help them in the fight. They'll report your success and you can collect your REWARD here afterwards.",
        special: 0,
    },
    Know {
        word: ["!reward", "?protect", "?city", "?stronghold", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_STRONGHOLD,
        temp: 518,
        answer: "Depending on your success you will get money, potions or experience. Ask about your POINTS...",
        special: 0,
    },
    Know {
        word: ["!stronghold", "?enter", "?attack", "?black", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_STRONGHOLD,
        temp: 518,
        answer: "The monsters are coming from the Black Stronghold. Some evil magic is creating them. As far as we know, this magic needs black candles. So if you can bring us some black candles we will know you were successful and REWARD you.",
        special: 0,
    },
    Know {
        word: ["!points", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_STRONGHOLD,
        temp: 518,
        answer: "You have %d points. You can BUY GOLD at one coin per point, BUY HEALING potions for 6 points, BUY MANA potions for 9 points or BUY EXPERIENCE at %d exp per point.",
        special: POINTS,
    },
    Know {
        word: ["!buy", "!gold", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_STRONGHOLD,
        temp: 518,
        answer: "",
        special: BUYGOLD,
    },
    Know {
        word: ["!buy", "!health", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_STRONGHOLD,
        temp: 518,
        answer: "",
        special: BUYHEALTH,
    },
    Know {
        word: ["!buy", "!healing", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_STRONGHOLD,
        temp: 518,
        answer: "",
        special: BUYHEALTH,
    },
    Know {
        word: ["!buy", "!mana", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_STRONGHOLD,
        temp: 518,
        answer: "",
        special: BUYMANA,
    },
    Know {
        word: ["!buy", "!exp", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_STRONGHOLD,
        temp: 518,
        answer: "",
        special: BUYEXP,
    },
    Know {
        word: ["!where", "!mine", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_MINE,
        temp: 0,
        answer: "The mine is north of the Temple of Skua.",
        special: 0,
    },
    Know {
        word: ["!danger", "!mine", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_MINE,
        temp: 0,
        answer: "Mining can be dangerous. Place supporting beams to prevent it from collapsing. And don't go into the lower levels too early.",
        special: 0,
    },
    Know {
        word: ["!tell", "!mine", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_MINE,
        temp: 0,
        answer: "The mine was opened only a few years ago. At first, it was a very profitable business. But the workers fled when some of them were killed by skeletons.",
        special: 0,
    },
    Know {
        word: ["!where", "?skeleton", "!lord", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_SLORD,
        temp: 90,
        answer: "The Skeleton Lord is in the dark, large building in the middle of Merchants Way.",
        special: 0,
    },
    Know {
        word: ["!where", "!potion", "?skeleton", "?lord", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 50,
        area: AR_SLORD,
        temp: 90,
        answer: "The potion the Skeleton Lord has could be in a hidden area.",
        special: 0,
    },
    Know {
        word: ["!danger", "?skeleton", "?lord", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 50,
        area: AR_SLORD,
        temp: 90,
        answer: "The Skeleton Lord still has some guards who will protect him. A bold Sergeant should be able to kill them.",
        special: 0,
    },
    Know {
        word: ["!tell", "?skeleton", "?lord", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 50,
        area: AR_SLORD,
        temp: 90,
        answer: "The Skeleton Lord still has some guards who will protect him. A bold Sergeant should be able to kill them.",
        special: 0,
    },
    Know {
        word: ["!who", "!in", "?skeleton", "?lord", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 50,
        area: AR_SLORD,
        temp: 90,
        answer: "The Skeleton Lord still has some guards who will protect him. A bold Sergeant should be able to kill them.",
        special: 0,
    },
    Know {
        word: ["!why", "?want", "!potion", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_SLORD,
        temp: 90,
        answer: "I created the potion when the lord was still alive. Now that he turned into a skeleton, he has no more need for it.",
        special: 0,
    },
    Know {
        word: ["!what", "!potion", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_SLORD,
        temp: 90,
        answer: "It's a greater healing potion. I created the potion when the lord was still alive.",
        special: 0,
    },
    Know {
        word: ["!tell", "!potion", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_SLORD,
        temp: 90,
        answer: "It's a greater healing potion. I created the potion when the lord was still alive.",
        special: 0,
    },
    Know {
        word: ["!where", "?templar", "!outlaw", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_OUTLAW,
        temp: 91,
        answer: "The Templar Outlaws live in a fortified building in the south-west corner of Aston.",
        special: 0,
    },
    Know {
        word: ["!danger", "?templar", "?outlaw", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_OUTLAW,
        temp: 91,
        answer: "The Templar Outlaws are very skilled fighters, and there are many of them. A Staff Sergeant could try to attack them.",
        special: 0,
    },
    Know {
        word: ["!tell", "?templar", "?outlaw", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_OUTLAW,
        temp: 91,
        answer: "The Templar Outlaws are very skilled fighters, and there are many of them. A Staff Sergeant could try to attack them.",
        special: 0,
    },
    Know {
        word: ["!why", "?want", "?barbarian", "!sword", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_OUTLAW,
        temp: 91,
        answer: "It's a good weapon. I like it. And the templars are outlaws, taking from them is not stealing...",
        special: 0,
    },
    Know {
        word: ["!what", "?barbarian", "!sword", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_OUTLAW,
        temp: 91,
        answer: "The barbarian sword is an old weapon. It was made by an now extinct race of barbarians.",
        special: 0,
    },
    Know {
        word: ["!tell", "?barbarian", "!sword", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_OUTLAW,
        temp: 91,
        answer: "The barbarian sword is an old weapon. It was made by an now extinct race of barbarians.",
        special: 0,
    },
    Know {
        word: ["?second", "!door", "?thief", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF2,
        temp: 107,
        answer: "The second locked door in the Thieves House? You might have to pick the lock.",
        special: 0,
    },
    Know {
        word: ["!danger", "?thief", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF2,
        temp: 107,
        answer: "Be careful behind the second locked door in the Thieves House. The thieves there know how to fight.",
        special: 0,
    },
    Know {
        word: ["!tell", "?thief", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF2,
        temp: 107,
        answer: "Be careful behind the second locked door in the Thieves House. The thieves there know how to fight.",
        special: 0,
    },
    Know {
        word: ["!who", "?thief", "?house", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF2,
        temp: 107,
        answer: "Be careful behind the second locked door in the Thieves House. The thieves there know how to fight.",
        special: 0,
    },
    Know {
        word: ["!why", "?want", "?ruby", "!amulet", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF2,
        temp: 107,
        answer: "The Ruby Amulet belongs to me. A few days after I showed it to Jamil it vanished. I'm sure the thieves took it.",
        special: 0,
    },
    Know {
        word: ["!what", "?ruby", "!amulet", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF2,
        temp: 107,
        answer: "It's a small golden amulet with a big ruby on it. It increases your ability to cast magic spells.",
        special: 0,
    },
    Know {
        word: ["!tell", "?ruby", "!amulet", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_THIEF2,
        temp: 107,
        answer: "It's a small golden amulet with a big ruby on it. It increases your ability to cast magic spells.",
        special: 0,
    },
    Know {
        word: ["!where", "?magic", "!maze", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_MMAZE,
        temp: 108,
        answer: "The Magic Maze is in the north-west corner of Aston.",
        special: 0,
    },
    Know {
        word: ["!danger", "?magic", "?maze", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 50,
        area: AR_MMAZE,
        temp: 108,
        answer: "The Magic Maze is full of traps. Be careful and look for all clues you can get. There's also a powerful sorceress you will have to fight.",
        special: 0,
    },
    Know {
        word: ["!tell", "?magic", "?maze", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 50,
        area: AR_MMAZE,
        temp: 108,
        answer: "The Magic Maze is full of traps. Be careful and look for all clues you can get. There's also a powerful sorceress you will have to fight.",
        special: 0,
    },
    Know {
        word: ["!who", "?magic", "?maze", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 50,
        area: AR_MMAZE,
        temp: 108,
        answer: "The Magic Maze is full of traps. Be careful and look for all clues you can get. There's also a powerful sorceress you will have to fight.",
        special: 0,
    },
    Know {
        word: ["!tell", "sorceress", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 50,
        area: AR_MMAZE,
        temp: 108,
        answer: "The sorceress is called Jane. I don't know much about her, only that she killed a few people who survived her maze.",
        special: 0,
    },
    Know {
        word: ["!why", "?want", "!potion", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_MMAZE,
        temp: 108,
        answer: "It's a very strong mana potion and I need it to complete a certain magic spell I've been studying for several years now.",
        special: 0,
    },
    Know {
        word: ["!what", "!potion", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_MMAZE,
        temp: 108,
        answer: "It's a very strong mana potion.",
        special: 0,
    },
    Know {
        word: ["!tell", "!potion", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 25,
        area: AR_MMAZE,
        temp: 108,
        answer: "It's a very strong mana potion.",
        special: 0,
    },
    Know {
        word: ["!where", "?stone", "!sword", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 109,
        answer: "Now where was that damn stone? Let me think. Ah! I remember. It's over there, in that corner.",
        special: 0,
    },
    Know {
        word: ["!where", "!stone", "?sword", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 109,
        answer: "Now where was that damn stone? Let me think. Ah! I remember. It's over there, in that corner.",
        special: 0,
    },
    Know {
        word: ["?too", "?weak", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 109,
        answer: "If you're not strong enough to take the sword now, you might need some form of enchantment. Why not go to Damor's Shop and see if he can help?",
        special: 0,
    },
    Know {
        word: ["?not", "?strong", "?enough", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 109,
        answer: "If you're not strong enough to take the sword now, you might need some form of enchantment. Why not go to Damor's Shop and see if he can help?",
        special: 0,
    },
    Know {
        word: ["?how", "!take", "!sword", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 109,
        answer: "If you're not strong enough to take the sword now, you might need some form of enchantment. Why not go to Damor's Shop and see if he can help?",
        special: 0,
    },
    Know {
        word: ["?how", "!get", "!sword", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 109,
        answer: "If you're not strong enough to take the sword now, you might need some form of enchantment. Why not go to Damor's Shop and see if he can help?",
        special: 0,
    },
    Know {
        word: ["!why", "?want", "?stone", "!sword", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 109,
        answer: "I've been staring at this stone for years. Can't you imagine I want the matter settled so I can throw the damn stone away?",
        special: 0,
    },
    Know {
        word: ["!what", "?stone", "!sword", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 109,
        answer: "The sword in the stone in that corner.",
        special: 0,
    },
    Know {
        word: ["!tell", "?stone", "!sword", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 109,
        answer: "The sword in the stone in that corner.",
        special: 0,
    },
    Know {
        word: ["!how", "?create", "?mix", "?potion", "?life", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 111,
        answer: "If I knew how to mix the Potion of Life, I'd do it myself. All I can tell you is that you need three rare flowers.",
        special: 0,
    },
    Know {
        word: ["!what", "!ingredients", "?potion", "?life", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 111,
        answer: "All I can tell you is that you need three rare flowers to mix the Potion of Life.",
        special: 0,
    },
    Know {
        word: ["!why", "?want", "!potion", "?life", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 111,
        answer: "I'm sick. The Potion of Life would cure me.",
        special: 0,
    },
    Know {
        word: ["!where", "!potion", "?life", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 111,
        answer: "I don't think you can find the Potion of life somewhere. You'll have to create it.",
        special: 0,
    },
    Know {
        word: ["!how", "?first", "?door", "?grolm", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 50,
        area: AR_GROLM,
        temp: 114,
        answer: "You need a key to get through the first door in the Grolm Gorge.",
        special: 0,
    },
    Know {
        word: ["!how", "!second", "?door", "?grolm", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 75,
        area: AR_GROLM,
        temp: 114,
        answer: "You need a crown to get through the second door in the Grolm Gorge.",
        special: 0,
    },
    Know {
        word: ["!how", "!third", "?door", "?grolm", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 75,
        area: AR_GROLM,
        temp: 114,
        answer: "You need a trident to get through the third door in the Grolm Gorge.",
        special: 0,
    },
    Know {
        word: ["!what", "!second", "?door", "?grolm", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 75,
        area: AR_GROLM,
        temp: 114,
        answer: "You need a crown to get through the second door in the Grolm Gorge.",
        special: 0,
    },
    Know {
        word: ["!what", "!third", "?door", "?grolm", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 75,
        area: AR_GROLM,
        temp: 114,
        answer: "You need a trident to get through the third door in the Grolm Gorge.",
        special: 0,
    },
    Know {
        word: ["!where", "!key", "?grolm", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_GROLM,
        temp: 114,
        answer: "The key for the first door in the Grolm Gorge is in a very hot place.",
        special: 0,
    },
    Know {
        word: ["!where", "!crown", "?grolm", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_GROLM,
        temp: 114,
        answer: "The crown from the Grolm Gorge? I'd expect the king wears it.",
        special: 0,
    },
    Know {
        word: ["!where", "!trident", "?grolm", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_GROLM,
        temp: 114,
        answer: "The trident from the Grolm Gorge? The Grolm Mages are said to use tridents.",
        special: 0,
    },
    Know {
        word: ["!what", "!key", "?grolm", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_GROLM,
        temp: 114,
        answer: "The key is used to open the first door.",
        special: 0,
    },
    Know {
        word: ["!what", "!crown", "?grolm", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_GROLM,
        temp: 114,
        answer: "The crown from the Grolm Gorge? It's used to open the second door.",
        special: 0,
    },
    Know {
        word: ["!what", "!trident", "?grolm", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_GROLM,
        temp: 114,
        answer: "The trident from the Grolm Gorge? It's used to open the third door.",
        special: 0,
    },
    Know {
        word: ["!how", "?first", "?door", "?lizard", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 50,
        area: AR_LIZARD,
        temp: 162,
        answer: "You need a key to get through the first door in the Lizard Gorge.",
        special: 0,
    },
    Know {
        word: ["!how", "!second", "?door", "?lizard", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 75,
        area: AR_LIZARD,
        temp: 162,
        answer: "You need a key to get through the second door in the Lizard Gorge.",
        special: 0,
    },
    Know {
        word: ["!how", "!third", "?door", "?lizard", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 75,
        area: AR_LIZARD,
        temp: 162,
        answer: "You need a key to get through the third door in the Lizard Gorge.",
        special: 0,
    },
    Know {
        word: ["!where", "!key", "?lizard", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_LIZARD,
        temp: 162,
        answer: "The Merchants here will give you the keys for special items. Go to them to learn which items you need.",
        special: 0,
    },
    Know {
        word: ["!where", "!coconut", "?lizard", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_LIZARD,
        temp: 162,
        answer: "The coconut? Well, coconuts grow in trees, you know?",
        special: 0,
    },
    Know {
        word: ["!where", "!potion", "?agility", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_LIZARD,
        temp: 162,
        answer: "You have to mix the Potion of Superior Agility from some flower which grow here.",
        special: 0,
    },
    Know {
        word: ["!where", "!teeth", "?lizard", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_LIZARD,
        temp: 162,
        answer: "Lizard's Teeth? I'd assume they have them in their mouths.",
        special: 0,
    },
    Know {
        word: ["!how", "?teeth", "!necklace", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_LIZARD,
        temp: 162,
        answer: "To create the Lizard's Teeth Necklace, you need a leather string and teeth. One of the Merchants sells leather strings.",
        special: 0,
    },
    Know {
        word: ["?ratling", "!eye", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 246,
        answer: "Yes. I need them to create a very powerful stimulant. If you want to help me, buy a collector in Damor's shop and bring me a full set of Ratling's Eyes.",
        special: 0,
    },
    Know {
        word: ["!ratling", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 246,
        answer: "The Ratlings live below the city, in the Underground. They look like humans, except for the head, which resembles that of a rat. Be careful, some of the are very strong, and all of them can see in the dark.",
        special: 0,
    },
    Know {
        word: ["!stimulant", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 246,
        answer: "The stimulant will be extremely powerful. I'll be able to create two potions of it. You'll get one of them as payment.",
        special: 0,
    },
    Know {
        word: ["!potion", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 246,
        answer: "The potion will be extremely powerful stimulant. I'll be able to create two potions of it. You'll get one of them as payment.",
        special: 0,
    },
    Know {
        word: ["!payment", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 246,
        answer: "I can create two potions from a complete set of eyes. I'll give you one of them as payment.",
        special: 0,
    },
    Know {
        word: ["!how", "powerful", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 246,
        answer: "Very.",
        special: 0,
    },
    Know {
        word: ["!underground", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 246,
        answer: "The City Guards have recently discovered some holes in the ground of several buildings. They lead into a maze of rooms and corridors. It seems the thieves are using them too.",
        special: 0,
    },
    Know {
        word: ["!thief", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 246,
        answer: "A thief was trying to sell information about Azraels Helmet. A City Guard was chasing him, but he vanished in the Thief House. Later searches showed he used a hidden hole in the floor to escape.",
        special: 0,
    },
    Know {
        word: ["!azrael", "?helm", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 246,
        answer: "Lord Azrael of Aston once ruled this city. But he vanished during a fight in his castle. He possesed a powerful helmet, the Helm of Shadows.",
        special: 0,
    },
    Know {
        word: ["!castle", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 246,
        answer: "It is now the Haunted Castle. After Azrael vanished, it was abandoned. Now it's inhabitated by ghosts, and some say they hear Azraels cry in there.",
        special: 0,
    },
    Know {
        word: ["!helm", "?shadow", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 246,
        answer: "Lord Azrael's famous helmet, the Helm of Shadows. It's enhanced with powerful magic.",
        special: 0,
    },
    Know {
        word: ["?ratling", "!eye", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 343,
        answer: "Yes. I need it to complete my collection. Robin wants a full set and he promised be a powerful potion in exchange.",
        special: 0,
    },
    Know {
        word: ["!ratling", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 343,
        answer: "The Ratlings live below the city, in the Underground. They look like humans, except for the head, which resembles that of a rat. Be careful, some of the are very strong, and all of them can see in the dark.",
        special: 0,
    },
    Know {
        word: ["!underground", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 343,
        answer: "The City Guards have recently discovered some holes in the ground of several buildings. They lead into a maze of rooms and corridors. It seems the thieves are using them too.",
        special: 0,
    },
    Know {
        word: ["!thief", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 343,
        answer: "A thief was trying to sell information about Azraels Helmet. A City Guard was chasing him, but he vanished in the Thief House. Later searches showed he used a hidden hole in the floor to escape.",
        special: 0,
    },
    Know {
        word: ["!azrael", "?helm", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 343,
        answer: "Lord Azrael of Aston once ruled this city. But he vanished during a fight in his castle. He possesed a powerful helmet, the Helm of Shadows.",
        special: 0,
    },
    Know {
        word: ["!castle", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 343,
        answer: "It is now the Haunted Castle. After Azrael vanished, it was abandoned. Now it's inhabitated by ghosts, and some say they hear Azraels cry in there.",
        special: 0,
    },
    Know {
        word: ["!helm", "?shadow", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 100,
        area: AR_UNDER_I,
        temp: 343,
        answer: "Lord Azrael's famous helmet, the Helm of Shadows. It's enhanced with powerful magic.",
        special: 0,
    },
    Know {
        word: ["?what", "!bartering", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "Bartering will help you to get better prices from merchants.",
        special: 0,
    },
    Know {
        word: ["?what", "!enchant", "!weapon", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "Enchant Weapon is a magic spell. It will make your weapon stronger when you use it.",
        special: 0,
    },
    Know {
        word: ["?what", "!recall", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "Recall is a magic spell. It will teleport you back to the Temple of Skua when you use it. But beware, there is a small delay between casting and teleport.",
        special: 0,
    },
    Know {
        word: ["?what", "!repair", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "Repair? That's the ability to repair your equipment.",
        special: 0,
    },
    Know {
        word: ["?what", "!stun", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "Stun is a magic spell. If you can overcome your target's Resistance, he'll be unable to move for a few seconds.",
        special: 0,
    },
    Know {
        word: ["?what", "!lockpicking", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "Lock-Picking?. It's used to pick locks, you know?",
        special: 0,
    },
    Know {
        word: ["?what", "!identify", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "Identify is a magic spell. It'll give you some information about an item or person.",
        special: 0,
    },
    Know {
        word: ["?what", "!resistance", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "Resistance is used against magic spells. If you're good at it, it's much harder to Curse or Stun you.",
        special: 0,
    },
    Know {
        word: ["?what", "!bless", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "Bless is a powerful spell. It increases all your abilities.",
        special: 0,
    },
    Know {
        word: ["?what", "!curse", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "Curse is a powerful spell. It decreases all your abilities.",
        special: 0,
    },
    Know {
        word: ["?what", "!guardian", "!angel", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "The spell Guardian Angel will lessen the effects of death on you. You can buy it from Damor, in Aston.",
        special: 0,
    },
    Know {
        word: ["?what", "!heal", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "Heal is a magic spell. You can use it to heal a persons injuries.",
        special: 0,
    },
    Know {
        word: ["?what", "!gate", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "The last gate of the Labyrinth.",
        special: 0,
    },
    Know {
        word: ["?what", "!labyrinth", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "The Labyrinth. It's a huge maze full of dangers. If you survive it, you'll become a Seyan'Du.",
        special: 0,
    },
    Know {
        word: ["?what", "!seyandu", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "The Seyan'Du are very powerful. They do not have the limitations other people have.",
        special: 0,
    },
    Know {
        word: ["?what", "!limitation", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "The Harakim are powerful sorcerers, the Templars strong fighters and the Mercenaries have a bit of both. But the Seyan'Du combine their abilities.",
        special: 0,
    },
    Know {
        word: ["?what", "!templar", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "The Templars are powerful fighters. But they're not very good with magic.",
        special: 0,
    },
    Know {
        word: ["?what", "!harakim", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "The Harakim are spellcasters. They don't fight very well.",
        special: 0,
    },
    Know {
        word: ["?what", "!mercenary", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "A Mercenary is both, a fighter and a spellcaster.",
        special: 0,
    },
    Know {
        word: ["!who", "!skua", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "Skua is the god of justice and order. He fights a perpetual battle against the Purple One.",
        special: 0,
    },
    Know {
        word: ["!who", "!purple", "!one", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "The Purple One? He's the god of chaos and disorder.",
        special: 0,
    },
    Know {
        word: ["?what", "!order", "?purple", "?one", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_PURPLE,
        temp: 180,
        answer: "Our order, the Cult of the Purple One, does not believes in rules. Join us, and you can do whatever you want.",
        special: 0,
    },
    Know {
        word: ["?tell", "!order", "?purple", "?one", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_PURPLE,
        temp: 180,
        answer: "Our order, the Cult of the Purple One, does not believes in rules. Join us, and you can do whatever you want.",
        special: 0,
    },
    Know {
        word: ["?how", "!join", "?order", "?purple", "?one", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_PURPLE,
        temp: 180,
        answer: "If you join us, you will be able to kill your fellow players. But beware! Others can kill you as well, and this decision is irrevocable! Do you want to join?",
        special: 0,
    },
    Know {
        word: ["?what", "!happens", "!join", "?order", "?purple", "?one", "?", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_PURPLE,
        temp: 180,
        answer: "If you join us, you will be able to kill your fellow players. But beware! Others can kill you as well, and this decision is irrevocable! Do you want to join?",
        special: 0,
    },
    Know {
        word: ["!yes", "!join", "?want", "!", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_PURPLE,
        temp: 180,
        answer: "So be it. But you have to pass a test first: Kill me!",
        special: 0,
    },
    Know {
        word: ["!yes", "!", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_PURPLE,
        temp: 180,
        answer: "So be it. But you have to pass a test first: Kill me!",
        special: 0,
    },
    Know {
        word: ["!kill", "?you", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_PURPLE,
        temp: 180,
        answer: "Yes. To join, you have to kill me. Go ahead, you coward!",
        special: 0,
    },
    Know {
        word: ["!poem", "?first", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_KNIGHT,
        temp: 317,
        answer: "Where the sun rises in a clouded sky     A mere touch will reveal where hidden lie       A skull which, when enlighted, will give thee    A brave enemy and a precious key.",
        special: 0,
    },
    Know {
        word: ["!poem", "?second", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_KNIGHT,
        temp: 317,
        answer: "A dark sky, a large tree                 Weavers work in crimson                         The corner of which guides thee                  To key's holder, the lord's son.",
        special: 0,
    },
    Know {
        word: ["!second", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_KNIGHT,
        temp: 317,
        answer: "A dark sky, a large tree                 Weavers work in crimson                         The corner of which guides thee                 To key's holder, the lord's son.",
        special: 0,
    },
    Know {
        word: ["!next", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_KNIGHT,
        temp: 317,
        answer: "A dark sky, a large tree                 Weavers work in crimson                         The corner of which guides thee                  To key's holder, the lord's son.",
        special: 0,
    },
    Know {
        word: ["!other", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_KNIGHT,
        temp: 317,
        answer: "A dark sky, a large tree                 Weavers work in crimson                         The corner of which guides thee                  To key's holder, the lord's son.",
        special: 0,
    },
    Know {
        word: ["?ice", "!egg", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_NEST,
        temp: 615,
        answer: "I heard that the ice gargoyles guard an ice egg in their nest. I'd be most grateful if you could bring it to me. But hurry, before it melts!",
        special: 0,
    },
    Know {
        word: ["?ice", "!cloak", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_NEST,
        temp: 615,
        answer: "The Ice Cloak is a fine piece of armor, but it melts in due time. I can give you one if you obtain an ice egg for me.",
        special: 0,
    },
    Know {
        word: ["!how", "!are", "!you", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "",
        special: HEALTH,
    },
    Know {
        word: ["!who", "!are", "!you", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "",
        special: WHOAMI,
    },
    Know {
        word: ["!where", "!are", "!you", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "",
        special: WHERE,
    },
    Know {
        word: ["!where", "!am", "!i", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "",
        special: WHERE,
    },
    Know {
        word: ["!buy", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "",
        special: SHOP,
    },
    Know {
        word: ["!sell", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "",
        special: SHOP,
    },
    Know {
        word: ["!shop", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "",
        special: SHOP,
    },
    Know {
        word: ["!exit", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "Enter the backroom of a tavern to leave the game, %s.",
        special: 0,
    },
    Know {
        word: ["!hello", "!", "$", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "",
        special: GREET,
    },
    Know {
        word: ["!bye", "!", "$", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "Goodbye, %s.",
        special: 0,
    },
    Know {
        word: ["!thank", "?you", "!", "$", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "You're welcome, %s.",
        special: 0,
    },
    Know {
        word: ["!thank", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "You're welcome, %s.",
        special: 0,
    },
    Know {
        word: ["!stop", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: CT_COMPANION,
        answer: "",
        special: STOP,
    },
    Know {
        word: ["!move", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: CT_COMPANION,
        answer: "",
        special: MOVE,
    },
    Know {
        word: ["!attack", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: CT_COMPANION,
        answer: "",
        special: ATTACK,
    },
    Know {
        word: ["!wait", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: CT_COMPANION,
        answer: "",
        special: WAIT,
    },
    Know {
        word: ["!follow", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: CT_COMPANION,
        answer: "",
        special: FOLLOW,
    },
    Know {
        word: ["!transfer", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: CT_COMPANION,
        answer: "",
        special: TRANSFER,
    },
    Know {
        word: ["!geronimo", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: CT_COMPANION,
        answer: "",
        special: SPELLINFO,
    },
    Know {
        word: ["!time", "!what", "?", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: 0,
        answer: "",
        special: TIME,
    },
    Know {
        word: ["!quiet", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 0,
        area: AR_GENERAL,
        temp: CT_COMPANION,
        answer: "",
        special: QUIET,
    },
    Know {
        word: ["!riddle", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_RIDDLE1,
        temp: 899,
        answer: RIDDLE_TEXT,
        special: 0,
    },
    Know {
        word: ["!riddles", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_RIDDLE1,
        temp: 899,
        answer: RIDDLE_TEXT,
        special: 0,
    },
    Know {
        word: ["!riddle", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_RIDDLE2,
        temp: 905,
        answer: RIDDLE_TEXT,
        special: 0,
    },
    Know {
        word: ["!riddles", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_RIDDLE2,
        temp: 905,
        answer: RIDDLE_TEXT,
        special: 0,
    },
    Know {
        word: ["!riddle", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_RIDDLE3,
        temp: 911,
        answer: RIDDLE_TEXT,
        special: 0,
    },
    Know {
        word: ["!riddles", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_RIDDLE3,
        temp: 911,
        answer: RIDDLE_TEXT,
        special: 0,
    },
    Know {
        word: ["!riddle", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_RIDDLE4,
        temp: 912,
        answer: RIDDLE_TEXT,
        special: 0,
    },
    Know {
        word: ["!riddles", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_RIDDLE4,
        temp: 912,
        answer: RIDDLE_TEXT,
        special: 0,
    },
    Know {
        word: ["!riddle", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_RIDDLE5,
        temp: 913,
        answer: RIDDLE_TEXT,
        special: 0,
    },
    Know {
        word: ["!riddles", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
        value: 10,
        area: AR_RIDDLE5,
        temp: 913,
        answer: RIDDLE_TEXT,
        special: 0,
    },
];

use crate::god::God;
use crate::repository::Repository;
use crate::state::State;

/// Port of `obey(int cn, int co)` from `talk.cpp`
/// Returns:
/// - 1 if co is the master of cn (companion)
/// - 2 if cn will obey co based on kindred/data flags
/// - 0 otherwise
pub fn obey(cn: usize, co: usize) -> i32 {
    Repository::with_characters(|characters| {
        // Check if co is the companion master (data[63])
        if characters[cn].data[63] == co as i32 {
            return 1;
        }
        // Check kindred and obedience flags (data[26] & data[28])
        if (characters[cn].data[26] & characters[co].kindred as i32) != 0
            && (characters[cn].data[28] & 1) != 0
        {
            return 2;
        }
        0
    })
}

/// Port of `answer_spellinfo(int cn, int co)` from `talk.cpp`
/// Lists all active spells on the companion
pub fn answer_spellinfo(cn: usize, co: usize) {
    if obey(cn, co) != 1 {
        return;
    }

    Repository::with_characters(|characters| {
        Repository::with_items(|items| {
            let mut found = false;

            for n in 0..20 {
                let spell_idx = characters[cn].spell[n] as usize;
                if spell_idx != 0 && core::types::Item::is_sane_item(spell_idx) {
                    let minutes = items[spell_idx].active / (18 * 60);
                    let seconds = (items[spell_idx].active / 18) % 60;
                    let name = items[spell_idx].get_name();

                    State::with(|state| {
                        state.do_sayx(cn, &format!("{}, for {}m {}s.", name, minutes, seconds));
                    });
                    found = true;
                }
            }

            if !found {
                State::with(|state| {
                    state.do_sayx(cn, "I have no spells on me at the moment.");
                });
            }
        });
    });
}

/// Port of `answer_transfer(int cn, int co)` from `talk.cpp`
/// Transfers companion's experience to master and destroys the companion
pub fn answer_transfer(cn: usize, co: usize) {
    if obey(cn, co) != 1 {
        return;
    }

    let (companion_name, exp_to_give, _cn_x, _cn_y, _co_x, _co_y) =
        Repository::with_characters(|characters| {
            (
                characters[cn].get_name().to_string(),
                characters[cn].data[28],
                characters[cn].x,
                characters[cn].y,
                characters[co].x,
                characters[co].y,
            )
        });

    State::with(|state| {
        state.do_sayx(
            cn,
            &format!(
                "I'd prefer to die in battle, {}. But I shall obey my master.",
                Repository::with_characters(|ch| ch[co].get_name().to_string())
            ),
        );
    });

    // Add visual effects (fx_add_effect equivalent - would need to be implemented)
    // fx_add_effect(6, 0, co_x, co_y, 0);
    // fx_add_effect(7, 0, cn_x, cn_y, 0);

    // Give experience (do_give_exp equivalent - would need to be implemented)
    // do_give_exp(co, exp_to_give, 1, -1);

    // Die companion (die_companion equivalent - would need to be implemented)
    // die_companion(cn);

    Repository::with_characters_mut(|characters| {
        if characters[co].luck > 0 {
            characters[co].luck -= 1;
        }
    });

    log::info!(
        "Character {} transferred companion {} (exp: {})",
        co,
        companion_name,
        exp_to_give
    );
}

/// Port of `answer_follow(int cn, int co)` from `talk.cpp`
/// Makes companion follow the master
pub fn answer_follow(cn: usize, co: usize) {
    if obey(cn, co) != 1 {
        return;
    }

    Repository::with_characters_mut(|characters| {
        // Clear data[80..92]
        for n in 80..92 {
            characters[cn].data[n] = 0;
        }

        characters[cn].attack_cn = 0;
        characters[cn].goto_x = 0;
        characters[cn].goto_y = 0;
        characters[cn].misc_action = 0;

        characters[cn].data[69] = co as i32; // Set follow target
        characters[cn].data[29] = 0; // Clear wait position

        let co_name = characters[co].get_name().to_string();
        State::with(|state| {
            state.do_sayx(cn, &format!("Yes, {}!", co_name));
        });
    });
}

/// Port of `answer_wait(int cn, int co)` from `talk.cpp`
/// Makes companion wait at current position
pub fn answer_wait(cn: usize, co: usize) {
    if obey(cn, co) != 1 {
        return;
    }

    Repository::with_characters_mut(|characters| {
        // Clear data[80..92]
        for n in 80..92 {
            characters[cn].data[n] = 0;
        }

        characters[cn].attack_cn = 0;
        characters[cn].goto_x = 0;
        characters[cn].goto_y = 0;
        characters[cn].misc_action = 0;

        // Set wait position and direction
        let x = characters[cn].x as i32;
        let y = characters[cn].y as i32;
        let dir = characters[cn].dir;

        characters[cn].data[29] = x + y * core::constants::SERVER_MAPX;
        characters[cn].data[30] = dir as i32;
        characters[cn].data[69] = 0; // Clear follow target

        let co_name = characters[co].get_name().to_string();
        State::with(|state| {
            state.do_sayx(cn, &format!("Yes, {}!", co_name));
        });
    });
}

/// Port of `answer_stop(int cn, int co)` from `talk.cpp`
/// Makes companion stop current action
pub fn answer_stop(cn: usize, co: usize) {
    if obey(cn, co) == 0 {
        return;
    }

    Repository::with_characters_mut(|characters| {
        // Clear data[80..92]
        for n in 80..92 {
            characters[cn].data[n] = 0;
        }

        characters[cn].attack_cn = 0;
        characters[cn].goto_x = 0;
        characters[cn].goto_y = 0;
        characters[cn].misc_action = 0;
        characters[cn].data[78] = 0;

        let ticker = Repository::with_globals(|globals| globals.ticker);
        characters[cn].data[27] = ticker as i32;

        let co_name = characters[co].get_name().to_string();
        State::with(|state| {
            state.do_sayx(cn, &format!("Yes master {}!", co_name));
        });
    });
}

/// Port of `answer_move(int cn, int co)` from `talk.cpp`
/// Makes companion move randomly nearby
pub fn answer_move(cn: usize, co: usize) {
    if obey(cn, co) == 0 {
        return;
    }

    Repository::with_characters_mut(|characters| {
        let cn_x = characters[cn].x;
        let cn_y = characters[cn].y;

        let mut rng = rand::thread_rng();
        use rand::Rng;

        characters[cn].attack_cn = 0;
        characters[cn].goto_x = ((cn_x as i32 + 4 - rng.gen_range(0..9))
            .max(0)
            .min(core::constants::SERVER_MAPX - 1)) as u16;
        characters[cn].goto_y = ((cn_y as i32 + 4 - rng.gen_range(0..9))
            .max(0)
            .min(core::constants::SERVER_MAPY - 1)) as u16;
        characters[cn].misc_action = 0;

        let co_name = characters[co].get_name().to_string();
        State::with(|state| {
            state.do_sayx(cn, &format!("Yes master {}!", co_name));
        });
    });
}

/// Port of `answer_attack(int cn, int co, char* text)` from `talk.cpp`
/// Makes companion attack a specified target
pub fn answer_attack(cn: usize, co: usize, text: &str) {
    if obey(cn, co) == 0 {
        return;
    }

    // Extract target name from text
    // Skip non-alphabetic characters then whitespace
    let mut start_pos = 0;
    for (i, c) in text.chars().enumerate() {
        if c.is_alphabetic() {
            start_pos = i;
            break;
        }
    }

    // Skip past the first alphabetic character and any following non-whitespace
    let mut found_space = false;
    for (i, c) in text[start_pos..].chars().enumerate() {
        if c.is_whitespace() {
            found_space = true;
        } else if found_space {
            start_pos += i;
            break;
        }
    }

    let remaining = &text[start_pos..];
    let target_name: String = remaining
        .chars()
        .take(45)
        .take_while(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();

    if target_name.trim().is_empty() {
        return;
    }

    let target_name_lower = target_name.to_lowercase();

    // Find closest matching character
    let (best_target, best_dist) = Repository::with_characters(|characters| {
        let cn_x = characters[cn].x as i32;
        let cn_y = characters[cn].y as i32;
        let mut best = 9999;
        let mut bestn = 0;

        for n in 1..core::constants::MAXCHARS {
            if characters[n].used != core::constants::USE_ACTIVE {
                continue;
            }
            if characters[n].flags & core::constants::CharacterFlags::CF_BODY.bits() != 0 {
                continue;
            }

            let char_name_lower = characters[n].get_name().to_lowercase();
            if char_name_lower == target_name_lower {
                let dist =
                    (cn_x - characters[n].x as i32).abs() + (cn_y - characters[n].y as i32).abs();
                if dist < best {
                    best = dist;
                    bestn = n;
                }
            }
        }
        (bestn, best)
    });

    if best_target != 0 && best_dist < 40 {
        Repository::with_characters(|characters| {
            // Prevent attacks on self
            if best_target == co {
                State::with(|state| {
                    state.do_sayx(
                        cn,
                        &format!(
                            "But {}, I would never attack you!",
                            characters[co].get_name()
                        ),
                    );
                });
                return;
            }
            if best_target == cn {
                State::with(|state| {
                    state.do_sayx(
                        cn,
                        &format!(
                            "You want me to attack myself? That's silly, {}!",
                            characters[co].get_name()
                        ),
                    );
                });
                return;
            }

            // Check if attack is allowed (may_attack_msg equivalent - would need to be implemented)
            // if !may_attack_msg(co, best_target, 0) {
            //     State::with(|state| {
            //         state.do_sayx(cn, &format!("The Gods would be angry if we did that, you didn't want to anger the Gods, {} did you?", characters[co].get_name()));
            //     });
            //     return;
            // }

            let target_name = characters[best_target].get_name().to_string();
            let co_name = characters[co].get_name().to_string();

            Repository::with_characters_mut(|characters_mut| {
                if best_target <= u16::MAX as usize {
                    characters_mut[cn].attack_cn = best_target as u16;
                }
                // TODO: char_id equivalent
                // let idx = best_target | (char_id(best_target) << 16);
                // characters_mut[cn].data[80] = idx as i32;
                characters_mut[cn].data[80] = best_target as i32;

                State::with(|state| {
                    state.do_sayx(
                        cn,
                        &format!("Yes {}, I will kill {}!", co_name, target_name),
                    );
                });
            });

            // Notify target (do_notify_char equivalent)
            // do_notify_char(best_target, NT_GOTMISS, co, 0, 0, 0);
        });
    }
}

/// Port of `answer_quiet(int cn, int co)` from `talk.cpp`
/// Toggles companion's talkative mode
pub fn answer_quiet(cn: usize, co: usize) {
    Repository::with_characters(|characters| {
        let is_talkative = characters[cn].data[core::constants::CHD_TALKATIVE] != 0;
        let template_talkative = Repository::with_character_templates(|templates| {
            if (characters[cn].temp as usize) < core::constants::MAXTCHARS {
                templates[characters[cn].temp as usize].data[core::constants::CHD_TALKATIVE]
            } else {
                0
            }
        });

        Repository::with_characters_mut(|characters_mut| {
            if !is_talkative {
                characters_mut[cn].data[core::constants::CHD_TALKATIVE] = template_talkative;
                State::with(|state| {
                    state.do_sayx(
                        cn,
                        &format!(
                            "Thank you, {}, for letting me talk again!",
                            characters[co].get_name()
                        ),
                    );
                });
            } else {
                State::with(|state| {
                    state.do_sayx(
                        cn,
                        &format!("Yes {}, I will shut up now.", characters[co].get_name()),
                    );
                });
                characters_mut[cn].data[core::constants::CHD_TALKATIVE] = 0;
            }
        });
    });
}

/// Port of `answer_health(int cn, int co)` from `talk.cpp`
/// Reports companion's health status
pub fn answer_health(cn: usize, co: usize) {
    Repository::with_characters(|characters| {
        let a_hp = characters[cn].a_hp;
        let hp_max = characters[cn].hp[5] as i32 * 550;
        let co_name = characters[co].get_name().to_string();

        State::with(|state| {
            if a_hp > hp_max {
                state.do_sayx(cn, &format!("I'm fine, {}.", co_name));
            } else if a_hp > (characters[cn].hp[5] as i32 * 250) {
                state.do_sayx(cn, &format!("I don't feel so good, {}.", co_name));
            } else {
                state.do_sayx(cn, "I'm dying!!");
            }
        });
    });
}

/// Port of `answer_shop(int cn, int co)` from `talk.cpp`
/// Explains how to use shop
pub fn answer_shop(cn: usize, co: usize) {
    Repository::with_characters(|characters| {
        let is_merchant =
            characters[cn].flags & core::constants::CharacterFlags::CF_MERCHANT.bits() != 0;
        let co_name = characters[co].get_name().to_string();

        State::with(|state| {
            if is_merchant {
                state.do_sayx(
                    cn,
                    &format!(
                        "Hold down ALT and right click on me to buy or sell, {}.",
                        co_name
                    ),
                );
            } else {
                state.do_sayx(cn, &format!("I'm not a merchant, {}.", co_name));
            }
        });
    });
}

/// Port of `answer_greeting(int cn, int co)` from `talk.cpp`
/// Greets the player
pub fn answer_greeting(cn: usize, co: usize) {
    Repository::with_characters(|characters| {
        let greeting_text = String::from_utf8_lossy(&characters[cn].text[2])
            .trim_matches('\0')
            .to_string();

        if !greeting_text.is_empty() && !greeting_text.starts_with('#') {
            // Special case for Purple One cultist (temp 180)
            if characters[cn].temp == 180
                && (characters[co].kindred & (core::constants::KIN_PURPLE as i32)) != 0
            {
                State::with(|state| {
                    state.do_sayx(cn, &format!("Greetings, {}!", characters[co].get_name()));
                });
                return;
            }

            State::with(|state| {
                let formatted = greeting_text.replace("%s", characters[co].get_name());
                state.do_sayx(cn, &formatted);
            });
        }
    });
}

/// Port of `answer_whoami(int cn, int co)` from `talk.cpp`
/// Tells who the NPC is
pub fn answer_whoami(cn: usize, _co: usize) {
    Repository::with_characters(|characters| {
        let name = characters[cn].get_name().to_string();
        State::with(|state| {
            state.do_sayx(cn, &format!("I am {}.", name));
        });
    });
}

/// Port of `answer_where(int cn, int co)` from `talk.cpp`
/// Tells the current area
pub fn answer_where(cn: usize, _co: usize) {
    // TODO: Implement get_area function
    State::with(|state| {
        state.do_sayx(cn, "I am here."); // Placeholder until get_area is implemented
    });
}

/// Port of `answer_time(int cn, int co)` from `talk.cpp`
/// Tells the current game time
pub fn answer_time(cn: usize, _co: usize) {
    Repository::with_globals(|globals| {
        let day = globals.mdday;
        let year = globals.mdyear;
        let hour = globals.mdtime / 3600;
        let minute = (globals.mdtime / 60) % 60;

        let suffix = match day {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        };

        State::with(|state| {
            state.do_sayx(
                cn,
                &format!(
                    "Today is the {}{} day of the Year {}. It is {}:{:02} Astonian Standard Time.\n",
                    day, suffix, year, hour, minute
                ),
            );
        });
    });
}

/// Port of `stronghold_points(int cn)` from `talk.cpp`
/// Calculates stronghold points for a character
pub fn stronghold_points(cn: usize) -> i32 {
    Repository::with_characters(|characters| {
        characters[cn].data[26] / 25 +      // kills below rank
        characters[cn].data[27] +           // kills at rank
        characters[cn].data[28] * 2 +       // kills above rank
        characters[cn].data[43] * 25 -      // candles
        characters[cn].data[41] // points spent
    })
}

/// Port of `stronghold_exp_per_pt(int cn)` from `talk.cpp`
/// Calculates experience per point for stronghold
pub fn stronghold_exp_per_pt(cn: usize) -> i32 {
    Repository::with_characters(|characters| {
        let exp_per_pt = characters[cn].points_tot / 45123;
        exp_per_pt.max(1).min(125)
    })
}

/// Port of `answer_points(int cn, int co, int nr)` from `talk.cpp`
/// Tells player their stronghold points
pub fn answer_points(cn: usize, co: usize, _know_idx: usize) {
    let exp = stronghold_exp_per_pt(co);
    let pts = stronghold_points(co);

    State::with(|state| {
        state.do_sayx(cn, &format!(
            "You have {} points. You can BUY GOLD at one coin per point, BUY HEALING potions for 6 points, BUY MANA potions for 9 points or BUY EXPerience at {} exp per point.",
            pts, exp
        ));
    });
}

/// Port of `answer_buygold(int cn, int co)` from `talk.cpp`
/// Exchanges stronghold points for gold
pub fn answer_buygold(cn: usize, co: usize) {
    let mut pts = stronghold_points(co);
    pts = pts.min(100);

    if pts < 1 {
        Repository::with_characters(|characters| {
            State::with(|state| {
                state.do_sayx(
                    cn,
                    &format!(
                        "But you don't have any points to spend, {}!",
                        characters[co].get_name()
                    ),
                );
            });
        });
        return;
    }

    Repository::with_characters_mut(|characters| {
        characters[co].data[41] += pts;
        characters[co].gold += pts * 100;

        let co_name = characters[co].get_name().to_string();
        State::with(|state| {
            state.do_sayx(
                cn,
                &format!(
                    "There you are, {}. {} gold coins. Thank you for your help!",
                    co_name, pts
                ),
            );
        });
    });

    log::info!("Character {} bought gold from cityguard ({} pts)", co, pts);
}

/// Port of `answer_buyhealth(int cn, int co)` from `talk.cpp`
/// Exchanges stronghold points for healing potion
pub fn answer_buyhealth(cn: usize, co: usize) {
    let pts = stronghold_points(co);

    if pts < 6 {
        Repository::with_characters(|characters| {
            State::with(|state| {
                state.do_sayx(
                    cn,
                    &format!(
                        "But you don't have enough points to spend, {}!",
                        characters[co].get_name()
                    ),
                );
            });
        });
        return;
    }

    Repository::with_characters_mut(|characters| {
        characters[co].data[41] += 6;
    });

    if let Some(item_id) = God::create_item(101) {
        God::give_character_item(co, item_id);

        Repository::with_characters(|characters| {
            State::with(|state| {
                state.do_sayx(
                    cn,
                    &format!(
                        "There you are, {}. A healing potion. Thank you for your help!",
                        characters[co].get_name()
                    ),
                );
            });
        });

        log::info!("Character {} bought healing potion from cityguard", co);
    }
}

/// Port of `answer_buymana(int cn, int co)` from `talk.cpp`
/// Exchanges stronghold points for mana potion
pub fn answer_buymana(cn: usize, co: usize) {
    let pts = stronghold_points(co);

    if pts < 9 {
        Repository::with_characters(|characters| {
            State::with(|state| {
                state.do_sayx(
                    cn,
                    &format!(
                        "But you don't have enough points to spend, {}!",
                        characters[co].get_name()
                    ),
                );
            });
        });
        return;
    }

    Repository::with_characters_mut(|characters| {
        characters[co].data[41] += 9;
    });

    if let Some(item_id) = God::create_item(102) {
        God::give_character_item(co, item_id);

        Repository::with_characters(|characters| {
            State::with(|state| {
                state.do_sayx(
                    cn,
                    &format!(
                        "There you are, {}. A mana potion. Thank you for your help!",
                        characters[co].get_name()
                    ),
                );
            });
        });

        log::info!("Character {} bought mana potion from cityguard", co);
    }
}

/// Port of `answer_buyexp(int cn, int co)` from `talk.cpp`
/// Exchanges stronghold points for experience
pub fn answer_buyexp(cn: usize, co: usize) {
    let pts = stronghold_points(co);
    let exp = stronghold_exp_per_pt(co);

    if pts < 1 {
        Repository::with_characters(|characters| {
            State::with(|state| {
                state.do_sayx(
                    cn,
                    &format!(
                        "But you don't have any points to spend, {}!",
                        characters[co].get_name()
                    ),
                );
            });
        });
        return;
    }

    let total_exp = pts * exp;

    Repository::with_characters_mut(|characters| {
        characters[co].data[41] += pts;
        characters[co].points += total_exp;
        characters[co].points_tot += total_exp;

        let co_name = characters[co].get_name().to_string();

        State::with(|state| {
            state.do_check_new_level(co);
            state.do_sayx(cn, &format!("Now I'll teach you a bit about life, the world and everything, {}. Thank you for your help!", co_name));
            state.do_character_log(
                co,
                core::types::FontColor::Yellow,
                &format!("You get {} experience points.", total_exp),
            );
        });
    });

    log::info!(
        "Character {} bought {} exps from cityguard ({} pts)",
        co,
        total_exp,
        pts
    );
}

/// Port of `special_answer(int cn, int co, int spec, char* word, int nr)` from `talk.cpp`
/// Dispatches special answer handlers
pub fn special_answer(cn: usize, co: usize, special: i32, word: &str, know_idx: usize) {
    match special {
        HEALTH => answer_health(cn, co),
        SHOP => answer_shop(cn, co),
        GREET => answer_greeting(cn, co),
        WHOAMI => answer_whoami(cn, co),
        WHERE => answer_where(cn, co),
        STOP => answer_stop(cn, co),
        MOVE => answer_move(cn, co),
        ATTACK => answer_attack(cn, co, word),
        WAIT => answer_wait(cn, co),
        FOLLOW => answer_follow(cn, co),
        TIME => answer_time(cn, co),
        POINTS => answer_points(cn, co, know_idx),
        BUYGOLD => answer_buygold(cn, co),
        BUYHEALTH => answer_buyhealth(cn, co),
        BUYMANA => answer_buymana(cn, co),
        BUYEXP => answer_buyexp(cn, co),
        TRANSFER => answer_transfer(cn, co),
        SPELLINFO => answer_spellinfo(cn, co),
        QUIET => answer_quiet(cn, co),
        _ => {}
    }
}

/// Helper function to convert string to lowercase
fn str_lower(s: &str) -> String {
    s.to_lowercase()
}

/// Helper function to check if a word is a fillword
fn is_fillword(word: &str) -> bool {
    const FILLWORDS: &[&str] = &[
        "the", "a", "an", "do", "'", "of", "is", "that", "those", "these", "they", "-", "does",
        "can", "oh", "me", "about", "to", "if", "for",
    ];
    FILLWORDS.contains(&word)
}

/// Helper function to replace synonyms
fn replace_synonym(word: &mut String) {
    const SYNONYMS: &[(&str, &str)] = &[
        ("1", "one"),
        ("2", "two"),
        ("3", "three"),
        ("4", "four"),
        ("5", "five"),
        ("6", "six"),
        ("7", "seven"),
        ("8", "eight"),
        ("9", "nine"),
        ("whats", "what"),
        ("which", "what"),
        ("wheres", "where"),
        ("dangers", "danger"),
        ("enemies", "danger"),
        ("enemy", "danger"),
        ("foe", "danger"),
        ("foes", "danger"),
        ("thieves", "thief"),
        ("trouble", "danger"),
        ("laby", "labyrinth"),
        ("rubies", "ruby"),
        ("joes", "joe"),
        ("skeletons", "skeleton"),
        ("templars", "templar"),
        ("outlaws", "outlaw"),
        ("merchants", "merchant"),
        ("hi", "hello"),
        ("hail", "hello"),
        ("greetings", "hello"),
        ("goodbye", "bye"),
        ("whos", "who"),
        ("thanks", "thank"),
        ("mission", "quest"),
        ("starting", "start"),
        ("damors", "damor"),
        ("jamils", "jamil"),
        ("sirjans", "sirjan"),
        ("point", "place"),
        ("1st", "first"),
        ("2nd", "second"),
        ("3rd", "third"),
        ("limitations", "limitation"),
        ("limits", "limitation"),
        ("limit", "limitation"),
        ("quit", "exit"),
        ("leave", "exit"),
        ("ratlings", "ratling"),
        ("eyes", "eye"),
        ("helmet", "helm"),
        ("shadows", "shadow"),
        ("poems", "poem"),
    ];

    for (from, to) in SYNONYMS {
        if word == from {
            *word = to.to_string();
            return;
        }
    }
}

/// Port of `npc_hear(int cn, int co, char* text)` from `talk.cpp`
/// Main NPC conversation handler
pub fn npc_hear(cn: usize, co: usize, text: &str) {
    // Check for stop keyword
    let stop_keyword = Repository::with_characters(|characters| {
        String::from_utf8_lossy(&characters[cn].text[6])
            .trim_matches('\0')
            .to_string()
    });

    if !stop_keyword.is_empty() && text.eq_ignore_ascii_case(&stop_keyword) {
        Repository::with_characters_mut(|characters| {
            for n in 80..92 {
                characters[cn].data[n] = 0;
            }
            characters[cn].attack_cn = 0;
            characters[cn].goto_x = 0;
            characters[cn].goto_y = 0;
            characters[cn].misc_action = 0;
            characters[cn].data[78] = 0;

            let ticker = Repository::with_globals(|g| g.ticker);
            characters[cn].data[27] = ticker as i32;

            let response = String::from_utf8_lossy(&characters[cn].text[7])
                .trim_matches('\0')
                .to_string();
            if !response.is_empty() {
                State::with(|state| {
                    state.do_sayx(cn, &response);
                });
            }
        });
        return;
    }

    // Don't talk to enemies
    if obey(cn, co) == 0 {
        let is_enemy = Repository::with_characters(|characters| {
            for n in 80..92 {
                if (characters[cn].data[n] & 0xffff) == co as i32 {
                    return true;
                }
            }
            false
        });

        if is_enemy {
            return;
        }
    }

    // Parse the text into words
    let mut words: Vec<String> = Vec::new();
    let mut exclam = 0;
    let mut question = 0;
    let mut name_mentioned = false;

    let text_lower = str_lower(text);
    let npc_name_lower =
        Repository::with_characters(|characters| str_lower(characters[cn].get_name()));

    for c in text.chars() {
        if c == '!' {
            exclam += 1;
        } else if c == '?' {
            question += 1;
        }
    }

    let mut current_word = String::new();
    let text_lower_str = text_lower.clone();
    for c in text_lower_str.chars() {
        if c.is_alphanumeric() || c.is_whitespace() {
            if c.is_whitespace() {
                if !current_word.is_empty() {
                    if is_fillword(&current_word) {
                        // Skip fillwords
                    } else if current_word == npc_name_lower {
                        name_mentioned = true;
                    } else {
                        words.push(current_word.clone());
                    }
                    current_word.clear();
                }
            } else if current_word.len() < 39 {
                current_word.push(c);
            }
        }
    }

    if !current_word.is_empty() {
        if is_fillword(&current_word) {
            // Skip fillwords
        } else if current_word == npc_name_lower {
            name_mentioned = true;
        } else {
            words.push(current_word);
        }
    }

    // Replace synonyms
    for word in &mut words {
        replace_synonym(word);
    }

    // Find best matching knowledge entry
    let mut best_conf = 0;
    let mut best_nr = None;

    let (npc_knowledge, npc_area, npc_temp) = Repository::with_characters(|characters| {
        (
            characters[cn].data[68],
            characters[cn].data[72],
            characters[cn].temp,
        )
    });

    for (idx, know_entry) in KNOW.iter().enumerate() {
        // Check if NPC has the required knowledge, area, and temp
        if npc_knowledge >= know_entry.value
            && (npc_area == know_entry.area || npc_area == AR_ALL || know_entry.area == AR_GENERAL)
            && (know_entry.temp == 0 || know_entry.temp == npc_temp as i32)
        {
            let mut hit = 0;
            let mut miss = 0;
            let mut got_word = vec![false; words.len()];

            for keyword in &know_entry.word {
                if keyword.is_empty() {
                    break;
                }

                if keyword.len() == 1 {
                    let found = match *keyword {
                        "?" => question > 0,
                        "!" => exclam > 0,
                        "$" => name_mentioned,
                        _ => false,
                    };
                    if found {
                        hit += 1;
                    } else {
                        miss += 1;
                    }
                } else {
                    let (miss_cost, hit_cost) = match keyword.chars().next() {
                        Some('?') => (1, 1),
                        Some('!') => (5, 2),
                        _ => (0, 0),
                    };

                    let keyword_stripped = if keyword.starts_with('!') || keyword.starts_with('?') {
                        &keyword[1..]
                    } else {
                        keyword
                    };

                    let mut found = false;
                    for (word_idx, word) in words.iter().enumerate() {
                        if word == keyword_stripped {
                            got_word[word_idx] = true;
                            found = true;
                            break;
                        }
                    }

                    if found {
                        hit += hit_cost;
                    } else {
                        miss += miss_cost;
                    }
                }
            }

            // Count ungot words as misses
            for got in got_word {
                if got {
                    hit += 1;
                } else {
                    miss += 1;
                }
            }

            let conf = hit - miss;
            if conf > best_conf {
                best_conf = conf;
                best_nr = Some(idx);
            }
        }
    }

    // Determine if NPC should talk
    let talk_level = Repository::with_characters(|characters| {
        characters[cn].data[core::constants::CHD_TALKATIVE]
    }) + if name_mentioned { 1 } else { 0 }
        + if obey(cn, co) != 0 { 20 } else { 0 };

    if talk_level > 0 && best_conf > 0 {
        if let Some(nr) = best_nr {
            let know_entry = &KNOW[nr];

            if know_entry.special == 0 {
                let answer = Repository::with_characters(|characters| {
                    know_entry.answer.replace("%s", characters[co].get_name())
                });

                State::with(|state| {
                    state.do_sayx(cn, &answer);
                });

                log::info!("Character {} answered \"{}\" with \"{}\"", cn, text, answer);
            } else {
                special_answer(cn, co, know_entry.special, text, nr);
                log::info!(
                    "Character {} answered \"{}\" with special {}",
                    cn,
                    text,
                    know_entry.special
                );
            }
        }
    } else if name_mentioned && talk_level > 0 {
        State::with(|state| {
            state.do_sayx(cn, "I don't know about that.");
        });
    }

    if best_conf <= 0 && talk_level > 0 {
        log::debug!("Character {} could not answer \"{}\"", cn, text);
    }
}
