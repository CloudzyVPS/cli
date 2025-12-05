#[derive(Clone)]
pub struct Step2FormData {
    pub hostnames_text: String,
    pub assign_ipv4: bool,
    pub assign_ipv6: bool,
    pub floating_ip_count: String,
}
