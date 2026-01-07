use crate::models::{ProductView, ProductEntry};
use crate::utils::value_to_short_string;
use super::client::api_call;

/// Load products/plans for a specific region.
/// Returns a list of product offerings with specifications and pricing.
pub async fn load_products(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    region_id: &str,
) -> Vec<ProductView> {
    let params = vec![
        ("regionId".into(), region_id.to_string()),
        ("per_page".into(), "1000".into()),
    ];
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/products", None, Some(params)).await;
    let mut out = vec![];
    
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(arr) = payload.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(obj) = item.as_object() {
                    let id = obj
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();

                    let plan = obj.get("plan").and_then(|v| v.as_object());
                    let price_items = obj.get("priceItems").and_then(|v| v.as_array());

                    let name = id.clone();
                    let display_name = name.clone();
                    let description = obj
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tags = obj
                        .get("tags")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(); 

                    let mut spec_entries = Vec::new();
                    let mut cpu = None;
                    let mut ram = None;
                    let mut storage = None;
                    let mut bandwidth = None;

                    if let Some(p) = plan {
                        if let Some(spec) = p.get("specification").and_then(|v| v.as_object()) {
                            if let Some(c) = spec.get("cpu") {
                                let val = value_to_short_string(c);
                                cpu = Some(format!("{} vCPU", val));
                                spec_entries.push(ProductEntry { term: "CPU".into(), value: format!("{} vCPU", val) });
                            }
                            if let Some(r) = spec.get("ram") {
                                let val = value_to_short_string(r);
                                ram = Some(format!("{} GB", val));
                                spec_entries.push(ProductEntry { term: "RAM".into(), value: format!("{} GB", val) });
                            }
                            if let Some(s) = spec.get("storage") {
                                let val = value_to_short_string(s);
                                storage = Some(format!("{} GB", val));
                                spec_entries.push(ProductEntry { term: "Storage".into(), value: format!("{} GB", val) });
                            }
                            if let Some(b) = spec.get("bandwidthInTB") {
                                let val = value_to_short_string(b);
                                bandwidth = Some(format!("{} TB", val));
                                spec_entries.push(ProductEntry { term: "Bandwidth".into(), value: format!("{} TB", val) });
                            }
                        }
                    }

                    let mut price_entries = Vec::new();
                    if let Some(items) = price_items {
                        for item in items {
                            if let Some(monthly) = item.get("monthlyPrice") {
                                price_entries.push(ProductEntry { term: "Monthly".into(), value: format!("${}", value_to_short_string(monthly)) });
                            }
                        }
                    }

                    out.push(ProductView {
                        id,
                        name,
                        display_name,
                        description,
                        tags,
                        spec_entries,
                        price_entries,
                        cpu,
                        ram,
                        storage,
                        bandwidth,
                    });
                }
            }
        }
    }
    out
}
