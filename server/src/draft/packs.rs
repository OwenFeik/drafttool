use std::fmt::Debug;

use rand::{seq::SliceRandom, thread_rng, Rng};

use crate::{
    cards::{Card, Rarity},
    err, Res,
};

use super::DraftConfig;

#[derive(Clone)]
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

    #[cfg(test)]
    pub fn sample(mythics: usize, rares: usize, uncommons: usize, commons: usize) -> Self {
        let mut pool = Self::new();
        for _ in 0..mythics {
            pool.add(Card::sample(Rarity::Mythic));
        }
        for _ in 0..rares {
            pool.add(Card::sample(Rarity::Rare));
        }
        for _ in 0..uncommons {
            pool.add(Card::sample(Rarity::Uncommon));
        }
        for _ in 0..commons {
            pool.add(Card::sample(Rarity::Common));
        }
        pool
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

    fn take(&mut self, rarity: Rarity, allow_fallback: bool) -> Res<Card> {
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
        } else if allow_fallback || rarity == Rarity::Mythic && !self.rares.is_empty() {
            if let Some(fallback) = self.replacement_rarity(rarity) {
                self.take(fallback, false)
            } else {
                err(format!("Insufficient {rarity:?}s in pool."))
            }
        } else {
            err("Insufficient cards in pool.")
        }
    }

    fn roll(&self, rarity: Rarity, allow_fallback: bool) -> Res<Card> {
        let rng = &mut thread_rng();
        let exact = match rarity {
            Rarity::Mythic => self.mythics.choose(rng),
            Rarity::Rare => self.rares.choose(rng),
            Rarity::Uncommon => self.uncommons.choose(rng),
            Rarity::Common => self.commons.choose(rng),
            Rarity::Bonus | Rarity::Special => None,
        };

        if let Some(card) = exact {
            Ok(card.clone())
        } else if allow_fallback || rarity == Rarity::Mythic && !self.rares.is_empty() {
            if let Some(fallback) = self.replacement_rarity(rarity) {
                self.roll(fallback, false)
            } else {
                err(format!("Insufficient {rarity:?}s in pool."))
            }
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

pub type Pack = Vec<Card>;

fn make_cube_packs_rarities(
    players: usize,
    config: &DraftConfig,
    mut pool: DraftPool,
) -> Res<Vec<Pack>> {
    let mut rng = thread_rng();
    pool.mythics.shuffle(&mut rng);
    pool.rares.shuffle(&mut rng);
    pool.uncommons.shuffle(&mut rng);
    pool.commons.shuffle(&mut rng);

    let mut packs = Vec::new();

    for _ in 0..(players * config.rounds) {
        let mut pack = Vec::new();

        for _ in 0..config.rares {
            if rng.gen_range(0.0..=1.0) < config.mythic_rate {
                pack.push(pool.take(Rarity::Mythic, config.allow_fallback)?);
            } else {
                pack.push(pool.take(Rarity::Rare, config.allow_fallback)?);
            }
        }

        for _ in 0..config.uncommons {
            pack.push(pool.take(Rarity::Uncommon, config.allow_fallback)?);
        }

        for _ in 0..config.commons {
            pack.push(pool.take(Rarity::Common, config.allow_fallback)?);
        }

        packs.push(pack)
    }

    Ok(packs)
}

fn make_cube_packs_no_rarities(
    players: usize,
    config: &DraftConfig,
    mut pool: DraftPool,
) -> Res<Vec<Pack>> {
    let mut cards = Vec::new();
    cards.append(&mut pool.mythics);
    cards.append(&mut pool.rares);
    cards.append(&mut pool.uncommons);
    cards.append(&mut pool.commons);
    cards.shuffle(&mut thread_rng());

    let mut packs = Vec::new();
    for _ in 0..(players * config.rounds) {
        let mut pack = Vec::new();
        for _ in 0..config.cards_per_pack {
            if let Some(card) = cards.pop() {
                pack.push(card);
            } else {
                return err("Insufficient cards in pool.");
            }
        }
        packs.push(pack);
    }

    Ok(packs)
}

fn make_draft_packs(players: usize, config: &DraftConfig, pool: DraftPool) -> Res<Vec<Pack>> {
    let rng = &mut thread_rng();
    let mut packs = Vec::new();

    for _ in 0..(players * config.rounds) {
        let mut pack = Vec::new();

        for _ in 0..config.rares {
            if rng.gen_range(0.0..=1.0) < config.mythic_rate {
                pack.push(pool.roll(Rarity::Mythic, config.allow_fallback)?);
            } else {
                pack.push(pool.roll(Rarity::Rare, config.allow_fallback)?);
            }
        }

        for _ in 0..config.uncommons {
            pack.push(pool.roll(Rarity::Uncommon, config.allow_fallback)?);
        }

        for _ in 0..config.commons {
            pack.push(pool.roll(Rarity::Common, config.allow_fallback)?);
        }

        packs.push(pack);
    }

    Ok(packs)
}

pub fn make_packs(players: usize, config: &DraftConfig, pool: DraftPool) -> Res<Vec<Pack>> {
    if config.unique_cards {
        if config.use_rarities {
            make_cube_packs_rarities(players, config, pool)
        } else {
            make_cube_packs_no_rarities(players, config, pool)
        }
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

    fn test_config() -> DraftConfig {
        DraftConfig {
            rounds: 2,
            cards_per_pack: 3,
            rares: 1,
            uncommons: 1,
            commons: 1,
            mythic_rate: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn test_make_cube_packs() {
        let config = test_config();

        let pool = DraftPool::sample(4, 4, 4, 4);
        let packs = make_packs(2, &config, pool).unwrap();
        assert!(packs.len() == 4); // 2 players, 2 packs each
        assert!(packs.iter().all(|p| p.len() == 3)); // 3 cards per pack

        // Each pack should contain a mythic as 100% of rares should be promoted
        // to mythics.
        assert!(packs
            .iter()
            .all(|p| p.iter().find(|c| c.rarity == Rarity::Mythic).is_some()));
    }

    #[test]
    fn test_make_draft_packs() {
        let mut pool = DraftPool::new();
        let mythic = Card::sample(Rarity::Rare);
        pool.add(mythic.clone());
        let uncommon = Card::sample(Rarity::Uncommon);
        pool.add(uncommon.clone());
        let common = Card::sample(Rarity::Common);
        pool.add(common.clone());

        let mut config = test_config();
        config.unique_cards = false;

        let packs = make_packs(2, &config, pool).unwrap();
        assert!(packs.len() == 4); // 2 players, 2 packs each
        assert!(packs.iter().all(|p| p.len() == 3)); // 3 cards per pack

        // Each pack should have one of each card.
        let cards = &[mythic, uncommon, common];
        assert!(packs.iter().all(|pack| cards
            .iter()
            .all(|card| pack.iter().any(|pack_card| pack_card.name() == card.name()))))
    }

    #[test]
    fn test_fail_make_packs() {
        // 1 pack with 1 rare per player.
        let mut config = test_config();
        config.allow_fallback = false;
        config.unique_cards = false; // Draft mode.
        config.rounds = 1;
        config.cards_per_pack = 1;
        config.uncommons = 0;
        config.commons = 0;

        let mut pool = DraftPool::new();
        pool.add(Card::sample(Rarity::Common));
        assert!(make_packs(1, &config, pool).is_err());
    }

    #[test]
    fn test_no_raritie_unique() {
        let pool = DraftPool::sample(1, 1, 1, 1);
        let config = DraftConfig {
            rounds: 1,
            cards_per_pack: 2,
            unique_cards: true,
            use_rarities: false,
            ..Default::default()
        };
        assert!(make_packs(2, &config, pool).is_ok());
    }
}
