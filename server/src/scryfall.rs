use std::{collections::HashMap, path::Path};

use bytes::Buf;
use serde::de::DeserializeOwned;

pub enum Rarity {
    Mythic,
    Rare,
    Uncommon,
    Common,
    Special,
    Bonus,
}

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

struct CardDatabase {
    sets: HashMap<String, Vec<Card>>,
    name_to_set: HashMap<String, Vec<String>>,
}

async fn get_bytes(uri: &str) -> Result<bytes::Bytes, String> {
    reqwest::get(uri)
        .await
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())
}

fn decode_json<T: DeserializeOwned>(bytes: bytes::Bytes) -> Result<T, String> {
    serde_json::de::from_reader(bytes.reader()).map_err(|e| e.to_string())
}

async fn download_list(path: &Path) -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct BulkDataInfo {
        uri: String,
    }

    let info: BulkDataInfo =
        decode_json(get_bytes("https://api.scryfall.com/bulk-data/oracle-cards").await?)?;
    let raw = get_bytes(&info.uri).await?;

    tokio::fs::write(path, raw)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Deserialize)]
struct ScryfallCardImages {
    png: Option<String>,
    border_crop: Option<String>,
    art_crop: Option<String>,
    large: Option<String>,
    normal: Option<String>,
    small: Option<String>,
}

impl ScryfallCardImages {
    fn choose(self) -> Option<String> {
        if self.large.is_some() {
            self.large
        } else if self.png.is_some() {
            self.png
        } else if self.normal.is_some() {
            self.normal
        } else if self.border_crop.is_some() {
            self.border_crop
        } else if self.small.is_some() {
            self.small
        } else {
            self.art_crop
        }
    }
}

#[derive(serde::Deserialize)]
struct ScryfallCard {
    /// Card name. Includes both faces (!).
    name: String,

    /// Set code.
    set: String,

    /// Object containing image URIs.
    image_uris: ScryfallCardImages,

    /// Rarity string, mythic, rare, uncommon, common, special, bonus.
    rarity: String,

    /// Oracle text for the card.
    text: Option<String>,
}

impl ScryfallCard {
    fn to_card(self) -> Option<Card> {
        let name = if self.name.contains("//") {
            self.name.split("//").next().unwrap().to_string()
        } else {
            self.name
        };

        let rarity = match self.rarity.as_str() {
            "mythic" => Rarity::Mythic,
            "rare" => Rarity::Rare,
            "uncommon" => Rarity::Uncommon,
            "common" => Rarity::Common,
            "special" => Rarity::Special,
            "bonus" => Rarity::Bonus,
            _ => return None,
        };

        Some(Card {
            name,
            set: self.set,
            image: self.image_uris.choose()?,
            rarity,
            text: self.text?
        })
    }
}

async fn load_cards(data: &Path) -> Result<CardDatabase, String> {
    let file = data.join("scryfall-cards.json");

    if !file.exists() {
        download_list(&file).await?;
    }

    let raw = tokio::fs::read(&file).await.map_err(|e| e.to_string())?;
    let cards: Vec<ScryfallCard> = decode_json(bytes::Bytes::from(raw))?;

    Err("failed :(".to_string())
}
