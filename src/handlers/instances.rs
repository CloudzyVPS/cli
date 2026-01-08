use axum::{
    extract::{State, Path, Form, Query},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use serde_json::Value;

use crate::models::{
    AppState, InstanceView, AddTrafficForm, ResizeForm, OsItem,
};
use crate::templates::{
    InstancesTemplate, InstanceDetailTemplate,
    ChangePassInstanceTemplate, ChangeOsInstanceTemplate, ResizeTemplate,
};
use crate::handlers::helpers::{
    build_template_globals, current_username_from_jar,
    render_template, api_call_wrapper, TemplateGlobals,
    load_regions_wrapper, load_products_wrapper,
    load_instances_for_user_paginated,
};
use crate::api::load_os_list;
use crate::services::instance_service::{enforce_instance_access, simple_instance_action};
use crate::services::persist_users_file;

#[derive(Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    page: usize,
    #[serde(default = "default_per_page")]
    per_page: usize,
}

fn default_page() -> usize {
    1
}

fn default_per_page() -> usize {
    10
}

pub async fn instances_real(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let username = current_username_from_jar(&state, &jar).expect("Middleware ensures user is logged in");
    let paginated = load_instances_for_user_paginated(&state, &username, params.page, params.per_page).await;
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    render_template(&state, &jar, InstancesTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            instances: &paginated.instances,
            current_page: paginated.current_page,
            total_pages: paginated.total_pages,
            per_page: paginated.per_page,
            total_count: paginated.total_count,
        },
    )
}

pub async fn instance_detail(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(instance_id): Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
    
    let mut details: Vec<(String, String)> = Vec::new();
    let mut hostname = "(no hostname)".to_string();
    let mut status = "".to_string();
    if let Some(obj) = payload.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            hostname = data
                .get("hostname")
                .and_then(|v| v.as_str())
                .unwrap_or("(no hostname)")
                .to_string();
            details.push(("Hostname".into(), hostname.clone()));
            status = data
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let status_display = crate::utils::format_status(&status);
            details.push(("Status".into(), status_display));
            let region = data
                .get("region")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            details.push(("Region".into(), region.clone()));
            let class = data
                .get("class")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            details.push(("Instance class".into(), class));
            let product_id = data
                .get("productId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if let Some(pid) = product_id.clone() {
                let product_name = if !region.is_empty() && !pid.is_empty() {
                    let products = load_products_wrapper(&state, &region).await;
                    products
                        .into_iter()
                        .find(|p| p.id == pid)
                        .map(|p| p.name)
                        .unwrap_or(pid.clone())
                } else {
                    pid.clone()
                };
                details.push(("Product".into(), product_name));
            }
            let vcpu = data.get("vcpuCount").and_then(|v| v.as_i64()).map(|v| v.to_string());
            if let Some(x) = vcpu { details.push(("vCPU".into(), x)); }
            let ram = data.get("ram").and_then(|v| v.as_i64()).map(|v| format!("{} MB", v));
            if let Some(x) = ram { details.push(("RAM".into(), x)); }
            let disk = data.get("disk").and_then(|v| v.as_i64()).map(|v| format!("{} GB", v));
            if let Some(x) = disk { details.push(("Disk".into(), x)); }
            let ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            if let Some(x) = ip { details.push(("IPv4".into(), x)); }
            let ip6 = data.get("mainIpv6").and_then(|v| v.as_str()).map(|s| s.to_string());
            if let Some(x) = ip6 { details.push(("IPv6".into(), x)); }
            if let Some(os_obj) = data.get("os") {
                let os_name = os_obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .or_else(|| os_obj.get("id").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string();
                if !os_name.is_empty() { details.push(("OS".into(), os_name)); }
            }
            if let Some(inserted) = data.get("insertedAt").and_then(|v| v.as_str()).map(|s| s.to_string()) {
                details.push(("Created".into(), inserted));
            }
            if let Some(features) = data.get("features").and_then(|v| v.as_array()) {
                let mut features_list = Vec::new();
                for item in features { if let Some(s) = item.as_str() { features_list.push(s.to_string()); } }
                if !features_list.is_empty() { details.push(("Features".into(), features_list.join(", "))); }
            }
        }
    }
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    let disabled_by_env = state.is_instance_disabled(&instance_id);
    let disabled_by_host = state.is_hostname_blocked(&hostname);
    
    render_template(&state, &jar, InstanceDetailTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            instance_id: instance_id.clone(),
            hostname,
            status,
            details,
            disabled_by_env,
            disabled_by_host,
        },
    )
}

