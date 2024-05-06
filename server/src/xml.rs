use quick_xml::DeError;

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
struct XmlCardInner {
    name: String,
    set: XmlSetEntry,

    #[serde(default, rename = "color")]
    colour: Vec<XmlColourHolder>,

    manacost: String,
    cmc: u32,

    #[serde(rename = "type")]
    ty: String,

    pt: String,
    text: String,
}

#[derive(serde::Deserialize)]
struct XmlCard {
    #[serde(rename = "card")]
    inner: XmlCardInner,
}

#[derive(serde::Deserialize)]
pub struct XmlCardDb {
    #[serde(default)]
    sets: Vec<XmlSet>,

    #[serde(default)]
    cards: Vec<XmlCard>,
}

pub fn decode_xml_cards(xml: bytes::Bytes) -> Result<XmlCardDb, DeError> {
    quick_xml::de::from_reader(&*xml)
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

        let card = &db.cards.first().unwrap().inner;
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
        assert_eq!(card.pt, "0/1");
        assert!(card.text.starts_with("Each other Zombie"));
    }
}
