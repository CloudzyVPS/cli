use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use crate::models::{
    AppState, Step1FormData, Step2FormData,
    CustomPlanFormValues, Region, ProductView, ProductEntry, OsItem,
    SshKeyDisplay, Extras, PlanState,
};
use crate::services::{parse_wizard_base, build_base_query_pairs};
use crate::utils::{build_query_string, parse_urlencoded_body};
use crate::api::{load_regions, load_products, load_os_list, load_applications};
use crate::templates::*;
use crate::handlers::helpers::{
    build_template_globals, absolute_url_from_state,
    ensure_admin_or_owner, TemplateGlobals, OneOrMany, render_template,
    api_call_wrapper, fetch_default_customer_id, load_ssh_keys_api,
};

fn value_to_short_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(arr) => arr
            .iter()
            .map(value_to_short_string)
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(obj) => {
            let mut parts = Vec::new();
            for (key, val) in obj {
                parts.push(format!("{}: {}", key, value_to_short_string(val)));
            }
            parts.join(", ")
        }
        Value::Null => String::new(),
    }
}

async fn load_regions_wrapper(state: &AppState) -> (Vec<Region>, HashMap<String, Region>) {
    load_regions(&state.client, &state.api_base_url, &state.api_token).await
}

async fn load_products_wrapper(state: &AppState, region_id: &str) -> Vec<ProductView> {
    load_products(&state.client, &state.api_base_url, &state.api_token, region_id).await
}

async fn load_os_list_wrapper(state: &AppState) -> Vec<OsItem> {
    load_os_list(&state.client, &state.api_base_url, &state.api_token).await
}

// These functions are used by wizard steps but defined elsewhere in main.rs
// We'll need them imported or moved here
// use crate::{fetch_default_customer_id, load_ssh_keys_api};

// ---------- Wizard Step 1 Template ----------

pub async fn create_step_1(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_admin_or_owner(&state, &jar) {
        return r.into_response();
    }
    let base = parse_wizard_base(&q);
    let (all_regions, _lookup) = load_regions_wrapper(&state).await;
    // Filter to only show active, non-hidden regions
    let regions: Vec<Region> = all_regions.into_iter()
        .filter(|r| r.is_active && !r.is_hidden)
        .collect();
    let mut region_sel = base.region.clone();
    if region_sel.is_empty() && !regions.is_empty() {
        region_sel = regions[0].id.clone();
    }
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    let form_data = Step1FormData {
        region: region_sel,
        instance_class: base.instance_class.clone(),
        plan_type: base.plan_type.clone(),
    };
    render_template(&state, &jar, Step1Template {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            regions: &regions,
            form_data,
        },
    )
}

// ---------- Wizard Step 2 (Hostnames & IP Assignment) ----------

