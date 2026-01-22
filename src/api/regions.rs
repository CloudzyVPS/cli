use std::collections::HashMap;
use crate::models::{Region, region::RegionConfig};
use super::client::api_call;

/// Parse RegionConfig from JSON object
fn parse_region_config(config_value: Option<&serde_json::Value>) -> RegionConfig {
    if let Some(config_obj) = config_value.and_then(|v| v.as_object()) {
        RegionConfig {
            support_ipv6: config_obj.get("supportIpv6").and_then(|v| v.as_bool()).unwrap_or(false),
            support_regular_cpu: config_obj.get("supportRegularCpu").and_then(|v| v.as_bool()).unwrap_or(false),
            support_high_frequency_cpu: config_obj.get("supportHighFrequencyCpu").and_then(|v| v.as_bool()).unwrap_or(false),
            support_monitoring: config_obj.get("supportMonitoring").and_then(|v| v.as_bool()).unwrap_or(false),
            support_gpu: config_obj.get("supportGpu").and_then(|v| v.as_bool()).unwrap_or(false),
            support_custom_plan: config_obj.get("supportCustomPlan").and_then(|v| v.as_bool()).unwrap_or(false),
            ram_threshold_in_gb: config_obj.get("ramThresholdInGB").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            ip_threshold: config_obj.get("ipThreshold").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            disk_threshold_in_gb: config_obj.get("diskThresholdInGB").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            support_ddos_ipv4: config_obj.get("supportDdosIpv4").and_then(|v| v.as_bool()),
            ddos_ipv4_threshold: config_obj.get("ddosIpv4Threshold").and_then(|v| v.as_i64()).map(|i| i as i32),
        }
    } else {
        // Default config if not present
        RegionConfig {
            support_ipv6: false,
            support_regular_cpu: true,
            support_high_frequency_cpu: false,
            support_monitoring: false,
            support_gpu: false,
            support_custom_plan: false,
            ram_threshold_in_gb: 0,
            ip_threshold: 0,
            disk_threshold_in_gb: 0,
            support_ddos_ipv4: None,
            ddos_ipv4_threshold: None,
        }
    }
}

/// Load all available regions from the API.
/// Returns a vector of regions and a hashmap for quick lookup by ID.
pub async fn load_regions(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
) -> (Vec<Region>, HashMap<String, Region>) {
    let params = vec![("per_page".to_string(), "1000".to_string())];
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/regions", None, Some(params)).await;
    let mut regions = Vec::new();
    let mut map = HashMap::new();
    
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(arr) = payload.get("data").and_then(|d| d.as_array()) {
            for r in arr {
                if let Some(obj) = r.as_object() {
                    let id = obj
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let name = obj
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&id)
                        .to_string();

                    let region = Region {
                        id: id.clone(),
                        name,
                        abbr: obj.get("abbr").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        image: obj.get("image").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        is_active: obj.get("isActive").and_then(|v| v.as_bool()).unwrap_or(false),
                        is_out_of_stock: obj.get("isOutOfStock").and_then(|v| v.as_bool()).unwrap_or(false),
                        overall_activeness: obj.get("overallActiveness").and_then(|v| v.as_bool()).unwrap_or(false),
                        ddos_activeness: obj.get("ddosActiveness").and_then(|v| v.as_bool()),
                        is_premium: obj.get("isPremium").and_then(|v| v.as_bool()).unwrap_or(false),
                        is_hidden: obj.get("isHidden").and_then(|v| v.as_bool()).unwrap_or(false),
                        has_offset_price: obj.get("hasOffsetPrice").and_then(|v| v.as_bool()).unwrap_or(false),
                        max_discount_percent: obj.get("maxDiscountPercent").and_then(|v| v.as_i64()).map(|i| i as i32),
                        position: obj.get("position").cloned().unwrap_or(serde_json::json!({})),
                        config: parse_region_config(obj.get("config")),
                    };
                    regions.push(region.clone());
                    map.insert(id, region);
                }
            }
        }
    }
    (regions, map)
}
