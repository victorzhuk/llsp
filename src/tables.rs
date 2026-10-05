/// Table utilities shared by configuration and dialect loading, so neither of
/// those modules has to depend on the other.
use anyhow::{Result, bail};

/// Deep-merges `over` into `base`: tables merge recursively, anything else is replaced.
pub fn merge_tables(base: &mut toml::Table, over: toml::Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge_tables(b, o),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

/// Converts JSON settings (initializationOptions, didChangeConfiguration) into a
/// TOML table; `null` values are dropped and non-objects at the top level error.
pub fn json_to_table(value: serde_json::Value) -> Result<toml::Table> {
    match json_to_toml(value) {
        Some(toml::Value::Table(t)) => Ok(t),
        None => Ok(toml::Table::new()),
        Some(other) => bail!("expected an object, got {}", other.type_str()),
    }
}

fn json_to_toml(value: serde_json::Value) -> Option<toml::Value> {
    use serde_json::Value as J;
    Some(match value {
        J::Null => return None,
        J::Bool(b) => toml::Value::Boolean(b),
        J::Number(n) => match n.as_i64() {
            Some(i) => toml::Value::Integer(i),
            None => toml::Value::Float(n.as_f64()?),
        },
        J::String(s) => toml::Value::String(s),
        J::Array(a) => toml::Value::Array(a.into_iter().filter_map(json_to_toml).collect()),
        J::Object(o) => toml::Value::Table(
            o.into_iter()
                .filter_map(|(k, v)| Some((k, json_to_toml(v)?)))
                .collect(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_conversion() {
        let t = json_to_table(serde_json::json!({"log": {"file": null, "level": "info"}})).unwrap();
        assert_eq!(t["log"]["level"].as_str(), Some("info"));
        assert!(t["log"].get("file").is_none());
        assert!(json_to_table(serde_json::json!([1])).is_err());
        assert!(json_to_table(serde_json::Value::Null).unwrap().is_empty());
    }

    #[test]
    fn merge_is_recursive() {
        let mut base: toml::Table = toml::from_str("[a]\nx = 1\ny = 2").unwrap();
        let over: toml::Table = toml::from_str("[a]\ny = 3").unwrap();
        merge_tables(&mut base, over);
        assert_eq!(base["a"]["x"].as_integer(), Some(1));
        assert_eq!(base["a"]["y"].as_integer(), Some(3));
    }
}