pub async fn create_step_2(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_admin_or_owner(&state, &jar) {
        return r.into_response();
    }
    let mut base = parse_wizard_base(&q);
    if base.region.is_empty() {
        return Redirect::to("/create/step-1").into_response();
    }
    // If hostnames passed as comma separated in textarea update parsing
    if let Some(raw_hosts) = q.get("hostnames") {
        base.hostnames = raw_hosts
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    let back_pairs = build_base_query_pairs(&base);
    let back_q = build_query_string(&back_pairs);
    let back_url = if back_q.is_empty() {
        absolute_url_from_state(&state, "/create/step-1")
    } else {
        absolute_url_from_state(&state, &format!("/create/step-1?{}", back_q))
    };
    let hostnames_text = base.hostnames.join(", ");
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    let form_data = Step2FormData {
        hostnames_text,
        assign_ipv4: base.assign_ipv4,
        assign_ipv6: base.assign_ipv6,
        floating_ip_count: base.floating_ip_count.to_string(),
    };
    render_template(&state, &jar, Step2Template {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            base_state: &base,
            form_data,
            back_url,
            submit_url: absolute_url_from_state(&state, "/create/step-3"),
        },
    )
}

// ---------- Wizard Step 3 (Product selection or custom resources) ----------

pub async fn create_step_3(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_admin_or_owner(&state, &jar) {
        return r.into_response();
    }
    let base = parse_wizard_base(&q);
    if base.hostnames.is_empty() || base.region.is_empty() {
        return Redirect::to("/create/step-1").into_response();
    }
    let back_pairs = build_base_query_pairs(&base);
    let back_q = build_query_string(&back_pairs);
    let back_url = if back_q.is_empty() {
        absolute_url_from_state(&state, "/create/step-2")
    } else {
        absolute_url_from_state(&state, &format!("/create/step-2?{}", back_q))
    };
    // Build the hostnames CSV and prepare ssh key CSV for the template where needed
    let hostnames_csv = base.hostnames.join(",");
    let ssh_key_ids_csv = base.ssh_key_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");

    if base.plan_type == "fixed" {
        let products = load_products_wrapper(&state, &base.region).await;
        let selected_product_id = q.get("product_id").cloned().unwrap_or_default();
        let TemplateGlobals {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
        } = build_template_globals(&state, &jar);
        // Use the outer variables defined above
        return render_template(&state, &jar, Step3FixedTemplate {
                current_user,
                api_hostname,
                base_url,
                flash_messages,
                has_flash_messages,
                base_state: &base,
                products: &products,
                has_products: !products.is_empty(),
                selected_product_id,
                region_name: base.region.clone(),
                floating_ip_count: base.floating_ip_count.to_string(),
                back_url,
                submit_url: absolute_url_from_state(&state, "/create/step-4"),
                restart_url: absolute_url_from_state(&state, "/create/step-1"),
                ssh_key_ids_csv: ssh_key_ids_csv.clone(),
                hostnames_csv: hostnames_csv.clone(),
            },
        );
    }
    let cpu = q.get("cpu").cloned().unwrap_or_else(|| "2".into());
    let ram = q.get("ramInGB").cloned().unwrap_or_else(|| "4".into());
    let disk = q.get("diskInGB").cloned().unwrap_or_else(|| "50".into());
    let bw = q
        .get("bandwidthInTB")
        .cloned()
        .unwrap_or_else(|| "1".into());
    
    let hostnames_csv = base.hostnames.join(",");
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    let form_values = CustomPlanFormValues {
        cpu,
        ram_in_gb: ram,
        disk_in_gb: disk,
        bandwidth_in_tb: bw,
    };
    render_template(&state, &jar, Step3CustomTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            base_state: &base,
            region_name: base.region.clone(),
            floating_ip_count: base.floating_ip_count.to_string(),
            back_url,
            submit_url: absolute_url_from_state(&state, "/create/step-5"),
            requirements: Vec::new(),
            minimum_ram: 1,
            minimum_disk: 1,
            form_values,
            ssh_key_ids_csv: ssh_key_ids_csv.clone(),
            hostnames_csv: hostnames_csv,
        },
    )
}

// ---------- Wizard Step 4 (Extras for fixed plans) ----------

pub async fn create_step_4(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_admin_or_owner(&state, &jar) {
        return r.into_response();
    }
    let base = parse_wizard_base(&q);
    if base.hostnames.is_empty() || base.region.is_empty() {
        return Redirect::to("/create/step-1").into_response();
    }
    let ssh_key_ids_csv = base.ssh_key_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
    let hostnames_csv = base.hostnames.join(",");
    let back_pairs = build_base_query_pairs(&base);
    let back_q = build_query_string(&back_pairs);
    let back_url = if back_q.is_empty() {
        absolute_url_from_state(&state, "/create/step-3")
    } else {
        absolute_url_from_state(&state, &format!("/create/step-3?{}", back_q))
    };
    if base.plan_type != "fixed" {
        let next_pairs = build_base_query_pairs(&base);
        let next_q = build_query_string(&next_pairs);
        let next_url = if next_q.is_empty() {
            "/create/step-5".to_string()
        } else {
            format!("/create/step-5?{}", next_q)
        };
        return Redirect::to(&next_url).into_response();
    }
    let product_id = q.get("product_id").cloned().unwrap_or_default();
    if product_id.is_empty() {
        return Redirect::to("/create/step-3").into_response();
    }
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    let extras = Extras {
        extra_disk: q.get("extra_disk").cloned().unwrap_or_else(|| "0".into()),
        extra_bandwidth: q
            .get("extra_bandwidth")
            .cloned()
            .unwrap_or_else(|| "0".into()),
    };
    render_template(&state, &jar, Step4Template {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            base_state: &base,
            floating_ip_count: base.floating_ip_count.to_string(),
            product_id,
            ssh_key_ids_csv: ssh_key_ids_csv,
            hostnames_csv: hostnames_csv,
            extras,
            back_url,
            submit_url: absolute_url_from_state(&state, "/create/step-5"),
        },
    )
}

