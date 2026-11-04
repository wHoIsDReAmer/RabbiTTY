use keyring::Entry;
use std::collections::{BTreeMap, HashMap};
use std::sync::{LazyLock, Mutex, OnceLock};

const SERVICE: &str = "rabbitty-ssh";
const PLUGIN_SERVICE: &str = "rabbitty-plugin";

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

type Secrets = BTreeMap<String, String>;

static PLUGIN_SECRETS: LazyLock<Mutex<HashMap<String, Secrets>>> = LazyLock::new(Default::default);

fn plugin_entry(plugin: &str) -> Option<Entry> {
    if cfg!(test) {
        return None;
    }
    Entry::new(PLUGIN_SERVICE, plugin).ok()
}

pub fn plugin_secrets(plugin: &str) -> Secrets {
    PLUGIN_SECRETS
        .lock()
        .expect("keychain cache poisoned")
        .entry(plugin.to_string())
        .or_insert_with(|| {
            plugin_entry(plugin)
                .and_then(|entry| entry.get_password().ok())
                .and_then(|stored| toml::from_str(&stored).ok())
                .unwrap_or_default()
        })
        .clone()
}

pub fn set_plugin_secret(plugin: &str, key: &str, value: &str) {
    let mut secrets = plugin_secrets(plugin);
    if value.is_empty() {
        if secrets.remove(key).is_none() {
            return;
        }
    } else {
        secrets.insert(key.to_string(), value.to_string());
    }
    store_plugin_secrets(plugin, secrets);
}

pub fn delete_plugin_secrets(plugin: &str) {
    store_plugin_secrets(plugin, Secrets::new());
}

fn store_plugin_secrets(plugin: &str, secrets: Secrets) {
    if let Some(entry) = plugin_entry(plugin) {
        let _ = match toml::to_string(&secrets) {
            Ok(serialized) if !secrets.is_empty() => entry.set_password(&serialized),
            _ => entry.delete_credential(),
        };
    }
    PLUGIN_SECRETS
        .lock()
        .expect("keychain cache poisoned")
        .insert(plugin.to_string(), secrets);
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
