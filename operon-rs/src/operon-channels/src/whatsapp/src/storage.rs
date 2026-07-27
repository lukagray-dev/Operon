//! SQLite-backed storage engine for whatsapp-rust session and key persistence.

use async_trait::async_trait;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Arc;

use bytes::Bytes;
use prost::Message;
use wacore::appstate::hash::HashState;
use wacore::appstate::processor::AppStateMutationMAC;
use wacore::store::traits::DeviceInfo;
use wacore::store::traits::DeviceStore as DeviceStoreTrait;
use wacore::store::traits::*;
use wacore::store::Device as CoreDevice;

#[derive(Clone)]
pub struct RusqliteStore {
    /// Database file path
    db_path: String,
    /// SQLite connection (thread-safe via Mutex)
    conn: Arc<Mutex<Connection>>,
    /// Device ID for this session
    device_id: i32,
}

macro_rules! to_store_err {
    // For expressions returning Result<usize, E>
    (execute: $expr:expr) => {
        $expr.map(|_| ()).map_err(|e| {
            wacore::store::error::StoreError::Database(
                Box::new(e) as Box<dyn std::error::Error + Send + Sync>
            )
        })
    };
    // For other expressions
    ($expr:expr) => {
        $expr.map_err(|e| {
            wacore::store::error::StoreError::Database(
                Box::new(e) as Box<dyn std::error::Error + Send + Sync>
            )
        })
    };
}

/// Device ID used for single-session stores; every store created by
/// [`RusqliteStore::new`] persists its device under this row id.
const DEFAULT_DEVICE_ID: i32 = 1;

/// Used by [`DeviceStoreTrait::exists`]: "does the store hold a device row to load?"
const DEVICE_EXISTS_SQL: &str = "SELECT COUNT(*) FROM device WHERE id = ?1";

/// Used by [`persisted_device_exists`]: "does the store hold a *linked* device?"
const LINKED_DEVICE_EXISTS_SQL: &str =
    "SELECT COUNT(*) FROM device WHERE id = ?1 AND pn IS NOT NULL";