// ---------- Wizard Step 5 (OS selection) ----------

pub async fn create_step_5(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_admin_or_owner(&state, &jar) {
        return r.into_response();
    }
    let base = parse_wizard_base(&q);
    if base.hostnames.is_empty() || base.region.is_empty() {
        return Redirect::to("/create/step-1").into_response();
    }
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    let product_id = q.get("product_id").cloned().unwrap_or_default();
    if base.plan_type == "fixed" && product_id.is_empty() {
        return Redirect::to("/create/step-3").into_response();
    }
    let extra_disk = q.get("extra_disk").cloned().unwrap_or_else(|| "0".into());
    let extra_bandwidth = q
        .get("extra_bandwidth")
        .cloned()
        .unwrap_or_else(|| "0".into());
    let custom_plan = CustomPlanFormValues {
        cpu: q.get("cpu").cloned().unwrap_or_else(|| "2".into()),
        ram_in_gb: q.get("ramInGB").cloned().unwrap_or_else(|| "4".into()),
        disk_in_gb: q.get("diskInGB").cloned().unwrap_or_else(|| "50".into()),
        bandwidth_in_tb: q
            .get("bandwidthInTB")
            .cloned()
            .unwrap_or_else(|| "1".into()),
    };
    let os_list = load_os_list_wrapper(&state).await;
    let applications = load_applications(&state.client, &state.api_base_url, &state.api_token).await;
    let mut selected_os_id = base.os_id.clone();
    if selected_os_id.is_empty() {
        selected_os_id = q.get("os_id").cloned().unwrap_or_default();
    }
    if selected_os_id.is_empty() {
        selected_os_id = os_list
            .iter()
            .find(|o| o.is_default)
            .map(|o| o.id.clone())
            .or_else(|| os_list.first().map(|o| o.id.clone()))
            .unwrap_or_default();
    }
    let selected_app_id = base.app_id.clone().or_else(|| q.get("app_id").cloned()).unwrap_or_default();
    let mut back_pairs = build_base_query_pairs(&base);
    let back_target = if base.plan_type == "fixed" {
        if !product_id.is_empty() {
            back_pairs.push(("product_id".into(), product_id.clone()));
        }
        back_pairs.push(("extra_disk".into(), extra_disk.clone()));
        back_pairs.push(("extra_bandwidth".into(), extra_bandwidth.clone()));
        "/create/step-4"
    } else {
        back_pairs.push(("cpu".into(), custom_plan.cpu.clone()));
        back_pairs.push(("ramInGB".into(), custom_plan.ram_in_gb.clone()));
        back_pairs.push(("diskInGB".into(), custom_plan.disk_in_gb.clone()));
        back_pairs.push(("bandwidthInTB".into(), custom_plan.bandwidth_in_tb.clone()));
        "/create/step-3"
    };
    let back_q = build_query_string(&back_pairs);
    let back_url = if back_q.is_empty() {
        absolute_url_from_state(&state, back_target)
    } else {
        absolute_url_from_state(&state, &format!("{}?{}", back_target, back_q))
    };
    let hostnames_csv = base.hostnames.join(",");
    let ssh_key_ids_csv = base.ssh_key_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
    render_template(&state, &jar, Step5Template {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            base_state: &base,
            os_list: &os_list,
            selected_os_id,
            applications: &applications,
            selected_app_id,
            product_id,
            extra_disk,
            extra_bandwidth,
            custom_plan,
            floating_ip_count: base.floating_ip_count.to_string(),
            back_url,
            submit_url: absolute_url_from_state(&state, "/create/step-6"),
            hostnames_csv: hostnames_csv,
            ssh_key_ids_csv: ssh_key_ids_csv,
        },
    )
}

