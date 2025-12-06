use askama::Template;
use crate::models::{CurrentUser, Region, ProductView};

#[derive(Template)]
#[template(path = "products.html")]
pub struct ProductsPageTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub regions: &'a [Region],
    pub selected_region: Option<&'a Region>,
    pub active_region_id: String,
    pub requested_region: Option<String>,
    pub products: &'a [ProductView],
}

crate::impl_base_template!(ProductsPageTemplate<'_>);
