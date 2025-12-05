use std::collections::HashMap;

use crate::util::{parse_flag, parse_optional_int, parse_int_list};
use crate::models::BaseState;

pub fn parse_wizard_base(query: &HashMap<String, String>) -> BaseState {
    let mut hostnames: Vec<String> = query
        .get("hostnames")
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    hostnames.retain(|h| !h.is_empty());
    let region = query
        .get("region")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let instance_class = query
        .get("instance_class")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "default".into());
    let plan_type = query
        .get("plan_type")
        .map(|s| s.trim().to_lowercase())
        .filter(|s| matches!(s.as_str(), "fixed" | "custom"))
        .unwrap_or_else(|| "fixed".into());
    let assign_ipv4 = parse_flag(query.get("assign_ipv4"), true);
    let assign_ipv6 = parse_flag(query.get("assign_ipv6"), false);
    let floating_ip_count = parse_optional_int(query.get("floating_ip_count")).unwrap_or(0);
    let ssh_raw = query
        .get("ssh_key_ids")
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let ssh_key_ids = parse_int_list(&ssh_raw);
    let os_id = query
        .get("os_id")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    BaseState {
        hostnames,
        region,
        instance_class,
        plan_type,
        assign_ipv4,
        assign_ipv6,
        floating_ip_count,
        ssh_key_ids,
        os_id,
    }
}

pub fn build_base_query_pairs(state: &BaseState) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for h in &state.hostnames {
        pairs.push(("hostnames".into(), h.clone()));
    }
    if !state.region.is_empty() {
        pairs.push(("region".into(), state.region.clone()));
    }
    pairs.push(("instance_class".into(), state.instance_class.clone()));
    pairs.push(("plan_type".into(), state.plan_type.clone()));
    pairs.push(("assign_ipv4".into(), (state.assign_ipv4 as u8).to_string()));
    pairs.push(("assign_ipv6".into(), (state.assign_ipv6 as u8).to_string()));
    if state.floating_ip_count > 0 {
        pairs.push((
            "floating_ip_count".into(),
            state.floating_ip_count.to_string(),
        ));
    }
    for id in &state.ssh_key_ids {
        pairs.push(("ssh_key_ids".into(), id.to_string()));
    }
    if !state.os_id.is_empty() {
        pairs.push(("os_id".into(), state.os_id.clone()));
    }
    pairs
}