// ---------- Wizard Step 6 (SSH key selection) ----------

pub async fn create_step_6(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_admin_or_owner(&state, &jar) {
        return r.into_response();
    }
    let base = parse_wizard_base(&q);
    if base.hostnames.is_empty() || base.region.is_empty() {
        return Redirect::to("/create/step-1").into_response();
    }
    if base.os_id.is_empty() {
        return Redirect::to("/create/step-5").into_response();
    }
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    let product_id = q.get("product_id").cloned().unwrap_or_default();
    if base.plan_type == "fixed" && product_id.is_empty() {
        return Redirect::to("/create/step-3").into_response();
    }
    let extra_disk = q.get("extra_disk").cloned().unwrap_or_else(|| "0".into());
    let extra_bandwidth = q
        .get("extra_bandwidth")
        .cloned()
        .unwrap_or_else(|| "0".into());
    let custom_plan = CustomPlanFormValues {
        cpu: q.get("cpu").cloned().unwrap_or_else(|| "2".into()),
        ram_in_gb: q.get("ramInGB").cloned().unwrap_or_else(|| "4".into()),
        disk_in_gb: q.get("diskInGB").cloned().unwrap_or_else(|| "50".into()),
        bandwidth_in_tb: q
            .get("bandwidthInTB")
            .cloned()
            .unwrap_or_else(|| "1".into()),
    };
    let mut back_pairs = build_base_query_pairs(&base);
    let back_target = if base.plan_type == "fixed" {
        if !product_id.is_empty() {
            back_pairs.push(("product_id".into(), product_id.clone()));
        }
        back_pairs.push(("extra_disk".into(), extra_disk.clone()));
        back_pairs.push(("extra_bandwidth".into(), extra_bandwidth.clone()));
        "/create/step-5"
    } else {
        back_pairs.push(("cpu".into(), custom_plan.cpu.clone()));
        back_pairs.push(("ramInGB".into(), custom_plan.ram_in_gb.clone()));
        back_pairs.push(("diskInGB".into(), custom_plan.disk_in_gb.clone()));
        back_pairs.push(("bandwidthInTB".into(), custom_plan.bandwidth_in_tb.clone()));
        "/create/step-5"
    };
    let back_q = build_query_string(&back_pairs);
    let back_url = if back_q.is_empty() {
        absolute_url_from_state(&state, back_target)
    } else {
        absolute_url_from_state(&state, &format!("{}?{}", back_target, back_q))
    };
    let customer_id = fetch_default_customer_id(&state).await;
    let ssh_keys = load_ssh_keys_api(&state, customer_id).await;
    let selected_ids: HashSet<String> =
        base.ssh_key_ids.iter().map(|id| id.to_string()).collect();
    let selectable: Vec<SshKeyDisplay> = ssh_keys
        .into_iter()
        .map(|key| {
            let is_selected = selected_ids.contains(&key.id);
            SshKeyDisplay {
                id: key.id,
                name: key.name,
                selected: is_selected,
            }
        })
        .collect();
    let hostnames_csv = base.hostnames.join(",");
    let _ssh_key_ids_csv = base.ssh_key_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
    render_template(&state, &jar, Step6Template {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            base_state: &base,
            floating_ip_count: base.floating_ip_count.to_string(),
            ssh_keys: &selectable,
            product_id,
            extra_disk,
            extra_bandwidth,
            custom_plan,
            back_url,
            submit_url: absolute_url_from_state(&state, "/create/step-7"),
            manage_keys_url: absolute_url_from_state(&state, "/ssh-keys"),
            hostnames_csv,
        },
    )
}

// ---------- Wizard Step 7 (Review & Create) ----------

