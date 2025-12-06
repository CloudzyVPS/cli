use askama::Template;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;
use std::collections::HashMap;

use crate::api::{load_applications, load_os_list, load_products, load_regions};
use crate::models::AppState;
use crate::templates::{ApplicationsTemplate, OsCatalogTemplate, ProductsPageTemplate, RegionsPageTemplate};

use super::helpers::{build_template_globals, ensure_logged_in, inject_context, TemplateGlobals};

pub async fn regions_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    let (list, _map) = load_regions(&state.client, &state.api_base_url, &state.api_token).await;
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    inject_context(
        &state,
        &jar,
        RegionsPageTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            regions: &list,
        }
        .render()
        .unwrap(),
    )
}

pub async fn products_get(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    let region_id = q.get("region").cloned().unwrap_or_default();
    if region_id.is_empty() {
        return Redirect::to("/regions").into_response();
    }
    let products = load_products(&state.client, &state.api_base_url, &state.api_token, &region_id).await;
    let (list, regions_map) = load_regions(&state.client, &state.api_base_url, &state.api_token).await;
    let selected_region = regions_map.get(&region_id);
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    inject_context(
        &state,
        &jar,
        ProductsPageTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            regions: &list,
            selected_region,
            active_region_id: region_id.clone(),
            requested_region: Some(region_id),
            products: &products,
        }
        .render()
        .unwrap(),
    )
}

pub async fn os_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    let list = load_os_list(&state.client, &state.api_base_url, &state.api_token).await;
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    inject_context(
        &state,
        &jar,
        OsCatalogTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            os_list: &list,
        }
        .render()
        .unwrap(),
    )
}

pub async fn applications_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    let apps = load_applications(&state.client, &state.api_base_url, &state.api_token).await;
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    inject_context(
        &state,
        &jar,
        ApplicationsTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            apps: &apps,
        }
        .render()
        .unwrap(),
    )
}
