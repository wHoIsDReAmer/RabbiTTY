use keyring::Entry;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const SERVICE: &str = "rabbitty-ssh";

fn entry_key(host: &str, user: &str) -> String {
    if user.is_empty() {
        host.to_string()
    } else {
        format!("{user}@{host}")
    }
}

fn entry_for(host: &str, user: &str) -> Option<Entry> {
    Entry::new(SERVICE, &entry_key(host, user)).ok()
}

// Process-wide cache so repeated `get_password` calls only hit the OS
// keychain once per (host, user) — on macOS this avoids re-triggering the
// "allow access" ACL dialog every connection.
fn cache() -> &'static Mutex<HashMap<String, String>> {
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get_password(host: &str, user: &str) -> Option<String> {
    let key = entry_key(host, user);
    {
        let guard = cache().lock().expect("keychain cache poisoned");
        if let Some(v) = guard.get(&key) {
            return Some(v.clone());
        }
    }
    let pw = entry_for(host, user)?.get_password().ok()?;
    cache()
        .lock()
        .expect("keychain cache poisoned")
        .insert(key, pw.clone());
    Some(pw)
}

pub fn set_password(host: &str, user: &str, password: &str) {
    if let Some(entry) = entry_for(host, user) {
        let _ = entry.set_password(password);
    }
    cache()
        .lock()
        .expect("keychain cache poisoned")
        .insert(entry_key(host, user), password.to_string());
}

pub fn delete_password(host: &str, user: &str) {
    if let Some(entry) = entry_for(host, user) {
        let _ = entry.delete_credential();
    }
    cache()
        .lock()
        .expect("keychain cache poisoned")
        .remove(&entry_key(host, user));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_key_format() {
        // With user
        let entry = entry_for("host.com", "admin").unwrap();
        // Just verify it doesn't panic — the key is "admin@host.com"
        drop(entry);

        // Without user
        let entry = entry_for("bare.host", "").unwrap();
        drop(entry);
    }

    #[test]
    fn roundtrip_set_get_delete() {
        let host = "rabbitty-test-host.local";
        let user = "testuser";
        let Some(entry) = entry_for(host, user) else {
            return;
        };

        // Some environments do not expose a writable native keychain.
        if entry.set_password("test_pw_12345").is_err() {
            return;
        }

        let Ok(pw) = entry.get_password() else {
            let _ = entry.delete_credential();
            return;
        };
        assert_eq!(pw, "test_pw_12345");

        let _ = entry.delete_credential();
        assert!(entry.get_password().is_err());
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let pw = get_password("nonexistent-rabbitty-host.test", "nobody");
        assert!(pw.is_none());
    }

    #[test]
    fn cache_serves_subsequent_reads() {
        let host = "rabbitty-cache-test.local";
        let user = "cacheuser";

        // Seed the cache via set_password without touching the keychain
        // (the keyring call may fail silently on CI; cache write still
        // happens).
        set_password(host, user, "cached_pw");
        assert_eq!(get_password(host, user).as_deref(), Some("cached_pw"));

        // Cleanup so other test runs aren't polluted.
        delete_password(host, user);
        let _ = entry_for(host, user).map(|e| e.delete_credential());
    }
}