async fn create_step_7_core(
    state: AppState,
    jar: CookieJar,
    method: axum::http::Method,
    query: HashMap<String, String>,
    form: HashMap<String, String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_admin_or_owner(&state, &jar) {
        return r.into_response();
    }
    let source = if method == axum::http::Method::POST {
        &form
    } else {
        &query
    };
    let base = parse_wizard_base(source);
    if base.hostnames.is_empty() || base.region.is_empty() {
        return Redirect::to("/create/step-1").into_response();
    }
    if base.os_id.is_empty() {
        return Redirect::to("/create/step-5").into_response();
    }
    let mut plan_state = PlanState::default();
    if base.plan_type == "fixed" {
        plan_state.product_id = source.get("product_id").cloned().unwrap_or_default();
        if plan_state.product_id.is_empty() {
            return Redirect::to("/create/step-3").into_response();
        }
        plan_state.extra_disk = source
            .get("extra_disk")
            .cloned()
            .unwrap_or_else(|| "0".into());
        plan_state.extra_bandwidth = source
            .get("extra_bandwidth")
            .cloned()
            .unwrap_or_else(|| "0".into());
    } else {
        plan_state.cpu = source.get("cpu").cloned().unwrap_or_else(|| "2".into());
        plan_state.ram_in_gb = source.get("ramInGB").cloned().unwrap_or_else(|| "4".into());
        plan_state.disk_in_gb = source
            .get("diskInGB")
            .cloned()
            .unwrap_or_else(|| "50".into());
        plan_state.bandwidth_in_tb = source
            .get("bandwidthInTB")
            .cloned()
            .unwrap_or_else(|| "1".into());
    }
    if method == axum::http::Method::POST {
        let mut payload = serde_json::json!({
            "hostnames": base.hostnames,
            "region": base.region,
            "class": base.instance_class,
            "assignIpv4": base.assign_ipv4,
            "assignIpv6": base.assign_ipv6,
            "osId": base.os_id,
        });
        if let Some(ref app_id) = base.app_id {
            if !app_id.is_empty() {
                payload["appId"] = Value::from(app_id.clone());
            }
        }
        if base.floating_ip_count > 0 {
            payload["floatingIPCount"] = Value::from(base.floating_ip_count);
        }
        if !base.ssh_key_ids.is_empty() {
            payload["sshKeyIds"] = Value::from(base.ssh_key_ids.clone());
        }
        if base.plan_type == "fixed" {
            payload["productId"] = Value::from(plan_state.product_id.clone());
            let mut extras = serde_json::Map::new();
            if let Some(d) = plan_state
                .extra_disk
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|v| *v > 0)
            {
                extras.insert("diskInGB".into(), Value::from(d));
            }
            if let Some(b) = plan_state
                .extra_bandwidth
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|v| *v > 0)
            {
                extras.insert("bandwidthInTB".into(), Value::from(b));
            }
            if !extras.is_empty() {
                payload["extraResource"] = Value::Object(extras);
            }
        } else {
            let mut extras = serde_json::Map::new();
            if let Some(cpu) = plan_state.cpu.trim().parse::<i64>().ok() {
                extras.insert("cpu".into(), Value::from(cpu));
            }
            if let Some(ram) = plan_state.ram_in_gb.trim().parse::<i64>().ok() {
                extras.insert("ramInGB".into(), Value::from(ram));
            }
            if let Some(disk) = plan_state.disk_in_gb.trim().parse::<i64>().ok() {
                extras.insert("diskInGB".into(), Value::from(disk));
            }
            if let Some(bw) = plan_state.bandwidth_in_tb.trim().parse::<i64>().ok() {
                extras.insert("bandwidthInTB".into(), Value::from(bw));
            }
            if !extras.is_empty() {
                payload["extraResource"] = Value::Object(extras);
            }
        }
        let resp = api_call_wrapper(&state, "POST", "/v1/instances", Some(payload.clone()), None).await;
        
        // Debug logging for creation failure
        tracing::info!(?payload, ?resp, "Create Instance Attempt");

        if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY")
            || resp.get("code").and_then(|c| c.as_str()) == Some("CREATED")
        {
            return Redirect::to("/instances").into_response();
        } else {
            // Build error / result page
            let mut errors: Vec<String> = Vec::new();
            if let Some(detail) = resp.get("detail").and_then(|d| d.as_str()) {
                if !detail.trim().is_empty() {
                    errors.push(detail.to_string());
                }
            }
            // Some APIs return 'errors' as array or map
            if let Some(arr) = resp.get("errors").and_then(|e| e.as_array()) {
                for entry in arr {
                    if let Some(s) = entry.as_str() {
                        errors.push(s.to_string());
                    } else if let Some(obj) = entry.as_object() {
                        for (k, v) in obj {
                            if let Some(s) = v.as_str() {
                                errors.push(format!("{}: {}", k, s));
                            } else {
                                errors.push(format!("{}: {}", k, value_to_short_string(v)));
                            }
                        }
                    } else {
                        errors.push(value_to_short_string(entry));
                    }
                }
            } else if let Some(obj) = resp.get("errors").and_then(|e| e.as_object()) {
                for (k, v) in obj {
                    if let Some(s) = v.as_str() {
                        errors.push(format!("{}: {}", k, s));
                    } else {
                        errors.push(format!("{}: {}", k, value_to_short_string(v)));
                    }
                }
            }
            let code = resp.get("code").and_then(|c| c.as_str()).map(|s| s.to_string());
            let detail = resp.get("detail").and_then(|d| d.as_str()).map(|s| s.to_string());
            // Do not expose raw JSON to rendered templates - keep UI friendly.
            let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
                return render_template(&state, &jar, Step8Template {
                    current_user,
                    api_hostname,
                    base_url,
                    flash_messages,
                    has_flash_messages,
                    back_url: absolute_url_from_state(&state, "/create/step-6"),
                    status_label: "Failed".into(),
                    code,
                    detail,
                    errors,
                });
        }
    }
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    let mut plan_summary = Vec::new();
    let mut price_entries = Vec::new();
    let mut footnote = None;
    
    if base.plan_type == "fixed" {
        let products = load_products_wrapper(&state, &base.region).await;
        if let Some(prod) = products.into_iter().find(|p| p.id == plan_state.product_id) {
            plan_summary = prod.spec_entries.clone();
            price_entries = prod.price_entries.clone();
            let desc = prod.description.clone();
            if !desc.trim().is_empty() {
                footnote = Some(desc.clone());
            }
        }
    } else {
        let mut summary = Vec::new();
        if !plan_state.cpu.trim().is_empty() {
            summary.push(ProductEntry {
                term: "vCPU".into(),
                value: plan_state.cpu.clone(),
            });
        }
        if !plan_state.ram_in_gb.trim().is_empty() {
            summary.push(ProductEntry {
                term: "RAM (GB)".into(),
                value: plan_state.ram_in_gb.clone(),
            });
        }
        if !plan_state.disk_in_gb.trim().is_empty() {
            summary.push(ProductEntry {
                term: "Disk (GB)".into(),
                value: plan_state.disk_in_gb.clone(),
            });
        }
        if !plan_state.bandwidth_in_tb.trim().is_empty() {
            summary.push(ProductEntry {
                term: "Bandwidth (TB)".into(),
                value: plan_state.bandwidth_in_tb.clone(),
            });
        }
        plan_summary = summary;
    }
    let os_list = load_os_list_wrapper(&state).await;
    let selected_os_label = os_list
        .iter()
        .find(|os| os.id == base.os_id)
        .map(|os| os.name.clone())
        .unwrap_or_else(|| base.os_id.clone());
    let selected_key_ids: Vec<String> = base.ssh_key_ids.iter().map(|id| id.to_string()).collect();
    let ssh_keys_display = if selected_key_ids.is_empty() {
        "None".into()
    } else {
        let id_set: HashSet<_> = selected_key_ids.iter().cloned().collect();
        let customer_id = fetch_default_customer_id(&state).await;
        let ssh_keys = load_ssh_keys_api(&state, customer_id).await;
        let mut names = Vec::new();
        for key in ssh_keys {
            if id_set.contains(&key.id) {
                names.push(key.name);
            }
        }
        if names.is_empty() {
            format!("{} SSH key(s)", id_set.len())
        } else {
            names.join(", ")
        }
    };
    let hostnames_display = if base.hostnames.is_empty() {
        "(none)".into()
    } else {
        base.hostnames.join(", ")
    };
    let hostnames_csv = base.hostnames.join(",");
    let ssh_key_ids_csv = if selected_key_ids.is_empty() {
        "".into()
    } else {
        selected_key_ids.join(",")
    };
    let plan_type_label = if base.plan_type == "fixed" {
        "Fixed plan".into()
    } else {
        "Custom plan".into()
    };
    let mut back_pairs = build_base_query_pairs(&base);
    if base.plan_type == "fixed" {
        back_pairs.push(("product_id".into(), plan_state.product_id.clone()));
        back_pairs.push(("extra_disk".into(), plan_state.extra_disk.clone()));
        back_pairs.push(("extra_bandwidth".into(), plan_state.extra_bandwidth.clone()));
    } else {
        back_pairs.push(("cpu".into(), plan_state.cpu.clone()));
        back_pairs.push(("ramInGB".into(), plan_state.ram_in_gb.clone()));
        back_pairs.push(("diskInGB".into(), plan_state.disk_in_gb.clone()));
        back_pairs.push(("bandwidthInTB".into(), plan_state.bandwidth_in_tb.clone()));
    }
    let back_q = build_query_string(&back_pairs);
    let back_url = if back_q.is_empty() {
        absolute_url_from_state(&state, "/create/step-6")
    } else {
        absolute_url_from_state(&state, &format!("/create/step-6?{}", back_q))
    };
    let has_plan_summary = !plan_summary.is_empty();
    let has_price_entries = !price_entries.is_empty();
    let footnote_text = footnote.unwrap_or_default();
    let has_footnote = !footnote_text.is_empty();
    render_template(&state, &jar, Step7Template {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            base_state: &base,
            floating_ip_count: base.floating_ip_count.to_string(),
            plan_state,
            plan_type_label,
            region_name: base.region.clone(),
            hostnames_display,
            plan_summary,
            has_plan_summary,
            price_entries,
            has_price_entries,
            selected_product_name: None,
            selected_product_tags: None,
            selected_product_description: None,
            selected_os_label,
            ssh_keys_display,
            ssh_key_ids_csv,
            hostnames_csv,
            footnote_text,
            has_footnote,
            back_url,
            submit_url: absolute_url_from_state(&state, "/create/step-7"),
        },
    )
}

