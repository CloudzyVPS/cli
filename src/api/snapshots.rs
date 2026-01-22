use super::client::api_call;
use serde_json::Value;

/// Snapshot view structure for display
#[derive(Clone, Debug)]
pub struct SnapshotView {
    pub id: String,
    pub name: String,
    pub size: Option<i64>,
    pub status: String,
    pub created_at: Option<i64>,
    pub last_restored_at: Option<i64>,
    pub is_instance_deleted: bool,
    pub instance_id: String,
    pub region_id: Option<String>,
}

/// Paginated result structure for snapshots
#[derive(Clone, Debug)]
pub struct PaginatedSnapshots {
    pub snapshots: Vec<SnapshotView>,
    pub total_count: usize,
    pub current_page: usize,
    pub total_pages: usize,
    pub per_page: usize,
}

/// Load snapshots from the API with optional filtering
pub async fn load_snapshots(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    instance_id: Option<String>,
    page: usize,
    per_page: usize,
) -> PaginatedSnapshots {
    let mut params = Vec::new();
    
    if let Some(iid) = instance_id {
        params.push(("instanceId".to_string(), iid));
    }
    
    // Add pagination (page 1 is the first page)
    if page >= 1 {
        params.push(("page".to_string(), page.to_string()));
        params.push(("per_page".to_string(), per_page.to_string()));
    }
    
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/snapshots", None, Some(params)).await;
    
    let mut snapshots = Vec::new();
    let mut total_count = 0;
    
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(data) = payload.get("data").and_then(|d| d.as_object()) {
            if let Some(arr) = data.get("snapshots").and_then(|s| s.as_array()) {
                for item in arr {
                    if let Some(obj) = item.as_object() {
                        snapshots.push(SnapshotView {
                            id: obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            name: obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            size: obj.get("size").and_then(|v| v.as_i64()),
                            status: obj.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            created_at: obj.get("createdAt").and_then(|v| v.as_i64()),
                            last_restored_at: obj.get("lastRestoredAt").and_then(|v| v.as_i64()),
                            is_instance_deleted: obj.get("isInstanceDeleted").and_then(|v| v.as_bool()).unwrap_or(false),
                            instance_id: obj.get("instanceId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            region_id: obj.get("regionId").and_then(|v| v.as_str()).map(|s| s.to_string()),
                        });
                    }
                }
            }
            
            total_count = data.get("total").and_then(|t| t.as_u64()).unwrap_or(snapshots.len() as u64) as usize;
        }
    }
    
    let actual_total = if total_count > 0 { total_count } else { snapshots.len() };
    let total_pages = if per_page > 0 { actual_total.div_ceil(per_page) } else { 1 };
    let current_page = if page >= 1 { page } else { 1 };
    
    PaginatedSnapshots {
        snapshots,
        total_count: actual_total,
        current_page,
        total_pages,
        per_page,
    }
}

/// Create a snapshot of an instance
pub async fn create_snapshot(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    instance_id: &str,
) -> Value {
    let payload = serde_json::json!({"instanceId": instance_id});
    api_call(client, api_base_url, api_token, "POST", "/v1/snapshots", Some(payload), None).await
}

/// Get snapshot details
pub async fn get_snapshot(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    snapshot_id: &str,
) -> Value {
    let endpoint = format!("/v1/snapshots/{}", snapshot_id);
    api_call(client, api_base_url, api_token, "GET", &endpoint, None, None).await
}

/// Delete a snapshot
pub async fn delete_snapshot(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    snapshot_id: &str,
) -> Value {
    let endpoint = format!("/v1/snapshots/{}", snapshot_id);
    api_call(client, api_base_url, api_token, "DELETE", &endpoint, None, None).await
}

/// Restore an instance from a snapshot
pub async fn restore_snapshot(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    snapshot_id: &str,
) -> Value {
    let endpoint = format!("/v1/snapshots/{}/restore", snapshot_id);
    api_call(client, api_base_url, api_token, "POST", &endpoint, None, None).await
}