pub async fn instance_poweron_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(instance_id): Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if let Some(reason) = crate::services::instance_service::check_instance_block(&state, &instance_id, None).await {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push(reason.message());
        }
        return Redirect::to(&format!("/instance/{}", instance_id)).into_response();
    }
    let _ = simple_instance_action(&state, "poweron", &instance_id).await;
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}

pub async fn instance_poweroff_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(instance_id): Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if let Some(reason) = crate::services::instance_service::check_instance_block(&state, &instance_id, None).await {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push(reason.message());
        }
        return Redirect::to(&format!("/instance/{}", instance_id)).into_response();
    }
    let _ = simple_instance_action(&state, "poweroff", &instance_id).await;
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}

pub async fn instance_reset_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(instance_id): Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if let Some(reason) = crate::services::instance_service::check_instance_block(&state, &instance_id, None).await {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push(reason.message());
        }
        return Redirect::to(&format!("/instance/{}", instance_id)).into_response();
    }
    let _ = simple_instance_action(&state, "reset", &instance_id).await;
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}

pub async fn instance_change_pass_get(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(instance_id): Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
    let mut instance = InstanceView::new_with_defaults(instance_id.clone());
    if let Some(obj) = payload.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            instance.hostname = data.get("hostname").and_then(|v| v.as_str()).unwrap_or(&instance.hostname).to_string();
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.main_ipv6 = data.get("mainIpv6").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.status_display = crate::utils::format_status(&instance.status);
        }
    }
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    let disabled_by_env = state.is_instance_disabled(&instance_id);
    let disabled_by_host = state.is_hostname_blocked(&instance.hostname);
    render_template(&state, &jar, ChangePassInstanceTemplate { current_user, api_hostname, base_url, flash_messages, has_flash_messages, instance, new_password: None, disabled_by_env, disabled_by_host })
}

pub async fn instance_change_pass_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(instance_id): Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if let Some(reason) = crate::services::instance_service::check_instance_block(&state, &instance_id, None).await {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push(reason.message());
        }
        return Redirect::to(&format!("/instance/{}/change-pass", instance_id)).into_response();
    }
    let endpoint = format!("/v1/instances/{}/change-pass", instance_id);
    let payload = api_call_wrapper(&state, "POST", &endpoint, None, None).await;
    let new_password = payload.get("data").and_then(|d| d.get("password")).and_then(|v| v.as_str()).map(|s| s.to_string());
    let get_endpoint = format!("/v1/instances/{}", instance_id);
    let payload2 = api_call_wrapper(&state, "GET", &get_endpoint, None, None).await;
    let mut instance = InstanceView::new_with_defaults(instance_id.clone());
    if let Some(obj) = payload2.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            instance.hostname = data.get("hostname").and_then(|v| v.as_str()).unwrap_or(&instance.hostname).to_string();
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.main_ipv6 = data.get("mainIpv6").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.status_display = crate::utils::format_status(&instance.status);
        }
    }
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    let disabled_by_env = state.is_instance_disabled(&instance_id);
    let disabled_by_host = state.is_hostname_blocked(&instance.hostname);
    render_template(&state, &jar, ChangePassInstanceTemplate { current_user, api_hostname, base_url, flash_messages, has_flash_messages, instance, new_password, disabled_by_env, disabled_by_host })
}