pub async fn create_step_7_get(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, OneOrMany>>,
) -> impl IntoResponse {
    // For GET requests, query params may have single or multiple values; flatten to CSV strings.
    let mut q_flat: HashMap<String, String> = HashMap::new();
    for (k, v) in q {
        q_flat.insert(k, v.to_csv());
    }
    create_step_7_core(state, jar, axum::http::Method::GET, q_flat, HashMap::new()).await
}

pub async fn create_step_8(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    let code = q.get("code").cloned();
    let detail = q.get("detail").cloned();
    // Raw JSON is no longer rendered in the UI; any raw response can be logged by server
    let errors = q.get("errors").map(|s| s.split('|').map(|s| s.to_string()).collect()).unwrap_or_else(Vec::new);
    render_template(&state, &jar, Step8Template {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
        back_url: q.get("back_url").cloned().unwrap_or_else(|| absolute_url_from_state(&state, "/create/step-1")),
        status_label: q.get("status_label").cloned().unwrap_or_else(|| "Result".into()),
        code,
        detail,
        errors,
        
    })
}

pub async fn create_step_7_post(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, OneOrMany>>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let mut q_flat: HashMap<String, String> = HashMap::new();
    for (k, v) in q {
        q_flat.insert(k, v.to_csv());
    }
    // Try to parse as HashMap<String, Vec<String>> first
    let mut f_flat: HashMap<String, String> = HashMap::new();
    let parsed_map = parse_urlencoded_body(&body);
    for (k, v) in parsed_map {
        f_flat.insert(k, v.join(","));
    }
    create_step_7_core(state, jar, axum::http::Method::POST, q_flat, f_flat).await
}
