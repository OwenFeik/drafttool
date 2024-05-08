use std::fmt::Debug;

use rand::{distributions::Uniform, random, seq::SliceRandom, thread_rng, Rng};

use crate::{
    cards::{Card, Rarity},
    err, Res,
};

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

    fn empty(&self) -> bool {
        self.mythics.is_empty()
            && self.rares.is_empty()
            && self.uncommons.is_empty()
            && self.commons.is_empty()
    }

    fn cards_of(&self, rarity: Rarity) -> &[Card] {
        match rarity {
            Rarity::Mythic => &self.mythics,
            Rarity::Rare => &self.rares,
            Rarity::Uncommon => &self.uncommons,
            Rarity::Common => &self.commons,
            Rarity::Bonus | Rarity::Special => &[],
        }
    }

    /// Given a rarity that we are out of, which rarity should we replace that
    /// card slot with.
    fn replacement_rarity(&self, rarity: Rarity) -> Option<Rarity> {
        use Rarity::*;

        // First element is the input rarity, following elements are the
        // replacement priority order.
        const PRIORITIES: &[&[Rarity]] = &[
            &[Mythic, Rare, Uncommon, Common],
            &[Rare, Mythic, Uncommon, Common],
            &[Uncommon, Common, Rare, Mythic],
            &[Common, Uncommon, Rare, Mythic],
        ];

        PRIORITIES
            .iter()
            .find(|l| l.starts_with(&[rarity]))
            .and_then(|l| l.iter().find(|r| !self.cards_of(**r).is_empty()))
            .copied()
    }

    fn take(&mut self, rarity: Rarity) -> Res<Card> {
        if self.empty() {
            return err("Insufficient cards in pool.");
        }

        let exact = match rarity {
            Rarity::Mythic => self.mythics.pop(),
            Rarity::Rare => self.rares.pop(),
            Rarity::Uncommon => self.uncommons.pop(),
            Rarity::Common => self.commons.pop(),
            Rarity::Bonus | Rarity::Special => None,
        };

        if let Some(card) = exact {
            Ok(card)
        } else if let Some(fallback) = self.replacement_rarity(rarity) {
            self.take(fallback)
        } else {
            err("Insufficient cards in pool.")
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

fn make_cube_packs(players: u32, config: DraftConfig, mut pool: DraftPool) -> Res<Vec<Pack>> {
    let mut rng = thread_rng();
    pool.mythics.shuffle(&mut rng);
    pool.rares.shuffle(&mut rng);
    pool.uncommons.shuffle(&mut rng);
    pool.commons.shuffle(&mut rng);

    let mut packs = Vec::new();

    for _ in 0..(players * config.packs) {
        let mut pack = Vec::new();

        for _ in 0..config.rares {
            if rng.gen_range(0.0..=1.0) < config.mythic_rate {
                pack.push(pool.take(Rarity::Mythic)?);
            } else {
                pack.push(pool.take(Rarity::Rare)?);
            }
        }

        for _ in 0..config.uncommons {
            pack.push(pool.take(Rarity::Uncommon)?);
        }

        for _ in 0..config.commons {
            pack.push(pool.take(Rarity::Common)?);
        }

        packs.push(Pack { cards: pack })
    }

    Ok(packs)
}

fn make_draft_packs(players: u32, config: DraftConfig, pool: DraftPool) -> Res<Vec<Pack>> {
    Ok(Vec::new())
}

pub fn make_packs(players: u32, config: DraftConfig, pool: DraftPool) -> Res<Vec<Pack>> {
    if config.unique_cards {
        make_cube_packs(players, config, pool)
    } else {
        make_draft_packs(players, config, pool)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        cards::{Card, Rarity},
        draft::DraftConfig,
    };

    use super::{make_packs, DraftPool};

    #[test]
    fn test_make_cube_packs() {
        let config = DraftConfig {
            packs: 2,
            cards_per_pack: 3,
            rares: 1,
            uncommons: 1,
            commons: 1,
            mythic_rate: 1.0,
            ..Default::default()
        };

        let mut pool = DraftPool::new();
        pool.add(Card::sample(Rarity::Mythic));
        pool.add(Card::sample(Rarity::Mythic));
        pool.add(Card::sample(Rarity::Mythic));
        pool.add(Card::sample(Rarity::Mythic));
        pool.add(Card::sample(Rarity::Rare));
        pool.add(Card::sample(Rarity::Rare));
        pool.add(Card::sample(Rarity::Rare));
        pool.add(Card::sample(Rarity::Rare));
        pool.add(Card::sample(Rarity::Uncommon));
        pool.add(Card::sample(Rarity::Uncommon));
        pool.add(Card::sample(Rarity::Uncommon));
        pool.add(Card::sample(Rarity::Uncommon));
        pool.add(Card::sample(Rarity::Common));
        pool.add(Card::sample(Rarity::Common));
        pool.add(Card::sample(Rarity::Common));
        pool.add(Card::sample(Rarity::Common));

        let packs = make_packs(2, config, pool).unwrap();
        assert!(packs.len() == 4); // 2 players, 2 packs each
        assert!(packs.iter().all(|p| p.cards.len() == 3)); // 3 cards per pack

        // Each pack should contain a mythic as 100% of rares should be promoted
        // to mythics.
        assert!(packs.iter().all(|p| p
            .cards
            .iter()
            .find(|c| c.rarity == Rarity::Mythic)
            .is_some()));
    }
}
