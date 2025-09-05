use std::{fs::File, io::BufReader, path::Path};

use serde::{Deserialize, Deserializer, de};

use crate::errors::Error;

#[derive(Debug, Clone, Deserialize)]
pub struct AirlineEntry {
    pub id: u32,
    pub name: String,
    #[serde(deserialize_with = "deserialize_option_string_n")]
    pub alias: Option<String>,
    #[serde(deserialize_with = "deserialize_option_string")]
    pub iata: Option<String>,
    #[serde(deserialize_with = "deserialize_option_string")]
    pub icao: Option<String>,
    #[serde(deserialize_with = "deserialize_option_string")]
    pub callsign: Option<String>,
    pub country: String,
    #[serde(deserialize_with = "deserialize_bool")]
    pub active: bool,
}

pub fn load_airlines_from_file(path: &Path) -> Result<Vec<AirlineEntry>, Error> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let res = serde_json::from_reader(reader)?;
    Ok(res)
}

fn deserialize_option_string_n<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Deserialize::deserialize(d)?;
    let res = s.filter(|s| s != "\\N");
    Ok(res)
}

fn deserialize_option_string<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Deserialize::deserialize(d)?;
    let res = s.filter(|s| !s.is_empty());
    Ok(res)
}

fn deserialize_bool<'de, D>(d: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(d)?;
    let res = match s {
        "N" => false,
        "Y" => true,
        _ => return Err(de::Error::custom(r#""Y" or "N""#)),
    };
    Ok(res)
}
