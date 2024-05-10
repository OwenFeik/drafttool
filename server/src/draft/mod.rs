use std::fmt::Debug;

use crate::cards::{Card, Rarity};

mod game;
pub mod handlers;
mod packs;
mod server;

#[derive(Debug)]
struct DraftConfig {
    /// Number of packs in the draft.
    rounds: usize,

    /// Number of cards in each pack.
    cards_per_pack: usize,

    /// Whether to choose cards with replacement (false) or not (true).
    unique_cards: bool,

    /// Whether to select cards by rarity or at random.
    use_rarities: bool,

    /// Whether to allow falling back to a different rarity on running out.
    allow_fallback: bool,

    /// Rate at which a rare is upgraded to a mythic rare.
    mythic_rate: f32,

    /// Number of rares in each pack.
    rares: usize,

    /// Number of uncommons in each pack.
    uncommons: usize,

    /// Number of commons in each pack.
    commons: usize,
}

impl Default for DraftConfig {
    fn default() -> Self {
        DraftConfig {
            rounds: 3,
            cards_per_pack: 15,
            unique_cards: true,
            use_rarities: true,
            allow_fallback: true,
            mythic_rate: 0.125,
            rares: 1,
            uncommons: 3,
            commons: 11,
        }
    }
}
