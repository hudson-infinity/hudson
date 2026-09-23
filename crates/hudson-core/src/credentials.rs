//! Trusted-host credential issuance and revocation. These operations must never
//! be exposed as agent tools or unauthenticated HTTP endpoints.
use crate::{
    models::{now, Actor},
    storage::Store,
    Error, Result,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiTokenMetadata {
    pub id: Uuid,
    pub actor: Actor,
    pub label: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub revoked_at: Option<u64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ApiTokenRecord {
    pub metadata: ApiTokenMetadata,
    digest: [u8; 32],
}

/// The bearer value exists only at issuance. It is deliberately not serializable.
pub struct IssuedApiToken {
    pub metadata: ApiTokenMetadata,
    bearer: String,
}
impl IssuedApiToken {
    pub fn bearer(&self) -> &str {
        &self.bearer
    }
}
impl std::fmt::Debug for IssuedApiToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuedApiToken")
            .field("metadata", &self.metadata)
            .field("bearer", &"[redacted]")
            .finish()
    }
}

fn digest(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}
fn identifier(value: &str) -> Result<Uuid> {
    let (id, secret) = value
        .strip_prefix("hudson_")
        .and_then(|v| v.split_once('.'))
        .ok_or(Error::Denied)?;
    let hexadecimal = |s: &str| {
        s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if id.len() != 32 || secret.len() != 64 || !hexadecimal(id) || !hexadecimal(secret) {
        return Err(Error::Denied);
    }
    Uuid::parse_str(id).map_err(|_| Error::Denied)
}
fn valid_label(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
impl Store {
    /// Installation authority must authorize issuance before calling this method.
    pub fn issue_api_token(
        &self,
        actor: Actor,
        label: String,
        expires_at: u64,
    ) -> Result<IssuedApiToken> {
        let created_at = now();
        if !valid_label(&actor.workspace_id)
            || !valid_label(&actor.id)
            || !valid_label(&label)
            || expires_at <= created_at
        {
            return Err(Error::Invalid(
                "token requires a valid actor, label and future expiry".into(),
            ));
        }
        let mut secret = [0u8; 32];
        getrandom::fill(&mut secret)
            .map_err(|_| Error::Conflict("credential entropy unavailable".into()))?;
        let id = Uuid::new_v4();
        let hex: String = secret.iter().map(|b| format!("{b:02x}")).collect();
        let bearer = format!("hudson_{}.{}", id.simple(), hex);
        let metadata = ApiTokenMetadata {
            id,
            actor,
            label,
            created_at,
            expires_at,
            revoked_at: None,
        };
        let record = ApiTokenRecord {
            metadata: metadata.clone(),
            digest: digest(&bearer),
        };
        self.transact(|data| {
            if data.api_tokens.contains_key(&id) {
                return Err(Error::Conflict("credential identity collision".into()));
            }
            data.api_tokens.insert(id, record);
            Ok(())
        })?;
        Ok(IssuedApiToken { metadata, bearer })
    }

    /// Resolve authority from the stored credential, never from request actor fields.
    pub fn authenticate_api_token(&self, bearer: &str) -> Result<Actor> {
        let id = identifier(bearer)?;
        let supplied = digest(bearer);
        self.read(|data| {
            let record = data.api_tokens.get(&id).ok_or(Error::Denied)?;
            if !bool::from(record.digest.ct_eq(&supplied))
                || record.metadata.revoked_at.is_some()
                || record.metadata.expires_at <= now()
            {
                return Err(Error::Denied);
            }
            Ok(record.metadata.actor.clone())
        })
    }

    /// Installation authority supplies the expected owner to prevent cross-scope changes.
    pub fn revoke_api_token(&self, actor: &Actor, id: Uuid) -> Result<()> {
        self.transact(|data| {
            let record = data
                .api_tokens
                .get_mut(&id)
                .filter(|record| {
                    record.metadata.actor.id == actor.id
                        && record.metadata.actor.workspace_id == actor.workspace_id
                })
                .ok_or(Error::NotFound)?;
            record.metadata.revoked_at.get_or_insert_with(now);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokens_preserve_authority_and_rotation_without_storing_the_bearer() {
        let store = Store::default();
        let actor = Actor {
            workspace_id: "project".into(),
            id: "service".into(),
        };
        let first = store
            .issue_api_token(actor.clone(), "first".into(), now() + 60_000)
            .unwrap();
        let second = store
            .issue_api_token(actor.clone(), "rotation".into(), now() + 60_000)
            .unwrap();
        assert_ne!(first.bearer(), second.bearer());
        assert_eq!(
            store
                .authenticate_api_token(first.bearer())
                .unwrap()
                .workspace_id,
            actor.workspace_id
        );
        let persisted = store.read(|data| Ok(serde_json::to_string(data)?)).unwrap();
        assert!(!persisted.contains(first.bearer()));
        assert!(!persisted.contains(first.bearer().split_once('.').unwrap().1));
        assert!(!format!("{first:?}").contains(first.bearer()));
        let other = Actor {
            workspace_id: "other".into(),
            ..actor.clone()
        };
        assert!(store.revoke_api_token(&other, first.metadata.id).is_err());
        store.revoke_api_token(&actor, first.metadata.id).unwrap();
        store.revoke_api_token(&actor, first.metadata.id).unwrap();
        assert!(store.authenticate_api_token(first.bearer()).is_err());
        assert_eq!(
            store.authenticate_api_token(second.bearer()).unwrap().id,
            actor.id
        );
        store
            .transact(|data| {
                data.api_tokens
                    .get_mut(&second.metadata.id)
                    .unwrap()
                    .metadata
                    .expires_at = now();
                Ok(())
            })
            .unwrap();
        assert!(store.authenticate_api_token(second.bearer()).is_err());
    }

    #[test]
    fn forged_and_malformed_credentials_do_not_authenticate() {
        let store = Store::default();
        let actor = Actor {
            workspace_id: "project".into(),
            id: "service".into(),
        };
        let token = store
            .issue_api_token(actor, "test".into(), now() + 60_000)
            .unwrap();
        let forged = format!("hudson_{}.{}", token.metadata.id.simple(), "0".repeat(64));
        for value in [
            "",
            "Bearer token",
            "hudson_",
            &forged,
            &token.bearer().to_uppercase(),
            &format!("{}extra", token.bearer()),
        ] {
            assert!(matches!(
                store.authenticate_api_token(value),
                Err(Error::Denied)
            ));
        }
    }
}
