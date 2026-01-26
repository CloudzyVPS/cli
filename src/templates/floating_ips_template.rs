use askama::Template;
use crate::models::{CurrentUser, Region};
use crate::api::FloatingIpView;

#[derive(Template)]
#[template(path = "floating_ips.html")]
pub struct FloatingIpsTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub floating_ips: &'a [FloatingIpView],
    pub current_page: usize,
    pub total_pages: usize,
    pub per_page: usize,
    pub total_count: usize,
    pub regions: &'a [Region],
}

crate::impl_base_template!(FloatingIpsTemplate<'_>);
