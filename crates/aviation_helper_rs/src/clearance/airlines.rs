use serde::{Deserialize, Deserializer, de};

use crate::errors::Error;
#[cfg(feature = "fs")]
use std::{fs::File, io::BufReader};

#[derive(Debug, Clone, Deserialize)]
pub struct AirlineEntry {
    #[serde(deserialize_with = "deserialize_string_as_i32")]
    pub id: i32,
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

#[derive(Debug, Clone, Deserialize)]
pub struct Airlines(pub Vec<AirlineEntry>);

impl Airlines {
    pub fn load_airlines<R>(reader: R) -> Result<Self, Error>
    where
        R: std::io::Read,
    {
        let res = serde_json::from_reader(reader)?;
        Ok(res)
    }

    #[cfg(feature = "fs")]
    pub fn load_airlines_from_file() -> Result<Self, Error> {
        let file = File::open(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/known-strings/airlines.json"
        ))
        .unwrap();
        let reader = BufReader::new(file);
        let res = serde_json::from_reader(reader)?;
        Ok(res)
    }
}

fn deserialize_string_as_i32<'de, D>(d: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(d)?;
    let res = s.parse().map_err(|err| {
        serde::de::Error::custom(format!("Could not parse u32: {s}, Err: {err:?}"))
    })?;
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
    let s: String = Deserialize::deserialize(d)?;
    let res = match s.as_str() {
        "N" => false,
        "Y" => true,
        "n" => false,
        "y" => true,
        _ => {
            return Err(de::Error::invalid_value(
                de::Unexpected::Str(&s),
                &r#""Y" or "N""#,
            ));
        }
    };
    Ok(res)
}
