//! Durable, native-only registration preparation. Uses its own SQLite file,
//! never the application's v15 database. No HTTP, token, IPC or automatic work.
//!
//! A provisioning claim never expires: OS keyring cannot fence a delayed writer.
//! Recovery only loads existing keys; missing keys require explicit intervention.
use super::*;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::{path::Path, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryError {
    Storage,
    Conflict,
    NotFound,
    ProvisioningUncertain,
    Corrupt,
    Identity(IdentityError),
}
impl From<sqlx::Error> for RegistryError {
    fn from(_: sqlx::Error) -> Self {
        Self::Storage
    }
}
impl From<IdentityError> for RegistryError {
    fn from(e: IdentityError) -> Self {
        Self::Identity(e)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationState {
    Reserved,
    Provisioning,
    Ready,
}

/// Public metadata only. Construction and mutation stay inside the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preparation {
    scope: CommunityScope,
    profile: RegistrationProfile,
    client_start_id: String,
    key_reference: String,
    state: PreparationState,
    revision: i64,
    public: Option<PublicIdentity>,
}
impl Preparation {
    pub fn state(&self) -> PreparationState {
        self.state
    }
    pub fn revision(&self) -> i64 {
        self.revision
    }
    pub fn key_reference(&self) -> &str {
        &self.key_reference
    }
    pub fn client_start_id(&self) -> &str {
        &self.client_start_id
    }
    pub fn public_identity(&self) -> Option<&PublicIdentity> {
        self.public.as_ref()
    }
}

#[derive(Clone)]
pub struct RegistrationRegistry {
    pool: SqlitePool,
}
impl RegistrationRegistry {
    /// Dedicated database ONLY. Existing unrelated databases are refused, not migrated.
    /// The host must protect the directory and later wire deletion/backup lifecycle.
    pub async fn open(path: &Path) -> Result<Self, RegistryError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let mut tx = pool.begin().await?;
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&mut *tx)
            .await?;
        let app: i64 = sqlx::query_scalar("PRAGMA application_id")
            .fetch_one(&mut *tx)
            .await?;
        let tables: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        )
        .fetch_one(&mut *tx)
        .await?;
        if !((version == 0 && app == 0 && tables == 0)
            || (version == 1 && app == 1095254851 && tables == 1))
        {
            return Err(RegistryError::Corrupt);
        }
        sqlx::query("CREATE TABLE IF NOT EXISTS community_preparations (scope_binding TEXT PRIMARY KEY, profile_id TEXT NOT NULL, api_root TEXT NOT NULL, origin TEXT NOT NULL, instance TEXT NOT NULL, display_name TEXT NOT NULL, about TEXT NOT NULL, client_start_id TEXT NOT NULL UNIQUE, key_reference TEXT NOT NULL UNIQUE, state TEXT NOT NULL CHECK(state IN ('reserved','provisioning','ready')), revision INTEGER NOT NULL CHECK(revision>0), owner TEXT, identity_public_key TEXT, operational_public_key TEXT, CHECK((state='reserved' AND owner IS NULL AND identity_public_key IS NULL AND operational_public_key IS NULL) OR (state='provisioning' AND owner IS NOT NULL AND identity_public_key IS NULL AND operational_public_key IS NULL) OR (state='ready' AND owner IS NULL AND identity_public_key IS NOT NULL AND operational_public_key IS NOT NULL)))").execute(&mut *tx).await?;
        sqlx::query("PRAGMA application_id=1095254851")
            .execute(&mut *tx)
            .await?;
        sqlx::query("PRAGMA user_version=1")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Self { pool })
    }
    pub async fn close(self) {
        self.pool.close().await;
    }

    /// Idempotent for the exact scope, public profile and original start ID.
    /// Persists the key reference before any key generation. No invite is accepted.
    pub async fn prepare(
        &self,
        scope: &CommunityScope,
        profile: &RegistrationProfile,
        client_start_id: &str,
    ) -> Result<Preparation, RegistryError> {
        if !(16..=128).contains(&client_start_id.len())
            || !client_start_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
        {
            return Err(RegistryError::Identity(IdentityError::Invalid));
        }
        let reference = CommunityKeyStore::reserve_reference()?;
        // Unique start IDs also prevent accidental reuse across different scopes.
        sqlx::query("INSERT INTO community_preparations(scope_binding,profile_id,api_root,origin,instance,display_name,about,client_start_id,key_reference,state,revision) VALUES(?,?,?,?,?,?,?,?,?,'reserved',1) ON CONFLICT DO NOTHING")
            .bind(scope.binding()).bind(&scope.profile_id).bind(&scope.api_root)
            .bind(&scope.origin).bind(&scope.instance).bind(&profile.display_name).bind(&profile.about)
            .bind(client_start_id).bind(reference).execute(&self.pool).await?;
        let record = self.get(scope).await.map_err(|e| {
            if e == RegistryError::NotFound {
                RegistryError::Conflict
            } else {
                e
            }
        })?;
        if record.profile != *profile || record.client_start_id != client_start_id {
            return Err(RegistryError::Conflict);
        }
        Ok(record)
    }

    pub async fn get(&self, scope: &CommunityScope) -> Result<Preparation, RegistryError> {
        let row = sqlx::query("SELECT * FROM community_preparations WHERE scope_binding=?")
            .bind(scope.binding())
            .fetch_optional(&self.pool)
            .await?
            .ok_or(RegistryError::NotFound)?;
        let text = |key: &str| {
            row.try_get::<String, _>(key)
                .map_err(|_| RegistryError::Corrupt)
        };
        let saved_scope = CommunityScope::new(
            &text("profile_id")?,
            &text("api_root")?,
            &text("origin")?,
            &text("instance")?,
        )
        .map_err(|_| RegistryError::Corrupt)?;
        if saved_scope != *scope || text("scope_binding")? != scope.binding() {
            return Err(RegistryError::Corrupt);
        }
        let profile = RegistrationProfile::new(&text("display_name")?, &text("about")?)
            .map_err(|_| RegistryError::Corrupt)?;
        if profile.display_name != text("display_name")? {
            return Err(RegistryError::Corrupt);
        }
        let client_start_id = text("client_start_id")?;
        if !(16..=128).contains(&client_start_id.len())
            || !client_start_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
        {
            return Err(RegistryError::Corrupt);
        }
        let key_reference = text("key_reference")?;
        super::vault::reference_valid(&key_reference).map_err(|_| RegistryError::Corrupt)?;
        let state = match text("state")?.as_str() {
            "reserved" => PreparationState::Reserved,
            "provisioning" => PreparationState::Provisioning,
            "ready" => PreparationState::Ready,
            _ => return Err(RegistryError::Corrupt),
        };
        let revision: i64 = row
            .try_get("revision")
            .map_err(|_| RegistryError::Corrupt)?;
        if revision <= 0 {
            return Err(RegistryError::Corrupt);
        }
        let public = if state == PreparationState::Ready {
            let a = text("identity_public_key")?;
            let b = text("operational_public_key")?;
            distinct(&a, &b).map_err(|_| RegistryError::Corrupt)?;
            Some(PublicIdentity {
                key_ref: key_reference.clone(),
                scope_binding: scope.binding(),
                identity_public_key: a,
                operational_public_key: b,
            })
        } else {
            None
        };
        Ok(Preparation {
            scope: saved_scope,
            profile,
            client_start_id,
            key_reference,
            state,
            revision,
            public,
        })
    }

    /// Permanent one-shot ownership. No TTL/takeover: a cancelled spawn_blocking
    /// keyring call may still be running. Another caller can only load, never provision.
    pub async fn provision(
        &self,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
        expected_revision: i64,
    ) -> Result<Preparation, RegistryError> {
        let record = self.get(scope).await?;
        if record.revision != expected_revision {
            return Err(RegistryError::Conflict);
        }
        if record.state != PreparationState::Reserved {
            return Err(RegistryError::ProvisioningUncertain);
        }
        let owner = uuid::Uuid::new_v4().to_string();
        let changed = sqlx::query("UPDATE community_preparations SET state='provisioning',owner=?,revision=revision+1 WHERE scope_binding=? AND state='reserved' AND revision=?")
            .bind(&owner).bind(scope.binding()).bind(expected_revision).execute(&self.pool).await?.rows_affected();
        if changed != 1 {
            return Err(RegistryError::Conflict);
        }
        // Claim is committed before side effects. Even errors leave it in place.
        let public = keys.provision(scope, &record.key_reference).await?;
        self.finish(scope, expected_revision + 1, Some(&owner), &public)
            .await
    }

    /// Safe after cancellation/restart: only observes existing keys, never creates,
    /// replaces or removes them. Missing/unavailable keys leave state unchanged.
    pub async fn recover(
        &self,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
        expected_revision: i64,
    ) -> Result<Preparation, RegistryError> {
        let record = self.get(scope).await?;
        if record.revision != expected_revision {
            return Err(RegistryError::Conflict);
        }
        if record.state == PreparationState::Reserved {
            return Err(RegistryError::ProvisioningUncertain);
        }
        let public = keys.load(scope, &record.key_reference).await?;
        if record.state == PreparationState::Ready {
            if record.public.as_ref() != Some(&public) {
                return Err(RegistryError::Identity(IdentityError::ScopeMismatch));
            }
            return Ok(record);
        }
        self.finish(scope, expected_revision, None, &public).await
    }

    async fn finish(
        &self,
        scope: &CommunityScope,
        revision: i64,
        owner: Option<&str>,
        public: &PublicIdentity,
    ) -> Result<Preparation, RegistryError> {
        let changed = sqlx::query("UPDATE community_preparations SET state='ready',owner=NULL,identity_public_key=?,operational_public_key=?,revision=revision+1 WHERE scope_binding=? AND revision=? AND state='provisioning' AND key_reference=? AND (? IS NULL OR owner=?)")
            .bind(&public.identity_public_key).bind(&public.operational_public_key)
            .bind(scope.binding()).bind(revision).bind(&public.key_ref).bind(owner).bind(owner)
            .execute(&self.pool).await?.rows_affected();
        if changed != 1 {
            return Err(RegistryError::Conflict);
        }
        self.get(scope).await
    }

    /// Reconstructs from durable original metadata and verifies vault continuity.
    /// Ready means local keys exist; it never means registered/authenticated online.
    pub async fn load_intent(
        &self,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
    ) -> Result<RegistrationIntent, RegistryError> {
        let record = self.get(scope).await?;
        if record.state != PreparationState::Ready {
            return Err(RegistryError::ProvisioningUncertain);
        }
        let actual = keys.load(scope, &record.key_reference).await?;
        if record.public.as_ref() != Some(&actual) {
            return Err(RegistryError::Identity(IdentityError::ScopeMismatch));
        }
        Ok(RegistrationIntent::new(
            scope.clone(),
            actual,
            record.profile,
            &record.client_start_id,
        )?)
    }
}

#[cfg(test)]
mod tests;
