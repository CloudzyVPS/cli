use serde::Deserialize;

#[derive(Deserialize)]
pub struct AddTrafficForm {
    pub traffic_amount: String,
}