/// Reports whether the session database holds a device linked to a WhatsApp account.
pub fn persisted_device_exists<P: AsRef<Path>>(db_path: P) -> bool {
    let path = db_path.as_ref();
    if !path.is_file() {
        return false;
    }
    let Ok(conn) = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return false;
    };
    conn.query_row(
        LINKED_DEVICE_EXISTS_SQL,
        params![DEFAULT_DEVICE_ID],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

impl RusqliteStore {
    pub fn new<P: AsRef<Path>>(db_path: P) -> anyhow::Result<Self> {
        let db_path = db_path.as_ref().to_string_lossy().to_string();

        // Create parent directory if needed
        if let Some(parent) = Path::new(&db_path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;

        // Enable WAL mode for better concurrency
        to_store_err!(conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        ))?;

        let store = Self {
            db_path,
            conn: Arc::new(Mutex::new(conn)),
            device_id: DEFAULT_DEVICE_ID,
        };

        store.init_schema()?;

        Ok(store)
    }

    /// Initialize all database tables
    fn init_schema(&self) -> anyhow::Result<()> {
        let mut conn = self.conn.lock();

        let needs_raw_id = {
            let mut stmt = conn.prepare("PRAGMA table_info(device_registry)")?;
            let mut has_raw_id = false;
            let mut table_exists = false;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            for r in rows {
                table_exists = true;
                if r? == "raw_id" {
                    has_raw_id = true;
                    break;
                }
            }
            table_exists && !has_raw_id
        };

        let device_06_migrations: Vec<(&'static str, &'static str)> = {
            let mut existing: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut stmt = conn.prepare("PRAGMA table_info(device)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            for r in rows {
                existing.insert(r?);
            }
            const ALL: &[(&str, &str)] = &[
                ("next_pre_key_id", "INTEGER NOT NULL DEFAULT 0"),
                ("server_has_prekeys", "INTEGER NOT NULL DEFAULT 0"),
                ("nct_salt", "BLOB"),
                ("server_cert_chain", "BLOB"),
                ("login_counter", "INTEGER NOT NULL DEFAULT 0"),
            ];
            if existing.is_empty() {
                Vec::new()
            } else {
                ALL.iter()
                    .copied()
                    .filter(|(col, _)| !existing.contains(*col))
                    .collect()
            }
        };

        let tx = to_store_err!(conn.transaction())?;

        to_store_err!(tx.execute_batch(
            "-- Main device table
            CREATE TABLE IF NOT EXISTS device (
                id INTEGER PRIMARY KEY,
                lid TEXT,
                pn TEXT,
                registration_id INTEGER NOT NULL,
                noise_key BLOB NOT NULL,
                identity_key BLOB NOT NULL,
                signed_pre_key BLOB NOT NULL,
                signed_pre_key_id INTEGER NOT NULL,
                signed_pre_key_signature BLOB NOT NULL,
                adv_secret_key BLOB NOT NULL,
                account BLOB,
                push_name TEXT NOT NULL,
                app_version_primary INTEGER NOT NULL,
                app_version_secondary INTEGER NOT NULL,
                app_version_tertiary INTEGER NOT NULL,
                app_version_last_fetched_ms INTEGER NOT NULL,
                edge_routing_info BLOB,
                props_hash TEXT,
                next_pre_key_id INTEGER NOT NULL DEFAULT 0,
                server_has_prekeys INTEGER NOT NULL DEFAULT 0,
                nct_salt BLOB,
                server_cert_chain BLOB,
                login_counter INTEGER NOT NULL DEFAULT 0
            );

            -- Signal identity keys
            CREATE TABLE IF NOT EXISTS identities (
                address TEXT NOT NULL,
                key BLOB NOT NULL,
                device_id INTEGER NOT NULL,
                PRIMARY KEY (address, device_id)
            );

            -- Signal protocol sessions
            CREATE TABLE IF NOT EXISTS sessions (
                address TEXT NOT NULL,
                record BLOB NOT NULL,
                device_id INTEGER NOT NULL,
                PRIMARY KEY (address, device_id)
            );

            -- Pre-keys for key exchange
            CREATE TABLE IF NOT EXISTS prekeys (
                id INTEGER NOT NULL,
                key BLOB NOT NULL,
                uploaded INTEGER NOT NULL DEFAULT 0,
                device_id INTEGER NOT NULL,
                PRIMARY KEY (id, device_id)
            );

            -- Signed pre-keys
            CREATE TABLE IF NOT EXISTS signed_prekeys (
                id INTEGER NOT NULL,
                record BLOB NOT NULL,
                device_id INTEGER NOT NULL,
                PRIMARY KEY (id, device_id)
            );

            -- Sender keys for group messaging
            CREATE TABLE IF NOT EXISTS sender_keys (
                address TEXT NOT NULL,
                record BLOB NOT NULL,
                device_id INTEGER NOT NULL,
                PRIMARY KEY (address, device_id)
            );

            -- App state sync keys
            CREATE TABLE IF NOT EXISTS app_state_keys (
                key_id BLOB NOT NULL,
                key_data BLOB NOT NULL,
                device_id INTEGER NOT NULL,
                PRIMARY KEY (key_id, device_id)
            );

            -- App state versions
            CREATE TABLE IF NOT EXISTS app_state_versions (
                name TEXT NOT NULL,
                state_data BLOB NOT NULL,
                device_id INTEGER NOT NULL,
                PRIMARY KEY (name, device_id)
            );

            -- App state mutation MACs
            CREATE TABLE IF NOT EXISTS app_state_mutation_macs (
                name TEXT NOT NULL,
                version INTEGER NOT NULL,
                index_mac BLOB NOT NULL,
                value_mac BLOB NOT NULL,
                device_id INTEGER NOT NULL,
                PRIMARY KEY (name, index_mac, device_id)
            );

            -- LID to phone number mapping
            CREATE TABLE IF NOT EXISTS lid_pn_mapping (
                lid TEXT NOT NULL,
                phone_number TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                learning_source TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                device_id INTEGER NOT NULL,
                PRIMARY KEY (lid, device_id)
            );

            -- SKDM recipients tracking
            CREATE TABLE IF NOT EXISTS skdm_recipients (
                group_jid TEXT NOT NULL,
                device_jid TEXT NOT NULL,
                device_id INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY (group_jid, device_jid, device_id)
            );

            -- Device registry for multi-device
            CREATE TABLE IF NOT EXISTS device_registry (
                user_id TEXT NOT NULL,
                devices_json TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                phash TEXT,
                raw_id INTEGER,
                device_id INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (user_id, device_id)
            );

            -- Per-device sender-key tracking
            CREATE TABLE IF NOT EXISTS sender_key_devices (
                group_jid TEXT NOT NULL,
                device_jid TEXT NOT NULL,
                has_key INTEGER NOT NULL,
                device_id INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (group_jid, device_jid, device_id)
            );

            -- Sent message retry store
            CREATE TABLE IF NOT EXISTS sent_messages (
                chat_jid TEXT NOT NULL,
                message_id TEXT NOT NULL,
                payload BLOB NOT NULL,
                device_id INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY (chat_jid, message_id, device_id)
            );

            -- Base keys for collision detection
            CREATE TABLE IF NOT EXISTS base_keys (
                address TEXT NOT NULL,
                message_id TEXT NOT NULL,
                base_key BLOB NOT NULL,
                device_id INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY (address, message_id, device_id)
            );

            -- Sender key status for lazy deletion
            CREATE TABLE IF NOT EXISTS sender_key_status (
                group_jid TEXT NOT NULL,
                participant TEXT NOT NULL,
                device_id INTEGER NOT NULL,
                marked_at INTEGER NOT NULL,
                PRIMARY KEY (group_jid, participant, device_id)
            );

            -- Trusted contact tokens
            CREATE TABLE IF NOT EXISTS tc_tokens (
                jid TEXT NOT NULL,
                token BLOB NOT NULL,
                token_timestamp INTEGER NOT NULL,
                sender_timestamp INTEGER,
                device_id INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (jid, device_id)
            );

            CREATE INDEX IF NOT EXISTS idx_sent_messages_device_created
                ON sent_messages(device_id, created_at);",
        ))?;

        if needs_raw_id {
            to_store_err!(execute: tx.execute(
                "ALTER TABLE device_registry ADD COLUMN raw_id INTEGER",
                [],
            ))?;
        }

        for (col, ty) in &device_06_migrations {
            to_store_err!(execute: tx.execute(
                &format!("ALTER TABLE device ADD COLUMN {col} {ty}"),
                [],
            ))?;
        }

        to_store_err!(tx.commit())?;
        Ok(())
    }
}

#[async_trait]
impl SignalStore for RusqliteStore {
    async fn put_identity(&self, address: &str, key: [u8; 32]) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "INSERT OR REPLACE INTO identities (address, key, device_id)
             VALUES (?1, ?2, ?3)",
            params![address, key.to_vec(), self.device_id],
        ))
    }

    async fn load_identity(&self, address: &str) -> wacore::store::error::Result<Option<[u8; 32]>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT key FROM identities WHERE address = ?1 AND device_id = ?2",
            params![address, self.device_id],
            |row| row.get::<_, Vec<u8>>(0),
        );

        match result {
            Ok(key) => {
                if key.len() != 32 {
                    return Err(wacore::store::error::StoreError::Validation(format!(
                        "identity key has invalid length {}, expected 32",
                        key.len()
                    )));
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&key);
                Ok(Some(arr))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn delete_identity(&self, address: &str) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "DELETE FROM identities WHERE address = ?1 AND device_id = ?2",
            params![address, self.device_id],
        ))
    }

    async fn get_session(&self, address: &str) -> wacore::store::error::Result<Option<Bytes>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT record FROM sessions WHERE address = ?1 AND device_id = ?2",
            params![address, self.device_id],
            |row| row.get::<_, Vec<u8>>(0),
        );

        match result {
            Ok(record) => Ok(Some(Bytes::from(record))),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn put_session(&self, address: &str, session: &[u8]) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "INSERT OR REPLACE INTO sessions (address, record, device_id)
             VALUES (?1, ?2, ?3)",
            params![address, session, self.device_id],
        ))
    }

    async fn delete_session(&self, address: &str) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "DELETE FROM sessions WHERE address = ?1 AND device_id = ?2",
            params![address, self.device_id],
        ))
    }

    async fn store_prekey(
        &self,
        id: u32,
        record: &[u8],
        uploaded: bool,
    ) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "INSERT OR REPLACE INTO prekeys (id, key, uploaded, device_id)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, record, uploaded, self.device_id],
        ))
    }

    async fn load_prekey(&self, id: u32) -> wacore::store::error::Result<Option<Bytes>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT key FROM prekeys WHERE id = ?1 AND device_id = ?2",
            params![id, self.device_id],
            |row| row.get::<_, Vec<u8>>(0),
        );

        match result {
            Ok(key) => Ok(Some(Bytes::from(key))),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn get_max_prekey_id(&self) -> wacore::store::error::Result<u32> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT MAX(id) FROM prekeys WHERE device_id = ?1",
            params![self.device_id],
            |row| row.get::<_, Option<i64>>(0),
        );

        match result {
            Ok(Some(id)) => Ok(u32::try_from(id).unwrap_or(0)),
            Ok(None) => Ok(0),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn remove_prekey(&self, id: u32) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "DELETE FROM prekeys WHERE id = ?1 AND device_id = ?2",
            params![id, self.device_id],
        ))
    }

    async fn store_signed_prekey(
        &self,
        id: u32,
        record: &[u8],
    ) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "INSERT OR REPLACE INTO signed_prekeys (id, record, device_id)
             VALUES (?1, ?2, ?3)",
            params![id, record, self.device_id],
        ))
    }

    async fn load_signed_prekey(&self, id: u32) -> wacore::store::error::Result<Option<Vec<u8>>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT record FROM signed_prekeys WHERE id = ?1 AND device_id = ?2",
            params![id, self.device_id],
            |row| row.get::<_, Vec<u8>>(0),
        );

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn load_all_signed_prekeys(&self) -> wacore::store::error::Result<Vec<(u32, Vec<u8>)>> {
        let conn = self.conn.lock();
        let mut stmt = to_store_err!(
            conn.prepare("SELECT id, record FROM signed_prekeys WHERE device_id = ?1")
        )?;

        let rows = to_store_err!(stmt.query_map(params![self.device_id], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, Vec<u8>>(1)?))
        }))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(to_store_err!(row)?);
        }

        Ok(result)
    }

    async fn remove_signed_prekey(&self, id: u32) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "DELETE FROM signed_prekeys WHERE id = ?1 AND device_id = ?2",
            params![id, self.device_id],
        ))
    }

    async fn put_sender_key(
        &self,
        address: &str,
        record: &[u8],
    ) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "INSERT OR REPLACE INTO sender_keys (address, record, device_id)
             VALUES (?1, ?2, ?3)",
            params![address, record, self.device_id],
        ))
    }

    async fn get_sender_key(&self, address: &str) -> wacore::store::error::Result<Option<Vec<u8>>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT record FROM sender_keys WHERE address = ?1 AND device_id = ?2",
            params![address, self.device_id],
            |row| row.get::<_, Vec<u8>>(0),
        );

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn delete_sender_key(&self, address: &str) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "DELETE FROM sender_keys WHERE address = ?1 AND device_id = ?2",
            params![address, self.device_id],
        ))
    }
}

