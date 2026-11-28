//! Hand-authored quest chains that drive the helper window's objectives list
//! and the compass target.
//!
//! Each [`QuestChain`] is an ordered list of [`QuestStep`]s from first contact
//! to turn-in. Only the chain's *current* step is shown: the step after the
//! last one whose server fact (from `SetCompletionData`) is set. Facts are
//! monotonic, so a later fact implies every earlier step is done (e.g. a
//! character that turned in the quest before "started" bits existed still
//! counts as finished).
//!
//! Coordinates are placeholders until populated by hand.

use mag_core::ranks::{Rank, points2rank};

use crate::player_state::CompletionSnapshot;

/// Server fact that marks a step as done; the `u8` is the server's
/// `QuestDef::id` (bit index in the matching completion bitset).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepDone {
    /// The quest NPC greeted the player with the quest request.
    Started(u8),
    /// The player has picked up the quest item at least once (sticky).
    ItemObtained(u8),
    /// The quest NPC accepted the quest item.
    Completed(u8),
}

impl StepDone {
    /// Returns whether this fact is set in `completion`.
    ///
    /// # Arguments
    ///
    /// * `completion` - Latest server completion snapshot.
    ///
    /// # Returns
    ///
    /// * `true` when the matching quest bit is set.
    fn is_set(self, completion: &CompletionSnapshot) -> bool {
        let (bits, id) = match self {
            StepDone::Started(id) => (completion.quest_started_bits, id),
            StepDone::ItemObtained(id) => (completion.quest_item_obtained_bits, id),
            StepDone::Completed(id) => (completion.quest_completion_bits, id),
        };
        id < 32 && (bits >> id) & 1 != 0
    }
}

/// One objective in a quest chain.
#[derive(Debug)]
pub struct QuestStep {
    /// Player-facing objective text.
    pub description: &'static str,
    /// World tile X the compass points at while this step is current.
    pub x: u16,
    /// World tile Y the compass points at while this step is current.
    pub y: u16,
    /// Server fact that completes this step.
    pub done_when: StepDone,
}

/// An ordered quest from first contact to turn-in.
#[derive(Debug)]
pub struct QuestChain {
    /// Internal/debug name of the chain.
    pub name: &'static str,
    /// Minimum player rank before the chain is suggested.
    pub min_rank: Rank,
    /// Steps in completion order.
    pub steps: &'static [QuestStep],
}

impl QuestChain {
    /// Returns the step the player should do next.
    ///
    /// # Arguments
    ///
    /// * `completion` - Latest server completion snapshot.
    ///
    /// # Returns
    ///
    /// * The step after the last completed one, or `None` when the chain is
    ///   finished.
    pub fn current_step(&self, completion: &CompletionSnapshot) -> Option<&QuestStep> {
        let next = self
            .steps
            .iter()
            .rposition(|s| s.done_when.is_set(completion))
            .map_or(0, |i| i + 1);
        self.steps.get(next)
    }
}