pub async fn instance_delete(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(instance_id): Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if let Some(reason) = crate::services::instance_service::check_instance_block(&state, &instance_id, None).await {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push(reason.message());
        }
        return Redirect::to(&format!("/instance/{}", instance_id)).into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call_wrapper(&state, "DELETE", &endpoint, None, None).await;
    
    let success = payload.get("code").and_then(|c| c.as_str()) == Some("OKAY");
    
    if success {
        {
            let mut users = state.users.lock().unwrap();
            for (_, rec) in users.iter_mut() {
                if rec.assigned_instances.contains(&instance_id) {
                    rec.assigned_instances.retain(|x| x != &instance_id);
                }
            }
        }
        if let Err(e) = persist_users_file(&state.users).await {
            tracing::error!(%e, "Failed to persist users after instance deletion");
        }
    }

    if let Some(sid) = jar.get("session_id") {
        let mut flashes = state.flash_store.lock().unwrap();
        let entry = flashes.entry(sid.value().to_string()).or_default();
        if success {
            entry.push("Instance deleted successfully.".into());
            return Redirect::to("/instances").into_response();
        } else {
            let detail = payload.get("detail").and_then(|d| d.as_str()).unwrap_or("Unknown error");
            entry.push(format!("Delete failed: {}", detail));
            return Redirect::to(&format!("/instance/{}", instance_id)).into_response();
        }
    }
    
    if success {
        Redirect::to("/instances").into_response()
    } else {
        Redirect::to(&format!("/instance/{}", instance_id)).into_response()
    }
}

pub async fn instance_add_traffic(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(instance_id): Path<String>,
    Form(form): Form<AddTrafficForm>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if let Some(reason) = crate::services::instance_service::check_instance_block(&state, &instance_id, None).await {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push(reason.message());
        }
        return Redirect::to(&format!("/instance/{}", instance_id)).into_response();
    }
    if let Ok(amount) = form.traffic_amount.parse::<f64>() {
        if amount > 0.0 {
            let endpoint = format!("/v1/instances/{}/add-traffic", instance_id);
            let payload = serde_json::json!({"amount": amount});
            let _ = api_call_wrapper(&state, "POST", &endpoint, Some(payload), None).await;
        }
    }
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}

pub async fn instance_resize_get(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(instance_id): Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
    let mut instance = InstanceView::new_with_defaults(instance_id.clone());
    if let Some(obj) = payload.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            instance.hostname = data.get("hostname").and_then(|v| v.as_str()).unwrap_or(&instance.hostname).to_string();
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.main_ipv6 = data.get("mainIpv6").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.status_display = crate::utils::format_status(&instance.status);
        }
    }
    let (regions, _map) = load_regions_wrapper(&state).await;
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    let disabled_by_env = state.is_instance_disabled(&instance_id);
    let disabled_by_host = state.is_hostname_blocked(&instance.hostname);
    render_template(&state, &jar, ResizeTemplate { current_user, api_hostname, base_url, flash_messages, has_flash_messages, instance, regions: &regions, disabled_by_env, disabled_by_host })
}

pub async fn instance_resize_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(instance_id): Path<String>,
    Form(form): Form<ResizeForm>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if let Some(reason) = crate::services::instance_service::check_instance_block(&state, &instance_id, None).await {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push(reason.message());
        }
        return Redirect::to(&format!("/instance/{}/resize", instance_id)).into_response();
    }
    let endpoint = format!("/v1/instances/{}/resize", instance_id);
    let mut payload = serde_json::json!({"type": form.r#type});

    if let Some(pid) = form.product_id {
        if !pid.trim().is_empty() {
            payload["productId"] = Value::from(pid);
        }
    }

    if let Some(rid) = form.region_id {
        if !rid.trim().is_empty() {
            payload["regionId"] = Value::from(rid);
        }
    }

    // Build extraResource based on resize type
    let mut extra_resource = serde_json::Map::new();
    
    if form.r#type.to_uppercase() == "FIXED" {
        // For FIXED resize: only diskInGB and bandwidthInTB are allowed
        if let Some(disk) = form.disk_in_gb {
            if let Ok(n) = disk.parse::<i64>() {
                if n > 0 {
                    extra_resource.insert("diskInGB".into(), Value::from(n));
                }
            }
        }
        if let Some(bw) = form.bandwidth_in_tb {
            if let Ok(n) = bw.parse::<i64>() {
                if n > 0 {
                    extra_resource.insert("bandwidthInTB".into(), Value::from(n));
                }
            }
        }
    } else if form.r#type.to_uppercase() == "CUSTOM" {
        // For CUSTOM resize: cpu, ramInGB, diskInGB, and bandwidthInTB are required
        if let Some(cpu) = form.cpu {
            if let Ok(n) = cpu.parse::<i64>() {
                extra_resource.insert("cpu".into(), Value::from(n));
            }
        }
        if let Some(ram) = form.ram_in_gb {
            if let Ok(n) = ram.parse::<i64>() {
                extra_resource.insert("ramInGB".into(), Value::from(n));
            }
        }
        if let Some(disk) = form.disk_in_gb {
            if let Ok(n) = disk.parse::<i64>() {
                extra_resource.insert("diskInGB".into(), Value::from(n));
            }
        }
        if let Some(bw) = form.bandwidth_in_tb {
            if let Ok(n) = bw.parse::<i64>() {
                extra_resource.insert("bandwidthInTB".into(), Value::from(n));
            }
        }
    }
    
    if !extra_resource.is_empty() {
        payload["extraResource"] = Value::Object(extra_resource);
    }
    let resp = api_call_wrapper(&state, "POST", &endpoint, Some(payload), None).await;
    
    if let Some(sid) = jar.get("session_id") {
        let mut flashes = state.flash_store.lock().unwrap();
        let entry = flashes.entry(sid.value().to_string()).or_default();
        if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
            entry.push("Instance resize initiated successfully.".into());
        } else {
            let detail = resp.get("detail").and_then(|d| d.as_str()).unwrap_or("Unknown error");
            entry.push(format!("Resize failed: {}", detail));
        }
    }

    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}

