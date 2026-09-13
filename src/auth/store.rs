//! Stored sign-ins, one per profile, in `credentials.json`.
//!
//! The file holds refresh tokens, so it is created owner-read/write only
//! (0600) and rewritten atomically through a sibling temp file. This is the
//! same trade-off `gh`, `gcloud` and `doctl` make: a desktop keychain is not
//! available on the servers and containers zy is typically run on.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use super::AuthError;

/// One profile's sign-in.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredProfile {
    /// Platform base URL the tokens were issued by.
    pub url: String,
    pub access_token: String,
    pub refresh_token: String,
    /// Unix seconds after which the access token must not be used.
    pub expires_at: u64,
    #[serde(default)]
    pub scope: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CredentialsFile {
    #[serde(default)]
    profiles: BTreeMap<String, StoredProfile>,
}

/// File-backed credential store.
#[derive(Debug, Clone)]
pub struct FileStore {
    dir: PathBuf,
}

impl FileStore {
    pub fn new(dir: PathBuf) -> Self {
        FileStore { dir }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join("credentials.json")
    }

    fn read(&self) -> Result<CredentialsFile, AuthError> {
        match fs::read(self.path()) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
                AuthError::Store(format!("{} is not valid JSON: {e}", self.path().display()))
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(CredentialsFile::default()),
            Err(e) => Err(AuthError::Store(format!(
                "cannot read {}: {e}",
                self.path().display()
            ))),
        }
    }

    fn write(&self, file: &CredentialsFile) -> Result<(), AuthError> {
        let err = |e: std::io::Error| {
            AuthError::Store(format!("cannot write {}: {e}", self.path().display()))
        };
        fs::create_dir_all(&self.dir).map_err(err)?;
        let tmp = self.dir.join(".credentials.json.tmp");
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp).map_err(err)?;
        let body = serde_json::to_vec_pretty(file).expect("credentials serialize");
        f.write_all(&body).map_err(err)?;
        f.sync_all().map_err(err)?;
        drop(f);
        fs::rename(&tmp, self.path()).map_err(err)
    }

    pub fn load(&self, profile: &str) -> Result<Option<StoredProfile>, AuthError> {
        Ok(self.read()?.profiles.get(profile).cloned())
    }

    pub fn save(&self, profile: &str, value: &StoredProfile) -> Result<(), AuthError> {
        let mut file = self.read()?;
        file.profiles.insert(profile.to_string(), value.clone());
        self.write(&file)
    }

    /// Returns whether the profile existed.
    pub fn remove(&self, profile: &str) -> Result<bool, AuthError> {
        let mut file = self.read()?;
        let existed = file.profiles.remove(profile).is_some();
        if existed {
            self.write(&file)?;
        }
        Ok(existed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> StoredProfile {
        StoredProfile {
            url: "https://dash.example.test".into(),
            access_token: "czat_a".into(),
            refresh_token: "czrt_r".into(),
            expires_at: 42,
            scope: "openid".into(),
        }
    }

    #[test]
    fn round_trip_and_remove() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileStore::new(dir.path().join("nested"));
        assert_eq!(store.load("default").unwrap(), None);
        store.save("default", &sample()).unwrap();
        store
            .save(
                "staging",
                &StoredProfile {
                    url: "https://staging.test".into(),
                    ..sample()
                },
            )
            .unwrap();
        assert_eq!(store.load("default").unwrap(), Some(sample()));
        assert!(store.remove("default").unwrap());
        assert!(!store.remove("default").unwrap());
        assert_eq!(
            store.load("staging").unwrap().unwrap().url,
            "https://staging.test"
        );
    }

    #[cfg(unix)]
    #[test]
    fn file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let store = FileStore::new(dir.path().to_path_buf());
        store.save("default", &sample()).unwrap();
        let mode = fs::metadata(store.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
