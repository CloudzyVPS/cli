use crate::models::{ProductView, ProductEntry, product_view::{Plan, PlanSpecification, PriceItem}};
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

                    let region_id = obj.get("regionId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let plan_id = obj.get("planId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let is_active = obj.get("isActive").and_then(|v| v.as_bool()).unwrap_or(false);
                    let network_max_rate = obj.get("networkMaxRate").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let network_max_rate95 = obj.get("networkMaxRate95").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let discount_percent = obj.get("discountPercent").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let remaining_actual_stock = obj.get("remainingActualStock").and_then(|v| v.as_i64()).map(|i| i as i32);
                    let remaining_preorder_capacity = obj.get("remainingPreorderCapacity").and_then(|v| v.as_i64()).map(|i| i as i32);
                    let overall_activeness = obj.get("overallActiveness").and_then(|v| v.as_bool()).unwrap_or(false);
                    let ddos_activeness = obj.get("ddosActiveness").and_then(|v| v.as_bool());

                    // Parse plan
                    let plan_obj = obj.get("plan").and_then(|v| v.as_object());
                    let plan = if let Some(p) = plan_obj {
                        let spec_obj = p.get("specification").and_then(|v| v.as_object());
                        let specification = if let Some(spec) = spec_obj {
                            PlanSpecification {
                                cpu: spec.get("cpu").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                ram: spec.get("ram").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                ram_in_mb: spec.get("ramInMB").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                storage: spec.get("storage").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                bandwidth_in_tb: spec.get("bandwidthInTB").and_then(|v| v.as_f64()).unwrap_or(0.0),
                            }
                        } else {
                            PlanSpecification {
                                cpu: 0.0,
                                ram: 0.0,
                                ram_in_mb: 0.0,
                                storage: 0.0,
                                bandwidth_in_tb: 0.0,
                            }
                        };

                        Plan {
                            id: p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            plan_type: p.get("type").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            gpu_name: p.get("gpuName").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            gpu_quantity: p.get("gpuQuantity").and_then(|v| v.as_i64()).map(|i| i as i32),
                            specification,
                            is_active: p.get("isActive").and_then(|v| v.as_bool()).unwrap_or(false),
                        }
                    } else {
                        Plan {
                            id: "".to_string(),
                            plan_type: None,
                            gpu_name: None,
                            gpu_quantity: None,
                            specification: PlanSpecification {
                                cpu: 0.0,
                                ram: 0.0,
                                ram_in_mb: 0.0,
                                storage: 0.0,
                                bandwidth_in_tb: 0.0,
                            },
                            is_active: false,
                        }
                    };

                    // Parse price items
                    let price_items_arr = obj.get("priceItems").and_then(|v| v.as_array());
                    let mut price_items = Vec::new();
                    if let Some(items) = price_items_arr {
                        for item in items {
                            if let Some(pi_obj) = item.as_object() {
                                price_items.push(PriceItem {
                                    id: pi_obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    name: pi_obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    hourly_price: pi_obj.get("hourlyPrice").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                    monthly_price: pi_obj.get("monthlyPrice").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                    hourly_price_without_discount: pi_obj.get("hourlyPriceWithoutDiscount").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                    monthly_price_without_discount: pi_obj.get("monthlyPriceWithoutDiscount").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                    discount_percent: pi_obj.get("discountPercent").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                                });
                            }
                        }
                    }

                    // Build display fields for templates
                    let description = "".to_string(); // Not in OpenAPI schema
                    let tags = "".to_string(); // Not in OpenAPI schema
                    
                    let mut spec_entries = Vec::new();

                    let spec = &plan.specification;
                    if spec.cpu > 0.0 {
                        let val = spec.cpu.to_string();
                        spec_entries.push(ProductEntry { term: "CPU".into(), value: format!("{} vCPU", val) });
                    }
                    if spec.ram > 0.0 {
                        let val = spec.ram.to_string();
                        spec_entries.push(ProductEntry { term: "RAM".into(), value: format!("{} GB", val) });
                    }
                    if spec.storage > 0.0 {
                        let val = spec.storage.to_string();
                        spec_entries.push(ProductEntry { term: "Storage".into(), value: format!("{} GB", val) });
                    }
                    if spec.bandwidth_in_tb > 0.0 {
                        let val = spec.bandwidth_in_tb.to_string();
                        spec_entries.push(ProductEntry { term: "Bandwidth".into(), value: format!("{} TB", val) });
                    }

                    let mut price_entries = Vec::new();
                    for pi in &price_items {
                        if pi.monthly_price > 0.0 {
                            price_entries.push(ProductEntry { 
                                term: "Monthly".into(), 
                                value: format!("${:.2}", pi.monthly_price) 
                            });
                        }
                    }

                    out.push(ProductView {
                        id,
                        region_id,
                        plan_id,
                        is_active,
                        network_max_rate,
                        network_max_rate95,
                        discount_percent,
                        remaining_actual_stock,
                        remaining_preorder_capacity,
                        plan,
                        overall_activeness,
                        ddos_activeness,
                        price_items,
                        description,
                        tags,
                        spec_entries,
                        price_entries,
                    });
                }
            }
        }
    }
    out
}
