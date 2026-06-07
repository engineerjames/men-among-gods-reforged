//! Version 2 game data type definitions.
//!
//! `v2::Character` and `v2::Item` differ from v1 only in the size of their
//! per-character skill matrix: the skill axis grew from 50 to
//! [`crate::skills::MAX_SKILLS`] (currently 75) to accommodate the Harakim
//! ability slots and reserved headroom for future class additions.
//!
//! All other entity shapes (`Map`, `Effect`, `Global`) are unchanged from
//! v1 and are re-exported verbatim.

pub use super::Character;
pub use super::Effect;
pub use super::Global;
pub use super::Item;
pub use super::Map;
