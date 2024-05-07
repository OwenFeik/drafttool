use std::fmt::Debug;

use crate::{
    cockatrice::decode_xml_cards,
    scryfall::{Card, Rarity},
};

struct CardDatabase {
    mythics: Vec<Card>,
    rares: Vec<Card>,
    uncommons: Vec<Card>,
    commons: Vec<Card>,
}

impl CardDatabase {
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

impl Debug for CardDatabase {
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
    database: CardDatabase,
    config: DraftConfig,
}

pub async fn handle_launch_request(
    mut data: axum::extract::Multipart,
) -> axum::response::Response<String> {
    use super::Resp;

    let mut cards = None;
    let mut list = None;
    let mut config = DraftConfig::new();
    while let Ok(Some(mut field)) = data.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name == "card_database" {
            match field.bytes().await {
                Ok(bytes) => match decode_xml_cards(bytes) {
                    Ok(db) => cards = Some(db),
                    Err(e) => return Resp::e422(format!("Failed to load card database: {e}")),
                },
                Err(e) => return Resp::e500(e),
            }
            continue;
        }

        let s = match field.text().await {
            Ok(s) => s,
            Err(e) => return Resp::e500(e),
        };

        match field_name.as_str() {
            "list" => list = Some(s),
            "packs" => match u32::from_str_radix(&s, 10) {
                Ok(n) => config.packs = n,
                Err(_) => return Resp::e422(format!("Invalid pack count: {s}")),
            },
            "cards_per_pack" => match u32::from_str_radix(&s, 10) {
                Ok(n) => config.cards_per_pack = n,
                Err(_) => return Resp::e422(format!("Invalid number of cards per pack: {s}")),
            },
            "unique_cards" => match s.as_str() {
                "checked" => config.unique_cards = true,
                "unchecked" => config.unique_cards = false,
                _ => return Resp::e422(format!("Invalid checkbox value for unique_cards: {s}")),
            },
            "use_rarities" => match s.as_str() {
                "checked" => config.use_rarities = true,
                "unchecked" => config.use_rarities = false,
                _ => return Resp::e422(format!("Invalid checkbox value for use_rarities: {s}")),
            },
            "mythic_incidence" => match s.parse::<f32>() {
                Ok(v) if v >= 0.0 && v <= 1.0 => config.mythic_rate = v,
                _ => return Resp::e422(format!("Invalid mythic incidence: {s}")),
            },
            "rares" => match u32::from_str_radix(&s, 10) {
                Ok(n) => config.rares = n,
                Err(_) => return Resp::e422(format!("Invalid number of rares per pack: {s}")),
            },
            "uncommons" => match u32::from_str_radix(&s, 10) {
                Ok(n) => config.uncommons = n,
                Err(_) => return Resp::e422(format!("Invalid number of commons per pack: {s}")),
            },
            "commons" => match u32::from_str_radix(&s, 10) {
                Ok(n) => config.commons = n,
                Err(_) => return Resp::e422(format!("Invalid number of commons per pack: {s}")),
            },
            _ => {}
        }
    }

    if config.rares + config.uncommons + config.commons != config.cards_per_pack {
        return Resp::e422(format!(
            "Count of rares ({}) + uncommons ({}) + commons ({}) greater than number of cards in pack ({}).",
            config.rares,
            config.uncommons,
            config.commons,
            config.cards_per_pack
        ));
    }

    let Some(mut cards) = cards else {
        return Resp::e422("No card database provided.");
    };
    let Some(list) = list else {
        return Resp::e422("No card list provided for draft.");
    };

    let mut db = CardDatabase::new();
    for line in list.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let Some(card) = cards.remove(&line.trim().to_lowercase()) else {
            return Resp::e422(format!("Card missing from database: {line}"));
        };

        db.add(card);
    }

    let lobby = DraftLobby {
        database: db,
        config,
    };

    dbg!(lobby);

    Resp::ok("ok!")
}
