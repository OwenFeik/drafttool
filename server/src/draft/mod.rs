use std::fmt::Debug;

use crate::cards::{Card, Rarity};

pub mod handlers;

struct DraftList {
    mythics: Vec<Card>,
    rares: Vec<Card>,
    uncommons: Vec<Card>,
    commons: Vec<Card>,
}

impl DraftList {
    fn new() -> Self {
        Self {
            mythics: Vec::new(),
            rares: Vec::new(),
            uncommons: Vec::new(),
            commons: Vec::new(),
        }
    }

    fn add(&mut self, card: Card) {
        match card.rarity {
            Rarity::Mythic => self.mythics.push(card),
            Rarity::Rare => self.rares.push(card),
            Rarity::Uncommon => self.uncommons.push(card),
            Rarity::Common => self.commons.push(card),
            Rarity::Bonus | Rarity::Special => {} // Special and bonus not part of pool.
        }
    }
}

impl Debug for DraftList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CardDatabase {{ mythics: {}, rares: {}, uncommons: {}, commons: {} }}",
            self.mythics.len(),
            self.rares.len(),
            self.uncommons.len(),
            self.commons.len()
        )
    }
}

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

#[derive(Debug)]
struct DraftLobby {
    database: DraftList,
    config: DraftConfig,
}