#[async_trait]
impl AppSyncStore for RusqliteStore {
    async fn get_sync_key(
        &self,
        key_id: &[u8],
    ) -> wacore::store::error::Result<Option<AppStateSyncKey>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT key_data FROM app_state_keys WHERE key_id = ?1 AND device_id = ?2",
            params![key_id, self.device_id],
            |row| {
                let key_data: Vec<u8> = row.get(0)?;
                serde_json::from_slice(&key_data)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
            },
        );

        match result {
            Ok(key) => Ok(Some(key)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn set_sync_key(
        &self,
        key_id: &[u8],
        key: AppStateSyncKey,
    ) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        let key_data = to_store_err!(serde_json::to_vec(&key))?;

        to_store_err!(execute: conn.execute(
            "INSERT OR REPLACE INTO app_state_keys (key_id, key_data, device_id)
             VALUES (?1, ?2, ?3)",
            params![key_id, key_data, self.device_id],
        ))
    }

    async fn get_version(&self, name: &str) -> wacore::store::error::Result<HashState> {
        let conn = self.conn.lock();
        match conn.query_row(
            "SELECT state_data FROM app_state_versions WHERE name = ?1 AND device_id = ?2",
            params![name, self.device_id],
            |row| row.get::<_, Vec<u8>>(0),
        ) {
            Ok(state_data) => to_store_err!(serde_json::from_slice(&state_data)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(HashState::default()),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn set_version(&self, name: &str, state: HashState) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        let state_data = to_store_err!(serde_json::to_vec(&state))?;

        to_store_err!(execute: conn.execute(
            "INSERT OR REPLACE INTO app_state_versions (name, state_data, device_id)
             VALUES (?1, ?2, ?3)",
            params![name, state_data, self.device_id],
        ))
    }

    async fn put_mutation_macs(
        &self,
        name: &str,
        version: u64,
        mutations: &[AppStateMutationMAC],
    ) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();

        for mutation in mutations {
            to_store_err!(execute: conn.execute(
                "INSERT OR REPLACE INTO app_state_mutation_macs
                 (name, version, index_mac, value_mac, device_id)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![name, i64::try_from(version).unwrap_or(i64::MAX), mutation.index_mac, mutation.value_mac, self.device_id],
            ))?;
        }

        Ok(())
    }

    async fn get_mutation_mac(
        &self,
        name: &str,
        index_mac: &[u8],
    ) -> wacore::store::error::Result<Option<Vec<u8>>> {
        let conn = self.conn.lock();

        let result = conn.query_row(
            "SELECT value_mac FROM app_state_mutation_macs
             WHERE name = ?1 AND index_mac = ?2 AND device_id = ?3",
            params![name, index_mac, self.device_id],
            |row| row.get::<_, Vec<u8>>(0),
        );

        match result {
            Ok(mac) => Ok(Some(mac)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn delete_mutation_macs(
        &self,
        name: &str,
        index_macs: &[Vec<u8>],
    ) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();

        for index_mac in index_macs {
            to_store_err!(execute: conn.execute(
                "DELETE FROM app_state_mutation_macs
                 WHERE name = ?1 AND index_mac = ?2 AND device_id = ?3",
                params![name, index_mac, self.device_id],
            ))?;
        }

        Ok(())
    }

    async fn get_latest_sync_key_id(&self) -> wacore::store::error::Result<Option<Vec<u8>>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT key_id FROM app_state_keys
             WHERE device_id = ?1
             ORDER BY key_id DESC
             LIMIT 1",
            params![self.device_id],
            |row| row.get::<_, Vec<u8>>(0),
        );

        match result {
            Ok(key_id) => Ok(Some(key_id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }
}

#[async_trait]
impl ProtocolStore for RusqliteStore {
    async fn get_sender_key_devices(
        &self,
        group_jid: &str,
    ) -> wacore::store::error::Result<Vec<(String, bool)>> {
        let conn = self.conn.lock();
        let mut stmt = to_store_err!(conn.prepare(
            "SELECT device_jid, has_key FROM sender_key_devices
             WHERE group_jid = ?1 AND device_id = ?2"
        ))?;

        let rows = to_store_err!(stmt.query_map(params![group_jid, self.device_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        }))?;

        let mut result = Vec::new();
        for row in rows {
            let (device_jid, has_key) = to_store_err!(row)?;
            result.push((device_jid, has_key != 0));
        }

        Ok(result)
    }

    async fn set_sender_key_status(
        &self,
        group_jid: &str,
        entries: &[(&str, bool)],
    ) -> wacore::store::error::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp();

        let tx = to_store_err!(conn.transaction())?;

        for (device_jid, has_key) in entries {
            to_store_err!(execute: tx.execute(
                "INSERT INTO sender_key_devices
                 (group_jid, device_jid, has_key, device_id, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(group_jid, device_jid, device_id) DO UPDATE SET
                   has_key = excluded.has_key,
                   updated_at = excluded.updated_at",
                params![
                    group_jid,
                    device_jid,
                    if *has_key { 1_i64 } else { 0_i64 },
                    self.device_id,
                    now,
                ],
            ))?;
        }

        to_store_err!(tx.commit())?;
        Ok(())
    }

    async fn clear_sender_key_devices(&self, group_jid: &str) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "DELETE FROM sender_key_devices WHERE group_jid = ?1 AND device_id = ?2",
            params![group_jid, self.device_id],
        ))
    }

    async fn delete_sender_key_device_rows(
        &self,
        device_jids: &[&str],
    ) -> wacore::store::error::Result<()> {
        if device_jids.is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock();
        for device_jid in device_jids {
            to_store_err!(execute: conn.execute(
                "DELETE FROM sender_key_devices
                 WHERE device_jid = ?1 AND device_id = ?2",
                params![device_jid, self.device_id],
            ))?;
        }
        Ok(())
    }

    async fn clear_all_sender_key_devices(&self) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "DELETE FROM sender_key_devices WHERE device_id = ?1",
            params![self.device_id],
        ))
    }

    async fn get_lid_mapping(
        &self,
        lid: &str,
    ) -> wacore::store::error::Result<Option<LidPnMappingEntry>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT lid, phone_number, created_at, learning_source, updated_at
             FROM lid_pn_mapping WHERE lid = ?1 AND device_id = ?2",
            params![lid, self.device_id],
            |row| {
                Ok(LidPnMappingEntry {
                    lid: row.get(0)?,
                    phone_number: row.get(1)?,
                    created_at: row.get(2)?,
                    learning_source: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        );

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn get_pn_mapping(
        &self,
        phone: &str,
    ) -> wacore::store::error::Result<Option<LidPnMappingEntry>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT lid, phone_number, created_at, learning_source, updated_at
             FROM lid_pn_mapping WHERE phone_number = ?1 AND device_id = ?2
             ORDER BY updated_at DESC LIMIT 1",
            params![phone, self.device_id],
            |row| {
                Ok(LidPnMappingEntry {
                    lid: row.get(0)?,
                    phone_number: row.get(1)?,
                    created_at: row.get(2)?,
                    learning_source: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        );

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn put_lid_mapping(&self, entry: &LidPnMappingEntry) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "INSERT OR REPLACE INTO lid_pn_mapping
             (lid, phone_number, created_at, learning_source, updated_at, device_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                entry.lid,
                entry.phone_number,
                entry.created_at,
                entry.learning_source,
                entry.updated_at,
                self.device_id,
            ],
        ))
    }

    async fn get_all_lid_mappings(&self) -> wacore::store::error::Result<Vec<LidPnMappingEntry>> {
        let conn = self.conn.lock();
        let mut stmt = to_store_err!(conn.prepare(
            "SELECT lid, phone_number, created_at, learning_source, updated_at
             FROM lid_pn_mapping WHERE device_id = ?1"
        ))?;

        let rows = to_store_err!(stmt.query_map(params![self.device_id], |row| {
            Ok(LidPnMappingEntry {
                lid: row.get(0)?,
                phone_number: row.get(1)?,
                created_at: row.get(2)?,
                learning_source: row.get(3)?,
                updated_at: row.get(4)?,
            })
        }))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(to_store_err!(row)?);
        }

        Ok(result)
    }

    async fn save_base_key(
        &self,
        address: &str,
        message_id: &str,
        base_key: &[u8],
    ) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp();

        to_store_err!(execute: conn.execute(
            "INSERT OR REPLACE INTO base_keys (address, message_id, base_key, device_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![address, message_id, base_key, self.device_id, now],
        ))
    }

    async fn has_same_base_key(
        &self,
        address: &str,
        message_id: &str,
        current_base_key: &[u8],
    ) -> wacore::store::error::Result<bool> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT base_key FROM base_keys
             WHERE address = ?1 AND message_id = ?2 AND device_id = ?3",
            params![address, message_id, self.device_id],
            |row| {
                let saved_key: Vec<u8> = row.get(0)?;
                Ok(saved_key == current_base_key)
            },
        );

        match result {
            Ok(same) => Ok(same),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn delete_base_key(
        &self,
        address: &str,
        message_id: &str,
    ) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "DELETE FROM base_keys WHERE address = ?1 AND message_id = ?2 AND device_id = ?3",
            params![address, message_id, self.device_id],
        ))
    }

    async fn update_device_list(
        &self,
        record: DeviceListRecord,
    ) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        let devices_json = to_store_err!(serde_json::to_string(&record.devices))?;
        let now = chrono::Utc::now().timestamp();

        to_store_err!(execute: conn.execute(
            "INSERT OR REPLACE INTO device_registry
             (user_id, devices_json, timestamp, phash, raw_id, device_id, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                record.user,
                devices_json,
                record.timestamp,
                record.phash,
                record.raw_id.map(|r| r as i64),
                self.device_id,
                now,
            ],
        ))
    }

    async fn get_devices(
        &self,
        user: &str,
    ) -> wacore::store::error::Result<Option<DeviceListRecord>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT user_id, devices_json, timestamp, phash, raw_id
             FROM device_registry WHERE user_id = ?1 AND device_id = ?2",
            params![user, self.device_id],
            |row| {
                fn to_rusqlite_err<E: std::error::Error + Send + Sync + 'static>(
                    e: E,
                ) -> rusqlite::Error {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                }

                let devices_json: String = row.get(1)?;
                let devices: Vec<DeviceInfo> =
                    serde_json::from_str(&devices_json).map_err(to_rusqlite_err)?;
                let raw_id: Option<i64> = row.get(4)?;
                Ok(DeviceListRecord {
                    user: row.get(0)?,
                    devices,
                    timestamp: row.get(2)?,
                    phash: row.get(3)?,
                    raw_id: raw_id.map(|r| r as u32),
                })
            },
        );

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn delete_devices(&self, user: &str) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "DELETE FROM device_registry WHERE user_id = ?1 AND device_id = ?2",
            params![user, self.device_id],
        ))
    }

    async fn get_tc_token(&self, jid: &str) -> wacore::store::error::Result<Option<TcTokenEntry>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT token, token_timestamp, sender_timestamp FROM tc_tokens
             WHERE jid = ?1 AND device_id = ?2",
            params![jid, self.device_id],
            |row| {
                Ok(TcTokenEntry {
                    token: row.get(0)?,
                    token_timestamp: row.get(1)?,
                    sender_timestamp: row.get(2)?,
                })
            },
        );

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn put_tc_token(
        &self,
        jid: &str,
        entry: &TcTokenEntry,
    ) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp();

        to_store_err!(execute: conn.execute(
            "INSERT OR REPLACE INTO tc_tokens
             (jid, token, token_timestamp, sender_timestamp, device_id, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                jid,
                entry.token,
                entry.token_timestamp,
                entry.sender_timestamp,
                self.device_id,
                now,
            ],
        ))
    }

    async fn delete_tc_token(&self, jid: &str) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        to_store_err!(execute: conn.execute(
            "DELETE FROM tc_tokens WHERE jid = ?1 AND device_id = ?2",
            params![jid, self.device_id],
        ))
    }

    async fn get_all_tc_token_jids(&self) -> wacore::store::error::Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt =
            to_store_err!(conn.prepare("SELECT jid FROM tc_tokens WHERE device_id = ?1"))?;

        let rows = to_store_err!(
            stmt.query_map(params![self.device_id], |row| { row.get::<_, String>(0) })
        )?;

        let mut result = Vec::new();
        for row in rows {
            result.push(to_store_err!(row)?);
        }

        Ok(result)
    }

    async fn delete_expired_tc_tokens(
        &self,
        cutoff_timestamp: i64,
    ) -> wacore::store::error::Result<u32> {
        let conn = self.conn.lock();
        let deleted = conn
            .execute(
                "DELETE FROM tc_tokens WHERE token_timestamp < ?1 AND device_id = ?2",
                params![cutoff_timestamp, self.device_id],
            )
            .map_err(|e| {
                wacore::store::error::StoreError::Database(
                    Box::new(e) as Box<dyn std::error::Error + Send + Sync>
                )
            })?;

        let deleted = u32::try_from(deleted).map_err(|_| {
            wacore::store::error::StoreError::Validation(format!(
                "Affected row count overflowed u32: {deleted}"
            ))
        })?;

        Ok(deleted)
    }

    async fn store_sent_message(
        &self,
        chat_jid: &str,
        message_id: &str,
        payload: &[u8],
    ) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp();
        to_store_err!(execute: conn.execute(
            "INSERT OR REPLACE INTO sent_messages
             (chat_jid, message_id, payload, device_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![chat_jid, message_id, payload, self.device_id, now],
        ))
    }

    async fn take_sent_message(
        &self,
        chat_jid: &str,
        message_id: &str,
    ) -> wacore::store::error::Result<Option<Vec<u8>>> {
        let mut conn = self.conn.lock();
        let tx = to_store_err!(conn.transaction())?;

        let payload: Option<Vec<u8>> = match tx.query_row(
            "SELECT payload FROM sent_messages
             WHERE chat_jid = ?1 AND message_id = ?2 AND device_id = ?3",
            params![chat_jid, message_id, self.device_id],
            |row| row.get::<_, Vec<u8>>(0),
        ) {
            Ok(p) => Some(p),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                return Err(wacore::store::error::StoreError::Database(Box::new(e)));
            }
        };

        if payload.is_some() {
            to_store_err!(execute: tx.execute(
                "DELETE FROM sent_messages
                 WHERE chat_jid = ?1 AND message_id = ?2 AND device_id = ?3",
                params![chat_jid, message_id, self.device_id],
            ))?;
        }

        to_store_err!(tx.commit())?;
        Ok(payload)
    }

    async fn delete_expired_sent_messages(
        &self,
        cutoff_timestamp: i64,
    ) -> wacore::store::error::Result<u32> {
        let conn = self.conn.lock();
        let deleted = conn
            .execute(
                "DELETE FROM sent_messages WHERE created_at < ?1 AND device_id = ?2",
                params![cutoff_timestamp, self.device_id],
            )
            .map_err(|e| {
                wacore::store::error::StoreError::Database(
                    Box::new(e) as Box<dyn std::error::Error + Send + Sync>
                )
            })?;
        u32::try_from(deleted).map_err(|_| {
            wacore::store::error::StoreError::Validation(format!(
                "Affected row count overflowed u32: {deleted}"
            ))
        })
    }
}

