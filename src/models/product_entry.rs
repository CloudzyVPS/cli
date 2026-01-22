use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProductEntry {
    pub term: String,
    pub value: String,
}
