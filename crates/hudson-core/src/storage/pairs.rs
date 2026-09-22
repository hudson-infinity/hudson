//! JSON object keys cannot encode composite keys; persist maps as key/value pairs.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
pub fn serialize<K: Serialize, V: Serialize, S: Serializer>(
    value: &BTreeMap<K, V>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    value.iter().collect::<Vec<_>>().serialize(serializer)
}
pub fn deserialize<'de, K: Deserialize<'de> + Ord, V: Deserialize<'de>, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<K, V>, D::Error> {
    let pairs = Vec::<(K, V)>::deserialize(deserializer)?;
    let count = pairs.len();
    let map: BTreeMap<_, _> = pairs.into_iter().collect();
    if map.len() != count {
        return Err(serde::de::Error::custom("duplicate stored key"));
    }
    Ok(map)
}