#[derive(Deserialize)]
pub struct ChangeOsForm {
    pub os_id: String,
}

pub async fn instance_change_os_get(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(instance_id): Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
    let mut instance = InstanceView { 
        id: instance_id.clone(), 
        hostname: "(no hostname)".into(), 
        region: "".into(), 
        main_ip: None, 
        main_ipv6: None, 
        status: "".into(), 
        status_display: "".into(), 
        vcpu_count_display: "—".into(), 
        ram_display: "—".into(), 
        disk_display: "—".into(), 
        os: None 
    };
    if let Some(obj) = payload.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            instance.hostname = data.get("hostname").and_then(|v| v.as_str()).unwrap_or(&instance.hostname).to_string();
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.main_ipv6 = data.get("mainIpv6").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.status_display = crate::utils::format_status(&instance.status);
            if let Some(os_obj) = data.get("os").and_then(|v| v.as_object()) {
                instance.os = Some(OsItem {
                    id: os_obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    name: os_obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    family: os_obj.get("family").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    arch: os_obj.get("arch").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    min_ram: os_obj.get("minRam").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    is_default: os_obj.get("isDefault").and_then(|v| v.as_bool()).unwrap_or(false),
                });
            }
        }
    }
    
    let os_list = load_os_list(&state.client, &state.api_base_url, &state.api_token).await;
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    let disabled_by_env = state.is_instance_disabled(&instance_id);
    let disabled_by_host = state.is_hostname_blocked(&instance.hostname);
    render_template(&state, &jar, ChangeOsInstanceTemplate { 
        current_user, 
        api_hostname, 
        base_url, 
        flash_messages, 
        has_flash_messages, 
        instance, 
        os_list, 
        disabled_by_env, 
        disabled_by_host 
    })
}

pub async fn instance_change_os_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(instance_id): Path<String>,
    Form(form): Form<ChangeOsForm>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if let Some(reason) = crate::services::instance_service::check_instance_block(&state, &instance_id, None).await {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push(reason.message());
        }
        return Redirect::to(&format!("/instance/{}/change-os", instance_id)).into_response();
    }
    
    let endpoint = format!("/v1/instances/{}/change-os", instance_id);
    let payload = serde_json::json!({"osId": form.os_id});
    let resp = api_call_wrapper(&state, "POST", &endpoint, Some(payload), None).await;
    
    if let Some(sid) = jar.get("session_id") {
        let mut flashes = state.flash_store.lock().unwrap();
        let entry = flashes.entry(sid.value().to_string()).or_default();
        if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
            entry.push("OS change initiated successfully.".into());
        } else {
            let detail = resp.get("detail").and_then(|d| d.as_str()).unwrap_or("Unknown error");
            entry.push(format!("OS change failed: {}", detail));
        }
    }
    
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}
