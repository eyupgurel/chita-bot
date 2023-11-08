use std::fmt;
use serde::{de, Deserialize, Deserializer};
use serde::de::{SeqAccess, Visitor};

#[derive(Debug, Clone)]
pub struct OrderBook {
    pub asks: Vec<(f64, f64)>,
    pub bids: Vec<(f64, f64)>,
}
pub fn deserialize_as_string_tuples<'de, D>(deserializer: D) -> Result<Vec<(f64, f64)>, D::Error>
    where
        D: Deserializer<'de>,
{
    let string_tuples: Vec<(String, String)> = Vec::deserialize(deserializer)?;

    let mut number_tuples: Vec<(f64, f64)> = Vec::with_capacity(string_tuples.len());
    for (s1, s2) in string_tuples {
        let n1 = s1.parse::<f64>().map_err(serde::de::Error::custom)?;
        let n2 = s2.parse::<f64>().map_err(serde::de::Error::custom)?;
        number_tuples.push((n1, n2));
    }

    Ok(number_tuples)
}

pub fn deserialize_as_mix_tuples<'de, D>(deserializer: D) -> Result<Vec<(f64, f64)>, D::Error>
    where
        D: Deserializer<'de>,
{
    // Assuming this visitor is defined somewhere in your code.
    struct StringTupleVisitor;

    impl<'de> Visitor<'de> for StringTupleVisitor {
        type Value = Vec<(f64, f64)>;

        // Existing implementation for expecting and visit_seq functions.

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a list of price-amount tuples where the amount can be a string or a number")
        }
        fn visit_seq<S>(self, mut seq: S) -> Result<Vec<(f64, f64)>, S::Error>
            where
                S: SeqAccess<'de>,
        {
            let mut tuples = Vec::new();

            while let Some((price, amount)) = seq.next_element::<(String, serde_json::Value)>()? {
                let price_parsed = price.parse::<f64>().map_err(de::Error::custom)?;
                let amount_parsed = match amount {
                    serde_json::Value::String(s) => s.parse::<f64>().map_err(de::Error::custom)?,
                    serde_json::Value::Number(n) => n.as_f64().ok_or_else(|| de::Error::custom("Invalid number"))?,
                    _ => return Err(de::Error::custom("Invalid type for amount")),
                };
                tuples.push((price_parsed, amount_parsed));
            }

            Ok(tuples)
        }

    }

    deserializer.deserialize_seq(StringTupleVisitor)
}
