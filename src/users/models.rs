use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use rand::RngCore;
use hex::encode as hex_encode;

use crate::config::{DEFAULT_PBKDF2_ITERATIONS, DEFAULT_OWNER_USERNAME, DEFAULT_OWNER_PASSWORD, DEFAULT_OWNER_ROLE};

#[derive(Clone, Serialize, Deserialize)]
pub struct UserRecord {
    pub password: String,
    pub role: String,
    pub assigned_instances: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurrentUser {
    pub username: String,
    pub role: String,
}

pub fn generate_password_hash(password: &str) -> String {
    let mut salt_bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut salt_bytes);
    let salt = hex_encode(salt_bytes);
    let mut dk = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt.as_bytes(), DEFAULT_PBKDF2_ITERATIONS, &mut dk);
    let hash_hex = hex_encode(dk);
    format!("pbkdf2:sha256:{}${}${}", DEFAULT_PBKDF2_ITERATIONS, salt, hash_hex)
}

pub fn verify_password(stored: &str, candidate: &str) -> bool {
    if let Some(rest) = stored.strip_prefix("pbkdf2:sha256:") {
        if let Some((iter_s, salt_hash)) = rest.split_once('$') {
            if let Some((salt, expected_hash)) = salt_hash.split_once('$') {
                if let Ok(iter) = iter_s.parse::<u32>() {
                    let mut dk = [0u8; 32];
                    pbkdf2_hmac::<Sha256>(candidate.as_bytes(), salt.as_bytes(), iter, &mut dk);
                    let computed = hex_encode(dk);
                    return computed == expected_hash;
                }
            }
        }
    }
    false
}

pub fn random_session_id() -> String {
    let mut b = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex_encode(b)
}

pub fn load_users_from_file() -> Arc<Mutex<HashMap<String, UserRecord>>> {
    let path = std::path::Path::new("users.json");
    let mut map: HashMap<String, UserRecord> = HashMap::new();
    
    if path.exists() {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(obj) = json_val.as_object() {
                    for (k, v) in obj.iter() {
                        if let Some(pw) = v.get("password").and_then(|x| x.as_str()) {
                            let role = v
                                .get("role")
                                .and_then(|x| x.as_str())
                                .unwrap_or("admin")
                                .to_string();
                            let assigned_instances = v
                                .get("assigned_instances")
                                .and_then(|a| a.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                        .collect()
                                })
                                .unwrap_or_else(|| vec![]);
                            map.insert(
                                k.to_lowercase(),
                                UserRecord {
                                    password: pw.to_string(),
                                    role,
                                    assigned_instances,
                                },
                            );
                        }
                    }
                }
            }
        }
    } else {
        let salt = {
            let mut b = [0u8; 12];
            rand::rngs::OsRng.fill_bytes(&mut b);
            hex_encode(b)
        };
        let mut dk = [0u8; 32];
        pbkdf2_hmac::<Sha256>(DEFAULT_OWNER_PASSWORD.as_bytes(), salt.as_bytes(), DEFAULT_PBKDF2_ITERATIONS, &mut dk);
        let hash_hex = hex_encode(dk);
        let full = format!("pbkdf2:sha256:{}${}${}", DEFAULT_PBKDF2_ITERATIONS, salt, hash_hex);
        map.insert(
            DEFAULT_OWNER_USERNAME.into(),
            UserRecord {
                password: full,
                role: DEFAULT_OWNER_ROLE.into(),
                assigned_instances: vec![],
            },
        );
        let mut serialized: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
        for (u, rec) in map.iter() {
            serialized.insert(u.clone(), serde_json::json!({"password": rec.password, "role": rec.role, "assigned_instances": rec.assigned_instances }));
        }
        let _ = std::fs::write(
            path,
            serde_json::to_string_pretty(&serde_json::Value::Object(serialized)).unwrap(),
        );
    }
    
    Arc::new(Mutex::new(map))
}

pub fn persist_users_file(users_arc: &Arc<Mutex<HashMap<String, UserRecord>>>) -> Result<(), std::io::Error> {
    let users = users_arc.lock().unwrap();
    let mut serialized: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for (u, rec) in users.iter() {
        serialized.insert(u.clone(), serde_json::json!({"password": rec.password, "role": rec.role, "assigned_instances": rec.assigned_instances }));
    }
    std::fs::write("users.json", serde_json::to_string_pretty(&serde_json::Value::Object(serialized))?)
}
