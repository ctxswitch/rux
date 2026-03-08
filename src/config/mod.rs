use std::collections::BTreeMap;

use clap::Parser;

use crate::storage::types::Consistency;

#[derive(Debug, Parser)]
#[command(name = "rux", about = "S3-compatible storage gateway")]
pub struct Config {
    #[arg(long, env = "RUX_BIND_ADDRESS", default_value = "0.0.0.0")]
    pub bind_address: String,

    #[arg(long, env = "RUX_PORT", default_value_t = 8080)]
    pub port: u16,

    #[arg(long, env = "RUX_LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    #[arg(long, env = "RUX_DEFAULT_CONSISTENCY", default_value = "eventual")]
    pub default_consistency: Consistency,

    #[command(flatten)]
    pub scylla: ScyllaConfig,
}

#[derive(Debug, Parser, Clone)]
pub struct ScyllaConfig {
    #[arg(long, env = "RUX_SCYLLA_CONTACT_POINTS", value_delimiter = ',')]
    pub contact_points: Vec<String>,

    #[arg(long, env = "RUX_SCYLLA_LOCAL_DC", default_value = "datacenter1")]
    pub local_dc: String,

    /// Replication map as comma-separated dc:rf pairs (e.g. "dc1:3,dc2:3").
    /// Defaults to local_dc with RF=3 if not set.
    #[arg(long, env = "RUX_SCYLLA_REPLICATION", value_delimiter = ',')]
    pub replication: Vec<String>,
}

pub(crate) fn validate_dc_name(dc: &str) -> Result<(), String> {
    if dc.is_empty()
        || !dc.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        || !dc
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!("invalid datacenter name: {dc}"));
    }
    Ok(())
}

impl ScyllaConfig {
    pub fn replication_map(&self) -> Result<BTreeMap<String, u32>, String> {
        if self.replication.is_empty() {
            validate_dc_name(&self.local_dc)?;
            let mut map = BTreeMap::new();
            map.insert(self.local_dc.clone(), 3);
            return Ok(map);
        }

        let mut map = BTreeMap::new();
        for entry in &self.replication {
            let (dc, rf) = entry
                .split_once(':')
                .ok_or_else(|| format!("invalid replication entry (expected dc:rf): {entry}"))?;
            let dc = dc.trim();
            validate_dc_name(dc)?;
            let rf: u32 = rf
                .trim()
                .parse()
                .map_err(|_| format!("invalid replication factor (expected 1-255) in: {entry}"))?;
            if rf == 0 || rf > 255 {
                return Err(format!(
                    "replication factor must be between 1 and 255 in: {entry}"
                ));
            }
            if map.contains_key(dc) {
                return Err(format!("duplicate datacenter in replication config: {dc}"));
            }
            map.insert(dc.to_string(), rf);
        }
        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_dc_name_accepts_valid_names() {
        assert!(validate_dc_name("datacenter1").is_ok());
        assert!(validate_dc_name("_dc").is_ok());
        assert!(validate_dc_name("dc-1").is_ok());
    }

    #[test]
    fn validate_dc_name_rejects_empty() {
        assert!(validate_dc_name("").is_err());
    }

    #[test]
    fn validate_dc_name_rejects_leading_digit() {
        assert!(validate_dc_name("1dc").is_err());
    }

    #[test]
    fn validate_dc_name_rejects_invalid_chars() {
        assert!(validate_dc_name("dc name").is_err());
        assert!(validate_dc_name("dc'inject").is_err());
    }

    #[test]
    fn replication_map_defaults_to_local_dc() {
        let config = ScyllaConfig {
            contact_points: vec![],
            local_dc: "datacenter1".into(),
            replication: vec![],
        };
        let map = config.replication_map().unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map["datacenter1"], 3);
    }

    #[test]
    fn replication_map_parses_valid_entry() {
        let config = ScyllaConfig {
            contact_points: vec![],
            local_dc: "datacenter1".into(),
            replication: vec!["dc1:3".into()],
        };
        let map = config.replication_map().unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map["dc1"], 3);
    }

    #[test]
    fn replication_map_rejects_duplicate_dcs() {
        let config = ScyllaConfig {
            contact_points: vec![],
            local_dc: "datacenter1".into(),
            replication: vec!["dc1:3".into(), "dc1:2".into()],
        };
        let err = config.replication_map().unwrap_err();
        assert!(err.contains("duplicate"));
    }

    #[test]
    fn replication_map_rejects_missing_colon() {
        let config = ScyllaConfig {
            contact_points: vec![],
            local_dc: "datacenter1".into(),
            replication: vec!["dc1".into()],
        };
        assert!(config.replication_map().is_err());
    }

    #[test]
    fn replication_map_rejects_rf_zero() {
        let config = ScyllaConfig {
            contact_points: vec![],
            local_dc: "datacenter1".into(),
            replication: vec!["dc1:0".into()],
        };
        assert!(config.replication_map().is_err());
    }

    #[test]
    fn replication_map_rejects_rf_256() {
        let config = ScyllaConfig {
            contact_points: vec![],
            local_dc: "datacenter1".into(),
            replication: vec!["dc1:256".into()],
        };
        assert!(config.replication_map().is_err());
    }
}
