use serde::Deserialize;

#[derive(Deserialize)]
pub struct ChangeOsForm {
    pub os_id: String,
}
