use std::path::Path;

use bytes::Buf;
use serde::de::DeserializeOwned;

use crate::cards::{Card, Rarity};

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
        download_uri: String,
    }

    let info: BulkDataInfo =
        decode_json(get_bytes("https://api.scryfall.com/bulk-data/oracle-cards").await?)?;
    let raw = get_bytes(&info.download_uri).await?;

    tokio::fs::write(path, raw)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Deserialize, Debug)]
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

#[derive(serde::Deserialize, Debug)]
struct ScryfallCard {
    /// Card name. Includes both faces (!).
    name: String,

    /// Set code.
    set: String,

    /// Object containing image URIs.
    image_uris: Option<ScryfallCardImages>,

    /// Rarity string, mythic, rare, uncommon, common, special, bonus.
    rarity: String,

    /// Oracle text for the card.
    oracle_text: Option<String>,
}

impl ScryfallCard {
    fn to_card(self) -> Option<Card> {
        let name = if self.name.contains("//") {
            self.name.split("//").next().unwrap().to_string()
        } else {
            self.name
        };

        let rarity = match self.rarity.as_str() {
            "mythic" => Rarity::Rare,
            "rare" => Rarity::Rare,
            "uncommon" => Rarity::Uncommon,
            "common" => Rarity::Common,
            "special" => Rarity::Special,
            "bonus" => Rarity::Bonus,
            _ => return None,
        };

        Some(Card::new(
            name,
            self.image_uris?.choose()?,
            self.set,
            self.oracle_text?,
            rarity,
        ))
    }
}

pub async fn load_cards(data: &Path) -> Result<Vec<Card>, String> {
    tracing::debug!("Loading scryfall card data.");

    tokio::fs::create_dir_all(data)
        .await
        .map_err(|e| e.to_string())?;
    let file = data.join("scryfall-cards.json");

    if !file.exists() {
        tracing::debug!("File not found in cache, downloading to {}", file.display());
        download_list(&file).await?;
        tracing::debug!("Successfully downloaded data.");
    }

    let raw = tokio::fs::read(&file).await.map_err(|e| e.to_string())?;
    tracing::debug!("Read scryfall data from disk. Parsing JSON.");
    let cards: Vec<ScryfallCard> = decode_json(bytes::Bytes::from(raw))?;
    tracing::debug!("Converting parsed JSON into card structs.");
    Ok(cards
        .into_iter()
        .filter_map(ScryfallCard::to_card)
        .collect())
}