#[async_trait]
impl DeviceStoreTrait for RusqliteStore {
    async fn save(&self, device: &CoreDevice) -> wacore::store::error::Result<()> {
        let conn = self.conn.lock();

        let noise_key = {
            let mut bytes = Vec::new();
            let priv_key = device.noise_key.private_key.serialize();
            bytes.extend_from_slice(priv_key.as_slice());
            bytes.extend_from_slice(device.noise_key.public_key.public_key_bytes());
            bytes
        };

        let identity_key = {
            let mut bytes = Vec::new();
            let priv_key = device.identity_key.private_key.serialize();
            bytes.extend_from_slice(priv_key.as_slice());
            bytes.extend_from_slice(device.identity_key.public_key.public_key_bytes());
            bytes
        };

        let signed_pre_key = {
            let mut bytes = Vec::new();
            let priv_key = device.signed_pre_key.private_key.serialize();
            bytes.extend_from_slice(priv_key.as_slice());
            bytes.extend_from_slice(device.signed_pre_key.public_key.public_key_bytes());
            bytes
        };

        let account = device.account.as_ref().map(|a| a.encode_to_vec());

        let server_cert_chain_blob = device
            .server_cert_chain
            .as_ref()
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|e| wacore::store::error::StoreError::Serialization(Box::new(e)))?;

