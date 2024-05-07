use crate::{cards::CardDatabase, draft::DraftConfig, Resp};

use super::packs::DraftPool;

pub async fn handle_launch_request(
    carddb: std::sync::Arc<CardDatabase>,
    mut data: axum::extract::Multipart,
) -> axum::response::Response<String> {
    let mut cards = None;
    let mut list = None;
    let mut config = DraftConfig::new();
    while let Ok(Some(field)) = data.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name == "card_database" {
            match field.bytes().await {
                Ok(bytes) => match crate::cards::cockatrice::decode_xml_cards(bytes) {
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

    let Some(custom_cards) = cards else {
        return Resp::e422("No card database provided.");
    };
    let Some(list) = list else {
        return Resp::e422("No card list provided for draft.");
    };

    let mut pool = DraftPool::new();
    for line in list.lines() {
        let key = &line.trim().to_lowercase();
        if key.is_empty() {
            continue;
        }

        let Some(card) = custom_cards.get(key).or_else(|| carddb.get(key)).cloned() else {
            return Resp::e422(format!("Card not found in custom list or database: {line}"));
        };

        pool.add(card);
    }

    Resp::ok("ok!")
}
