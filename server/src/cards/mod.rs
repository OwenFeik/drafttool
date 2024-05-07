use std::collections::HashMap;

pub mod cockatrice;
pub mod scryfall;

#[derive(Clone, Debug)]
pub enum Rarity {
    Mythic,
    Rare,
    Uncommon,
    Common,
    Special,
    Bonus,
}

#[derive(Clone, Debug)]
pub struct Card {
    name: String,
    image: String,
    set: String,
    pub rarity: Rarity,
    text: String,
}

impl Card {
    pub fn new(name: String, image: String, set: String, text: String, rarity: Rarity) -> Self {
        Self {
            name,
            image,
            set,
            rarity,
            text,
        }
    }
}

pub struct CardDatabase {
    /// Map from lowercased card name to card.
    name_to_card: HashMap<String, Card>,
}

impl CardDatabase {
    pub fn new() -> Self {
        Self {
            name_to_card: HashMap::new(),
        }
    }

    pub fn add(&mut self, card: Card) {
        // Add a mapping from this cards name to the set that its in.
        let key = card.name.to_ascii_lowercase();
        self.name_to_card.insert(key, card);
    }

    pub fn get(&self, name: &str) -> Option<&Card> {
        self.name_to_card.get(&name.to_ascii_lowercase())
    }

    pub fn size(&self) -> usize {
        self.name_to_card.len()
    }
}