/// Builds the standard talk -> fetch item -> return chain for a quest.
///
/// # Arguments
///
/// * `id` - Server `QuestDef::id` (bit index in the completion bitsets).
/// * `talk` - `(text, x, y)` for "talk to the quest NPC".
/// * `find` - `(text, x, y)` for "find the requested item".
/// * `turn_in` - `(text, x, y)` for "bring the item back".
///
/// # Returns
///
/// * The three steps, completed by the started / item-obtained / completed
///   facts respectively.
const fn fetch_quest(
    id: u8,
    talk: (&'static str, u16, u16),
    find: (&'static str, u16, u16),
    turn_in: (&'static str, u16, u16),
) -> [QuestStep; 3] {
    [
        QuestStep {
            description: talk.0,
            x: talk.1,
            y: talk.2,
            done_when: StepDone::Started(id),
        },
        QuestStep {
            description: find.0,
            x: find.1,
            y: find.2,
            done_when: StepDone::ItemObtained(id),
        },
        QuestStep {
            description: turn_in.0,
            x: turn_in.1,
            y: turn_in.2,
            done_when: StepDone::Completed(id),
        },
    ]
}

/// All quest chains, in suggested play order.
///
/// Ids match `server::quest_completion::QUEST_DEFS`.
///
/// TODO: replace the placeholder `(0, 0)` coordinates with real locations and
/// set each chain's real `min_rank`.
pub static QUEST_CHAINS: &[QuestChain] = &[
    QuestChain {
        name: "Black Candle (Cityguard)",
        min_rank: Rank::Private,
        steps: &fetch_quest(
            0,
            ("Talk to the Cityguard", 0, 0),
            ("Find a Black Candle", 0, 0),
            ("Return the Black Candle to the Cityguard", 0, 0),
        ),
    },
    QuestChain {
        name: "Barter (Jamil)",
        min_rank: Rank::Private,
        steps: &fetch_quest(
            1,
            ("Talk to Jamil", 542, 529),
            ("Find Jamil's amulet in the Thieves House", 529, 543),
            ("Return the amulet to Jamil", 542, 529), // TODO: Simplify to just providing two coordinates?
        ),
    },
    QuestChain {
        name: "Enhance Weapon (Sirjan)",
        min_rank: Rank::Private,
        steps: &fetch_quest(
            2,
            ("Talk to Sirjan", 559, 500),
            ("Find the Belt in the Haunted Castle", 599, 542),
            ("Return the Belt to Sirjan", 559, 500),
        ),
    },
    QuestChain {
        name: "Recall (Inga)",
        min_rank: Rank::Private,
        steps: &fetch_quest(
            3,
            ("Talk to Inga", 568, 528),
            ("Find the Ruby in the Cursed Tomb", 543, 561),
            ("Return the Ruby to Inga", 568, 528),
        ),
    },
    QuestChain {
        name: "Repair (Jefferson)",
        min_rank: Rank::Private,
        steps: &fetch_quest(
            4,
            ("Talk to Jefferson", 0, 0),
            ("Find the Bronze Armor Jefferson wants", 0, 0),
            ("Return the Bronze Armor to Jefferson", 0, 0),
        ),
    },
    QuestChain {
        name: "Stun (Ingrid)",
        min_rank: Rank::Private,
        steps: &fetch_quest(
            5,
            ("Talk to Ingrid", 595, 499),
            (
                "Find the Greater Healing Potion from the Skeletonn Lord",
                600,
                488,
            ),
            ("Return the Greater Healing Potion to Ingrid", 595, 499),
        ),
    },
    QuestChain {
        name: "Lock (Steven)",
        min_rank: Rank::Private,
        steps: &fetch_quest(
            6,
            ("Talk to Steven", 0, 0),
            ("Find the Barbarian Sword Steven wants", 0, 0),
            ("Return the Barbarian Sword to Steven", 0, 0),
        ),
    },
    QuestChain {
        name: "Bless (Cirrus)",
        min_rank: Rank::Private,
        steps: &fetch_quest(
            7,
            ("Talk to Cirrus", 0, 0),
            ("Find the Ruby Amulet Cirrus wants", 0, 0),
            ("Return the Ruby Amulet to Cirrus", 0, 0),
        ),
    },
    QuestChain {
        name: "Identify (Nasir)",
        min_rank: Rank::Private,
        steps: &fetch_quest(
            8,
            ("Talk to Nasir", 0, 0),
            ("Find the Greater Mana Potion Nasir wants", 0, 0),
            ("Return the Greater Mana Potion to Nasir", 0, 0),
        ),
    },
    QuestChain {
        name: "Resist (Serena)",
        min_rank: Rank::Private,
        steps: &fetch_quest(
            9,
            ("Talk to Serena", 0, 0),
            ("Find the Sword of Stealth Serena wants", 0, 0),
            ("Return the Sword of Stealth to Serena", 0, 0),
        ),
    },
    QuestChain {
        name: "Curse (Gordon)",
        min_rank: Rank::Private,
        steps: &fetch_quest(
            10,
            ("Talk to Gordon", 0, 0),
            ("Find the Potion of Life Gordon wants", 0, 0),
            ("Return the Potion of Life to Gordon", 0, 0),
        ),
    },
    QuestChain {
        name: "Sense (Manfred)",
        min_rank: Rank::Private,
        steps: &fetch_quest(
            11,
            ("Talk to Manfred", 0, 0),
            ("Find the Silver Small Ruby Ring Manfred wants", 0, 0),
            ("Return the Silver Small Ruby Ring to Manfred", 0, 0),
        ),
    },
    QuestChain {
        name: "Rest (Leopold)",
        min_rank: Rank::Private,
        steps: &fetch_quest(
            12,
            ("Talk to Leopold", 0, 0),
            ("Find the Ratling Fighter's Eye Leopold wants", 0, 0),
            ("Return the Ratling Fighter's Eye to Leopold", 0, 0),
        ),
    },
    QuestChain {
        name: "Heal (Gunther)",
        min_rank: Rank::Private,
        steps: &fetch_quest(
            13,
            ("Talk to Gunther", 0, 0),
            ("Find the Amulet of Resistance Gunther wants", 0, 0),
            ("Return the Amulet of Resistance to Gunther", 0, 0),
        ),
    },
];

/// One suggested objective with a single world coordinate of interest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestLogEntry {
    /// Name of the chain this objective comes from; stable as the chain
    /// advances, so it identifies the player's tracked objective.
    pub chain: &'static str,
    /// Short player-facing description of the objective.
    pub description: String,
    /// World tile X of the objective's point of interest.
    pub x: u16,
    /// World tile Y of the objective's point of interest.
    pub y: u16,
    /// Minimum rank of the chain this objective belongs to.
    pub min_rank: Rank,
}

/// Maximum number of objectives suggested at once.
pub const MAX_QUEST_LOG_ENTRIES: usize = 3;

/// Ordered list of suggested objectives, most relevant first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestLog {
    /// Objectives in display order.
    pub entries: Vec<QuestLogEntry>,
}

impl QuestLog {
    /// Builds the objectives for a player from [`QUEST_CHAINS`].
    ///
    /// # Arguments
    ///
    /// * `completion` - Latest server completion snapshot.
    /// * `points_total` - The player's total experience points (for rank).
    ///
    /// # Returns
    ///
    /// * Up to [`MAX_QUEST_LOG_ENTRIES`] current steps of unfinished chains at
    ///   most one rank above the player, lowest `min_rank` first.
    pub fn build(completion: &CompletionSnapshot, points_total: u32) -> Self {
        Self::build_from(QUEST_CHAINS, completion, points_total)
    }

    /// Builds the objectives from an explicit chain list.
    ///
    /// # Arguments
    ///
    /// * `chains` - Quest chains in suggested play order.
    /// * `completion` - Latest server completion snapshot.
    /// * `points_total` - The player's total experience points (for rank).
    ///
    /// # Returns
    ///
    /// * Up to [`MAX_QUEST_LOG_ENTRIES`] current steps of unfinished chains
    ///   whose `min_rank` is at most one above the player's, sorted by
    ///   `min_rank` (catalog order within a rank).
    fn build_from(
        chains: &[QuestChain],
        completion: &CompletionSnapshot,
        points_total: u32,
    ) -> Self {
        let max_rank = points2rank(points_total) as usize + 1;
        let mut eligible: Vec<&QuestChain> = chains
            .iter()
            .filter(|chain| chain.min_rank.index() <= max_rank)
            .collect();
        // Stable: chains of equal rank keep catalog order.
        eligible.sort_by_key(|chain| chain.min_rank.index());
        let entries = eligible
            .into_iter()
            .filter_map(|chain| {
                chain.current_step(completion).map(|step| QuestLogEntry {
                    chain: chain.name,
                    description: step.description.to_owned(),
                    x: step.x,
                    y: step.y,
                    min_rank: chain.min_rank,
                })
            })
            .take(MAX_QUEST_LOG_ENTRIES)
            .collect();
        Self { entries }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completion(started: u32, obtained: u32, completed: u32) -> CompletionSnapshot {
        CompletionSnapshot {
            quest_started_bits: started,
            quest_item_obtained_bits: obtained,
            quest_completion_bits: completed,
            ..CompletionSnapshot::default()
        }
    }

    fn jamil(completion: &CompletionSnapshot) -> Option<String> {
        QuestLog::build(completion, 0)
            .entries
            .into_iter()
            .find(|e| e.description.contains("Jamil"))
            .map(|e| e.description)
    }

    #[test]
    fn jamil_chain_advances_with_server_facts() {
        let bit = 1 << 1;
        assert_eq!(
            jamil(&completion(0, 0, 0)).as_deref(),
            Some("Talk to Jamil")
        );
        assert_eq!(
            jamil(&completion(bit, 0, 0)).as_deref(),
            Some("Find Jamil's amulet in the Thieves House")
        );
        assert_eq!(
            jamil(&completion(bit, bit, 0)).as_deref(),
            Some("Return the amulet to Jamil")
        );
        assert_eq!(jamil(&completion(bit, bit, bit)), None);
    }

    #[test]
    fn later_facts_imply_earlier_steps() {
        let bit = 1 << 1;
        // Found the amulet without talking to Jamil first.
        assert_eq!(
            jamil(&completion(0, bit, 0)).as_deref(),
            Some("Return the amulet to Jamil")
        );
        // Turned in before started/obtained bits existed.
        assert_eq!(jamil(&completion(0, 0, bit)), None);
        // Other quests' bits don't advance this chain.
        assert_eq!(
            jamil(&completion(!bit, !bit, !bit)).as_deref(),
            Some("Talk to Jamil")
        );
    }

    #[test]
    fn every_chain_ends_with_a_distinct_in_range_turn_in() {
        let ids: Vec<u8> = QUEST_CHAINS
            .iter()
            .filter_map(|chain| match chain.steps.last()?.done_when {
                StepDone::Completed(id) => Some(id),
                _ => None,
            })
            .collect();
        assert_eq!(
            ids.len(),
            QUEST_CHAINS.len(),
            "every chain ends with a turn-in"
        );
        assert!(ids.iter().all(|&id| id < 32));
        let unique: std::collections::HashSet<u8> = ids.iter().copied().collect();
        assert_eq!(unique.len(), ids.len(), "chains must not share quest ids");
    }

    #[test]
    fn log_is_rank_limited_sorted_and_capped() {
        static STEPS: &[QuestStep] = &[QuestStep {
            description: "Step",
            x: 7,
            y: 9,
            done_when: StepDone::Completed(5),
        }];
        let chain = |name, min_rank| QuestChain {
            name,
            min_rank,
            steps: STEPS,
        };
        let chains = [
            chain("corporal", Rank::Corporal),
            chain("pfc", Rank::PrivateFirstClass),
            chain("lance", Rank::LanceCorporal),
            chain("private-a", Rank::Private),
            chain("private-b", Rank::Private),
        ];
        let none = CompletionSnapshot::default();
        let names = |points| -> Vec<&str> {
            QuestLog::build_from(&chains, &none, points)
                .entries
                .iter()
                .map(|e| e.chain)
                .collect()
        };

        // Private: one rank above (PFC) is shown, two above (Lance) is not.
        assert_eq!(names(0), vec!["private-a", "private-b", "pfc"]);
        // 4900 points = Corporal: all eligible, sorted, capped at 3.
        assert_eq!(names(4900), vec!["private-a", "private-b", "pfc"]);
        // Finishing the low chains surfaces the next ranks in order.
        let done_low = QuestLog::build_from(&chains[..3], &none, 4900);
        let ranks: Vec<Rank> = done_low.entries.iter().map(|e| e.min_rank).collect();
        assert_eq!(
            ranks,
            vec![Rank::PrivateFirstClass, Rank::LanceCorporal, Rank::Corporal]
        );
    }
}
