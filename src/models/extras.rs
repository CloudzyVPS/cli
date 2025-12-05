use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Extras {
    pub extra_disk: String,
    pub extra_bandwidth: String,
}
