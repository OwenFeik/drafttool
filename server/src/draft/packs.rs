use std::fmt::Debug;

use rand::{distributions::Uniform, random, seq::SliceRandom, thread_rng, Rng};

use crate::cards::{Card, Rarity};

use super::DraftConfig;

pub struct DraftPool {
    mythics: Vec<Card>,
    rares: Vec<Card>,
    uncommons: Vec<Card>,
    commons: Vec<Card>,
}

impl DraftPool {
    pub fn new() -> Self {
        Self {
            mythics: Vec::new(),
            rares: Vec::new(),
            uncommons: Vec::new(),
            commons: Vec::new(),
        }
    }

    pub fn add(&mut self, card: Card) {
        match card.rarity {
            Rarity::Mythic => self.mythics.push(card),
            Rarity::Rare => self.rares.push(card),
            Rarity::Uncommon => self.uncommons.push(card),
            Rarity::Common => self.commons.push(card),
            Rarity::Bonus | Rarity::Special => {} // Special and bonus not part of pool.
        }
    }
}

impl Debug for DraftPool {
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

pub struct Pack {
    cards: Vec<Card>,
}

fn make_cube_packs(players: u32, config: DraftConfig, mut pool: DraftPool) -> Vec<Pack> {
    let mut rng = thread_rng();
    pool.mythics.shuffle(&mut rng);
    pool.rares.shuffle(&mut rng);
    pool.uncommons.shuffle(&mut rng);
    pool.commons.shuffle(&mut rng);

    let mut packs = Vec::new();

    for _ in 0..(players * config.packs) {
        let mut pack = Vec::new();

        for _ in 0..config.rares {
            if rng.gen_range(0.0..=1.0) < config.mythic_rate {}
        }

        packs.push(Pack { cards: pack })
    }

    packs
}

fn make_draft_packs(players: u32, config: DraftConfig, pool: DraftPool) -> Vec<Pack> {
    Vec::new()
}

pub fn make_packs(players: u32, config: DraftConfig, pool: DraftPool) -> Vec<Pack> {
    if config.unique_cards {
        make_cube_packs(players, config, pool)
    } else {
        make_draft_packs(players, config, pool)
    }
}
