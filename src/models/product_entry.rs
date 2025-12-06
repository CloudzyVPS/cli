use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ProductEntry {
    pub term: String,
    pub value: String,
}
