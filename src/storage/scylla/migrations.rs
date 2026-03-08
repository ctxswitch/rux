use std::collections::BTreeMap;
use std::fmt::Write;

use scylla::client::session::Session;
use scylla::response::query_result::QueryResult;
use tracing::info;

use crate::storage::error::StorageError;

const MIGRATIONS: &[(&str, &str)] = &[(
    "001_initial_schema",
    include_str!("../../../migrations/001_initial_schema.cql"),
)];

fn build_replication_cql(replication: &BTreeMap<String, u32>) -> Result<String, StorageError> {
    if replication.is_empty() {
        return Err(StorageError::Internal(
            "replication map must contain at least one datacenter".into(),
        ));
    }
    let mut cql = String::from(
        "CREATE KEYSPACE IF NOT EXISTS rux \
         WITH replication = {'class': 'NetworkTopologyStrategy'",
    );
    for (dc, rf) in replication {
        if !dc
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(StorageError::Internal(format!(
                "invalid datacenter name in replication config: {dc}"
            )));
        }
        write!(cql, ", '{}': {}", dc, rf)
            .map_err(|e| StorageError::Internal(format!("failed to build replication CQL: {e}")))?;
    }
    cql.push_str("} AND durable_writes = true");
    Ok(cql)
}

// NOTE: Migrations are not safe for concurrent execution across multiple gateway
// instances. All current DDL is idempotent (CREATE ... IF NOT EXISTS), but future
// non-idempotent migrations should be run via a single-instance Kubernetes Job.
pub async fn run(
    session: &Session,
    replication: &BTreeMap<String, u32>,
) -> Result<(), StorageError> {
    let keyspace_cql = build_replication_cql(replication)?;
    let _: QueryResult = session
        .query_unpaged(keyspace_cql.as_str(), &[])
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

    let _: QueryResult = session
        .query_unpaged(
            "CREATE TABLE IF NOT EXISTS rux.schema_version (\
             version int, \
             applied_at timestamp, \
             description text, \
             PRIMARY KEY (version))",
            &[],
        )
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

    for (i, (description, cql)) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i32;

        let result: QueryResult = session
            .query_unpaged(
                "SELECT version FROM rux.schema_version WHERE version = ?",
                (version,),
            )
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let rows = result
            .into_rows_result()
            .map_err(|e| StorageError::Internal(format!("migration check failed: {e}")))?;

        if rows.rows_num() > 0 {
            continue;
        }

        info!(
            version = version,
            description = description,
            "applying migration"
        );

        // IMPORTANT: Naive semicolon split. Migration CQL must NOT contain semicolons
        // inside string literals, UDT definitions, or function bodies. All migration
        // statements must be idempotent (CREATE ... IF NOT EXISTS) because partial
        // failures will cause the entire migration to re-run on next startup.
        // Non-idempotent migrations must be run via a separate one-shot Kubernetes Job.
        for (stmt_idx, statement) in cql.split(';').filter(|s| !s.trim().is_empty()).enumerate() {
            let trimmed = statement.trim();
            info!(
                version = version,
                statement_index = stmt_idx,
                "executing migration statement"
            );
            let _: QueryResult = session.query_unpaged(trimmed, &[]).await.map_err(|e| {
                StorageError::Internal(format!(
                    "migration {version} statement {stmt_idx} failed: {e}"
                ))
            })?;
        }

        let _: QueryResult = session
            .query_unpaged(
                "INSERT INTO rux.schema_version (version, applied_at, description) \
                 VALUES (?, toTimestamp(now()), ?)",
                (version, *description),
            )
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_replication_cql_single_dc() {
        let mut map = BTreeMap::new();
        map.insert("dc1".to_string(), 3);
        let cql = build_replication_cql(&map).unwrap();
        assert!(cql.contains("'dc1': 3"));
        assert!(cql.contains("NetworkTopologyStrategy"));
        assert!(cql.contains("durable_writes = true"));
    }

    #[test]
    fn build_replication_cql_multiple_dcs() {
        let mut map = BTreeMap::new();
        map.insert("dc1".to_string(), 3);
        map.insert("dc2".to_string(), 2);
        let cql = build_replication_cql(&map).unwrap();
        assert!(cql.contains("'dc1': 3"));
        assert!(cql.contains("'dc2': 2"));
    }

    #[test]
    fn build_replication_cql_empty_map_errors() {
        let map = BTreeMap::new();
        assert!(build_replication_cql(&map).is_err());
    }

    #[test]
    fn build_replication_cql_rejects_invalid_dc_name() {
        let mut map = BTreeMap::new();
        map.insert("dc1'; DROP KEYSPACE rux; --".to_string(), 3);
        assert!(build_replication_cql(&map).is_err());
    }
}