        to_store_err!(execute: conn.execute(
            "INSERT OR REPLACE INTO device (
                id, lid, pn, registration_id, noise_key, identity_key,
                signed_pre_key, signed_pre_key_id, signed_pre_key_signature,
                adv_secret_key, account, push_name, app_version_primary,
                app_version_secondary, app_version_tertiary, app_version_last_fetched_ms,
                edge_routing_info, props_hash,
                next_pre_key_id, server_has_prekeys, nct_salt,
                server_cert_chain, login_counter
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                ?19, ?20, ?21, ?22, ?23
            )",
            params![
                self.device_id,
                device.lid.as_ref().map(|j| j.to_string()),
                device.pn.as_ref().map(|j| j.to_string()),
                device.registration_id,
                noise_key,
                identity_key,
                signed_pre_key,
                device.signed_pre_key_id,
                device.signed_pre_key_signature.to_vec(),
                device.adv_secret_key.to_vec(),
                account,
                &device.push_name,
                device.app_version_primary,
                device.app_version_secondary,
                device.app_version_tertiary,
                device.app_version_last_fetched_ms,
                device.edge_routing_info.clone(),
                device.props_hash.clone(),
                device.next_pre_key_id,
                device.server_has_prekeys as i64,
                device.nct_salt.clone(),
                server_cert_chain_blob,
                device.login_counter,
            ],
        ))
    }

    async fn load(&self) -> wacore::store::error::Result<Option<CoreDevice>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT * FROM device WHERE id = ?1",
            params![self.device_id],
            |row| {
                fn to_rusqlite_err<E: std::error::Error + Send + Sync + 'static>(
                    e: E,
                ) -> rusqlite::Error {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                }

                let noise_key_bytes: Vec<u8> = row.get("noise_key")?;
                let identity_key_bytes: Vec<u8> = row.get("identity_key")?;
                let signed_pre_key_bytes: Vec<u8> = row.get("signed_pre_key")?;

                if noise_key_bytes.len() != 64
                    || identity_key_bytes.len() != 64
                    || signed_pre_key_bytes.len() != 64
                {
                    return Err(rusqlite::Error::InvalidParameterName("key_pair".into()));
                }

                use wacore::libsignal::protocol::{KeyPair, PrivateKey, PublicKey};

                let noise_key = KeyPair::new(
                    PublicKey::from_djb_public_key_bytes(&noise_key_bytes[32..64])
                        .map_err(to_rusqlite_err)?,
                    PrivateKey::deserialize(&noise_key_bytes[0..32]).map_err(to_rusqlite_err)?,
                );

                let identity_key = KeyPair::new(
                    PublicKey::from_djb_public_key_bytes(&identity_key_bytes[32..64])
                        .map_err(to_rusqlite_err)?,
                    PrivateKey::deserialize(&identity_key_bytes[0..32]).map_err(to_rusqlite_err)?,
                );

                let signed_pre_key = KeyPair::new(
                    PublicKey::from_djb_public_key_bytes(&signed_pre_key_bytes[32..64])
                        .map_err(to_rusqlite_err)?,
                    PrivateKey::deserialize(&signed_pre_key_bytes[0..32])
                        .map_err(to_rusqlite_err)?,
                );

                let lid_str: Option<String> = row.get("lid")?;
                let pn_str: Option<String> = row.get("pn")?;
                let signature_bytes: Vec<u8> = row.get("signed_pre_key_signature")?;
                let adv_secret_bytes: Vec<u8> = row.get("adv_secret_key")?;
                let account_bytes: Option<Vec<u8>> = row.get("account")?;

                let mut signature = [0u8; 64];
                let mut adv_secret = [0u8; 32];
                signature.copy_from_slice(&signature_bytes);
                adv_secret.copy_from_slice(&adv_secret_bytes);

                let account = if let Some(bytes) = account_bytes {
                    Some(
                        waproto::whatsapp::AdvSignedDeviceIdentity::decode(&*bytes)
                            .map_err(to_rusqlite_err)?,
                    )
                } else {
                    None
                };

                let server_cert_chain: Option<wacore::store::device::CachedServerCertChain> = {
                    let bytes: Option<Vec<u8>> = row.get("server_cert_chain")?;
                    match bytes {
                        Some(b) => Some(serde_json::from_slice(&b).map_err(to_rusqlite_err)?),
                        None => None,
                    }
                };
                let server_has_prekeys_int: i64 = row.get("server_has_prekeys")?;

                Ok(CoreDevice {
                    lid: lid_str.and_then(|s| s.parse().ok()),
                    pn: pn_str.and_then(|s| s.parse().ok()),
                    registration_id: row.get("registration_id")?,
                    noise_key,
                    identity_key,
                    signed_pre_key,
                    signed_pre_key_id: row.get("signed_pre_key_id")?,
                    signed_pre_key_signature: signature,
                    adv_secret_key: adv_secret,
                    account,
                    push_name: row.get("push_name")?,
                    app_version_primary: row.get("app_version_primary")?,
                    app_version_secondary: row.get("app_version_secondary")?,
                    app_version_tertiary: row.get("app_version_tertiary")?,
                    app_version_last_fetched_ms: row.get("app_version_last_fetched_ms")?,
                    edge_routing_info: row.get("edge_routing_info")?,
                    props_hash: row.get("props_hash")?,
                    next_pre_key_id: row.get("next_pre_key_id")?,
                    server_has_prekeys: server_has_prekeys_int != 0,
                    nct_salt: row.get("nct_salt")?,
                    server_cert_chain,
                    login_counter: row.get("login_counter")?,
                    ..Default::default()
                })
            },
        );

        match result {
            Ok(device) => Ok(Some(device)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(wacore::store::error::StoreError::Database(Box::new(e))),
        }
    }

    async fn exists(&self) -> wacore::store::error::Result<bool> {
        let conn = self.conn.lock();
        let count: i64 =
            to_store_err!(
                conn.query_row(DEVICE_EXISTS_SQL, params![self.device_id], |row| row.get(0),)
            )?;

        Ok(count > 0)
    }

    async fn create(&self) -> wacore::store::error::Result<i32> {
        Ok(self.device_id)
    }

    async fn snapshot_db(
        &self,
        name: &str,
        extra_content: Option<&[u8]>,
    ) -> wacore::store::error::Result<()> {
        let snapshot_path = format!("{}.snapshot.{}", self.db_path, name);

        to_store_err!(std::fs::copy(&self.db_path, &snapshot_path))?;

        if let Some(content) = extra_content {
            let content_path = format!("{}.extra", snapshot_path);
            to_store_err!(std::fs::write(&content_path, content))?;
        }

        Ok(format!("{}.snapshot.{}", self.db_path, name)).map(|_| ())
    }
}
