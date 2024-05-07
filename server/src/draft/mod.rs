use std::fmt::Debug;

use crate::cards::{Card, Rarity};

pub mod handlers;
mod packs;

#[derive(Debug)]
struct DraftConfig {
    packs: u32,
    cards_per_pack: u32,
    unique_cards: bool,
    use_rarities: bool,
    mythic_rate: f32,
    rares: u32,
    uncommons: u32,
    commons: u32,
}

impl DraftConfig {
    fn new() -> Self {
        DraftConfig {
            packs: 3,
            cards_per_pack: 15,
            unique_cards: true,
            use_rarities: true,
            mythic_rate: 0.125,
            rares: 1,
            uncommons: 3,
            commons: 11,
        }
    }
}
