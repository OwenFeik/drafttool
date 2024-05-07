use std::collections::HashMap;

use quick_xml::DeError;

use crate::scryfall::{Card, Rarity};

#[derive(serde::Deserialize)]
struct XmlSetInner {
    name: String,
    longname: String,
}

#[derive(serde::Deserialize)]
struct XmlSet {
    #[serde(rename = "set")]
    inner: XmlSetInner,
}

#[derive(serde::Deserialize)]
struct XmlSetEntry {
    #[serde(rename = "@rarity")]
    rarity: String,

    #[serde(rename = "@picURL")]
    image: String,

    #[serde(rename = "$text")]
    name: String,
}

#[derive(serde::Deserialize, PartialEq, Debug)]
enum XmlColour {
    W,
    U,
    B,
    R,
    G,
}

#[derive(serde::Deserialize, PartialEq, Debug)]
struct XmlColourHolder {
    #[serde(rename = "$text")]
    inner: XmlColour,
}

#[derive(serde::Deserialize)]
struct XmlCard {
    name: String,
    set: XmlSetEntry,

    #[serde(default, rename = "color")]
    colour: Vec<XmlColourHolder>,

    manacost: String,
    cmc: u32,

    #[serde(rename = "type")]
    ty: String,

    pt: Option<String>,
    text: String,
}

impl XmlCard {
    fn rarity(&self) -> Option<Rarity> {
        let rarity_str = self.set.rarity.replace(" Rare", "");
        match rarity_str.as_str() {
            "Mythic" => Some(Rarity::Mythic),
            "Rare" => Some(Rarity::Rare),
            "Uncommon" => Some(Rarity::Uncommon),
            "Common" => Some(Rarity::Common),
            _ => None,
        }
    }
}

#[derive(serde::Deserialize)]
struct XmlCardList {
    #[serde(default, rename = "card")]
    list: Vec<XmlCard>,
}

#[derive(serde::Deserialize)]
struct XmlCardDb {
    #[serde(default)]
    sets: Vec<XmlSet>,

    cards: XmlCardList,
}

/// Decode the provided cockatrice card database XML into a map from lowercased
/// card name to card object. This ensures that all cards in the database are
/// unique and handles name case normalisation for building the card list.
pub fn decode_xml_cards(data: bytes::Bytes) -> Result<HashMap<String, Card>, DeError> {
    let mut map = HashMap::new();
    let xml: XmlCardDb = quick_xml::de::from_reader(&*data)?;

    for card in xml.cards.list {
        if let Some(rarity) = card.rarity() {
            map.insert(
                card.name.to_lowercase(),
                Card::new(card.name, card.set.image, card.set.name, card.text, rarity),
            );
        }
    }

    Ok(map)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_decode() {
        const DATA: &str = r#"
<?xml version="1.0" encoding="UTF-8"?>
<cockatrice_carddatabase version="3">
  <sets>
    <set>
      <name>KR2</name>
      <longname>KR2</longname>
    </set>
  </sets>
  <cards>
    <card>
      <name>Nibbles, Corpse Companion</name>
      <set rarity="Uncommon" picURL="https://mtg.design/i/vjre15.jpg">KR2</set>
      <color>B</color>
      <color>G</color>
      <manacost>G/B</manacost>
      <cmc>1</cmc>
      <type>Legendary Creature — Zombie Squirrel</type>
      <pt>0/1</pt>
      <tablerow>1</tablerow>
      <text>Each other Zombie or Gnome creature you control enters the battlefield with an additional +1/+1 counter on it.
Pay 1 life: Regenerate Nibbles. </text>
    </card>
  </cards>
</cockatrice_carddatabase>
      "#;

        let db: XmlCardDb = quick_xml::de::from_str(DATA).unwrap();

        let set = &db.sets.first().unwrap().inner;
        assert_eq!(set.name, "KR2");
        assert_eq!(set.longname, "KR2");

        let card = &db.cards.list.first().unwrap();
        assert_eq!(card.name, "Nibbles, Corpse Companion");
        assert_eq!(card.set.rarity, "Uncommon");
        assert_eq!(card.set.image, "https://mtg.design/i/vjre15.jpg");
        assert_eq!(card.set.name, "KR2");
        assert_eq!(
            card.colour,
            vec![
                XmlColourHolder {
                    inner: XmlColour::B
                },
                XmlColourHolder {
                    inner: XmlColour::G
                }
            ]
        );
        assert_eq!(card.manacost, "G/B");
        assert_eq!(card.cmc, 1);
        assert_eq!(card.ty, "Legendary Creature — Zombie Squirrel");
        assert_eq!(card.pt.as_ref().unwrap(), "0/1");
        assert!(card.text.starts_with("Each other Zombie"));
    }

    #[test]
    fn test_reject() {
        assert!(quick_xml::de::from_str::<XmlCardDb>("<root></root>").is_err());
    }
}
