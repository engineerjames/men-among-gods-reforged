//! In-game Journal (Guidebook) data: static navigation catalog, a minimal
//! markdown-subset parser, and file-backed content loading.
//!
//! The Journal is a purely client-side, static-content overlay opened from
//! the HUD button bar. Unlike the legacy quest log it carries no network
//! traffic or server-driven state — all content lives in `.md` files under
//! `client/assets/journal/` and is read from disk on demand.

pub mod catalog;
pub mod content;
pub mod markdown;
