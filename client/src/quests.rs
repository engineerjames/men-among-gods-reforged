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

/// All quest chains, in suggested play order.
///
/// TODO: replace the placeholder `(0, 0)` coordinates with real locations.
pub static QUEST_CHAINS: &[QuestChain] = &[QuestChain {
    name: "Barter (Jamil)",
    min_rank: Rank::Private,
    steps: &[
        QuestStep {
            description: "Talk to Jamil",
            x: 0,
            y: 0,
            done_when: StepDone::Started(1),
        },
        QuestStep {
            description: "Find Jamil's amulet in the Thieves House",
            x: 0,
            y: 0,
            done_when: StepDone::ItemObtained(1),
        },
        QuestStep {
            description: "Return the amulet to Jamil",
            x: 0,
            y: 0,
            done_when: StepDone::Completed(1),
        },
    ],
}];

/// One suggested objective with a single world coordinate of interest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestLogEntry {
    /// Short player-facing description of the objective.
    pub description: String,
    /// World tile X of the objective's point of interest.
    pub x: u16,
    /// World tile Y of the objective's point of interest.
    pub y: u16,
    /// Minimum rank of the chain this objective belongs to.
    pub min_rank: Rank,
}

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
    /// * The current step of every unfinished chain the player's rank allows,
    ///   in catalog order.
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
    /// * The current step of every eligible, unfinished chain.
    fn build_from(
        chains: &[QuestChain],
        completion: &CompletionSnapshot,
        points_total: u32,
    ) -> Self {
        let rank = points2rank(points_total) as usize;
        let entries = chains
            .iter()
            .filter(|chain| chain.min_rank.index() <= rank)
            .filter_map(|chain| {
                chain.current_step(completion).map(|step| QuestLogEntry {
                    description: step.description.to_owned(),
                    x: step.x,
                    y: step.y,
                    min_rank: chain.min_rank,
                })
            })
            .collect();
        Self { entries }
    }

    /// Returns the coordinate the compass should point at.
    ///
    /// # Returns
    ///
    /// * The first objective's `(x, y)`, or `None` when there are none.
    pub fn compass_target(&self) -> Option<(u16, u16)> {
        self.entries.first().map(|e| (e.x, e.y))
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
    fn chains_above_player_rank_are_hidden_and_order_is_kept() {
        static STEPS: &[QuestStep] = &[QuestStep {
            description: "Step",
            x: 7,
            y: 9,
            done_when: StepDone::Completed(5),
        }];
        let chains = [
            QuestChain {
                name: "high",
                min_rank: Rank::Corporal,
                steps: STEPS,
            },
            QuestChain {
                name: "low",
                min_rank: Rank::Private,
                steps: STEPS,
            },
        ];
        let none = CompletionSnapshot::default();

        let fresh = QuestLog::build_from(&chains, &none, 0);
        assert_eq!(fresh.entries.len(), 1);
        assert_eq!(fresh.entries[0].min_rank, Rank::Private);
        assert_eq!(fresh.compass_target(), Some((7, 9)));

        // 4900 points = Corporal: both chains, catalog order preserved.
        let corporal = QuestLog::build_from(&chains, &none, 4900);
        let ranks: Vec<Rank> = corporal.entries.iter().map(|e| e.min_rank).collect();
        assert_eq!(ranks, vec![Rank::Corporal, Rank::Private]);

        assert_eq!(QuestLog::default().compass_target(), None);
    }
}
