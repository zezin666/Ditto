use std::collections::HashMap;

use parking_lot::Mutex;
use thiserror::Error;

use crate::protocol::HelperMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenBinding {
    pub core_pid: u32,
    pub target_pid: u32,
    pub target_window: isize,
    pub clip_id: i64,
    pub mode: HelperMode,
    pub user_sid: String,
}

#[derive(Debug, Clone)]
pub struct IssuedToken {
    pub request_id: u64,
    pub token: [u8; 32],
    pub expires_at_ms: u64,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TokenError {
    #[error("elevation token is missing or already used")]
    MissingOrUsed,
    #[error("elevation token expired")]
    Expired,
    #[error("elevation token does not match")]
    Mismatch,
}

#[derive(Default)]
pub struct TokenStore {
    pending: Mutex<HashMap<u64, PendingToken>>,
}

struct PendingToken {
    token: [u8; 32],
    expires_at_ms: u64,
    binding: TokenBinding,
}

impl TokenStore {
    pub fn issue(
        &self,
        request_id: u64,
        token: [u8; 32],
        binding: TokenBinding,
        now_ms: u64,
        ttl_ms: u64,
    ) -> IssuedToken {
        let expires_at_ms = now_ms.saturating_add(ttl_ms);
        self.pending.lock().insert(
            request_id,
            PendingToken {
                token,
                expires_at_ms,
                binding,
            },
        );
        IssuedToken {
            request_id,
            token,
            expires_at_ms,
        }
    }

    pub fn authorize(
        &self,
        request_id: u64,
        token: [u8; 32],
        binding: &TokenBinding,
        now_ms: u64,
    ) -> Result<(), TokenError> {
        let pending = self
            .pending
            .lock()
            .remove(&request_id)
            .ok_or(TokenError::MissingOrUsed)?;
        if now_ms >= pending.expires_at_ms {
            return Err(TokenError::Expired);
        }
        if !constant_time_eq(&pending.token, &token) || pending.binding != *binding {
            return Err(TokenError::Mismatch);
        }
        Ok(())
    }

    pub fn pending_len(&self) -> usize {
        self.pending.lock().len()
    }
}

fn constant_time_eq(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> TokenBinding {
        TokenBinding {
            core_pid: 10,
            target_pid: 20,
            target_window: 0x1234,
            clip_id: 7,
            mode: HelperMode::Plain,
            user_sid: "S-1-5-21-test".to_string(),
        }
    }

    #[test]
    fn token_is_bound_and_single_use() {
        let store = TokenStore::default();
        let issued = store.issue(1, [7; 32], binding(), 100, 10_000);
        store
            .authorize(issued.request_id, issued.token, &binding(), 101)
            .expect("first use");
        assert_eq!(
            store
                .authorize(issued.request_id, issued.token, &binding(), 102)
                .expect_err("replay"),
            TokenError::MissingOrUsed
        );
    }

    #[test]
    fn expired_token_is_rejected_and_removed() {
        let store = TokenStore::default();
        let issued = store.issue(2, [3; 32], binding(), 100, 50);
        assert_eq!(
            store
                .authorize(issued.request_id, issued.token, &binding(), 150)
                .expect_err("expired"),
            TokenError::Expired
        );
        assert_eq!(store.pending_len(), 0);
    }

    #[test]
    fn wrong_token_binding_or_request_is_rejected() {
        let store = TokenStore::default();
        let issued = store.issue(3, [4; 32], binding(), 100, 1000);
        assert_eq!(
            store
                .authorize(issued.request_id, [5; 32], &binding(), 101)
                .expect_err("token"),
            TokenError::Mismatch
        );

        let issued = store.issue(4, [6; 32], binding(), 100, 1000);
        let mut changed = binding();
        changed.target_pid = 99;
        assert_eq!(
            store
                .authorize(issued.request_id, issued.token, &changed, 101)
                .expect_err("binding"),
            TokenError::Mismatch
        );

        let issued = store.issue(5, [8; 32], binding(), 100, 1000);
        assert_eq!(
            store
                .authorize(issued.request_id + 1, issued.token, &binding(), 101)
                .expect_err("request"),
            TokenError::MissingOrUsed
        );
    }
}
