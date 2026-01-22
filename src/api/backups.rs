use super::client::api_call;
use serde_json::Value;

/// Backup profile view structure
#[derive(Clone, Debug)]
pub struct BackupProfileView {
    pub instance_id: String,
    pub status: String,
    pub schedule_frequency: Option<String>,
    pub monthly_price: Option<f64>,
    pub max_files: Option<i32>,
    // Created timestamp from API - preserved for future sorting/filtering
    // pub created_at: Option<i64>,
}

/// Load backup profiles from the API
pub async fn load_backups(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
) -> Vec<BackupProfileView> {
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/backups", None, None).await;
    let mut backups = Vec::new();
    
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(data) = payload.get("data").and_then(|d| d.as_object()) {
            if let Some(arr) = data.get("backups").or_else(|| data.get("data")).and_then(|b| b.as_array()) {
                for item in arr {
                    if let Some(obj) = item.as_object() {
                        backups.push(BackupProfileView {
                            instance_id: obj.get("instanceId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            status: obj.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            schedule_frequency: obj.get("scheduleFrequency").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            monthly_price: obj.get("monthlyPrice").and_then(|v| v.as_f64()),
                            max_files: obj.get("maxFiles").and_then(|v| v.as_i64()).map(|i| i as i32),
                            // created_at: obj.get("createdAt").and_then(|v| v.as_i64()),
                        });
                    }
                }
            }
        }
    }
    
    backups
}

// Get backup profile for instance - preserved for future use
// pub async fn get_backup_profile(
//     client: &reqwest::Client,
//     api_base_url: &str,
//     api_token: &str,
//     instance_id: &str,
// ) -> Value {
//     let endpoint = format!("/v1/backups/{}", instance_id);
//     api_call(client, api_base_url, api_token, "GET", &endpoint, None, None).await
// }

/// Create backup profile
pub async fn create_backup_profile(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    instance_id: &str,
    schedule_frequency: &str,
    period_id: i32,
    schedule_week_days: Option<Vec<String>>,
) -> Value {
    let mut payload = serde_json::json!({
        "instanceId": instance_id,
        "scheduleFrequency": schedule_frequency,
        "periodId": period_id
    });
    
    if let Some(days) = schedule_week_days {
        payload["scheduleWeekDays"] = Value::Array(days.into_iter().map(Value::String).collect());
    }
    
    api_call(client, api_base_url, api_token, "POST", "/v1/backups", Some(payload), None).await
}

// Update backup profile - preserved for future use
// pub async fn update_backup_profile(
//     client: &reqwest::Client,
//     api_base_url: &str,
//     api_token: &str,
//     instance_id: &str,
//     schedule_frequency: &str,
//     period_id: i32,
//     schedule_week_days: Option<Vec<String>>,
// ) -> Value {
//     let mut payload = serde_json::json!({
//         "instanceId": instance_id,
//         "scheduleFrequency": schedule_frequency,
//         "periodId": period_id
//     });
//     
//     if let Some(days) = schedule_week_days {
//         payload["scheduleWeekDays"] = Value::Array(days.into_iter().map(Value::String).collect());
//     }
//     
//     api_call(client, api_base_url, api_token, "PUT", "/v1/backups", Some(payload), None).await
// }

// Delete backup profile - preserved for future use
// pub async fn delete_backup_profile(
//     client: &reqwest::Client,
//     api_base_url: &str,
//     api_token: &str,
//     instance_id: &str,
// ) -> Value {
//     let endpoint = format!("/v1/backups/{}", instance_id);
//     api_call(client, api_base_url, api_token, "DELETE", &endpoint, None, None).await
// }
