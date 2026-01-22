use askama::Template;
use crate::models::CurrentUser;
use crate::api::ImageView;

#[derive(Template)]
#[template(path = "images.html")]
pub struct ImagesTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub images: &'a [ImageView],
    // pub current_page: usize,
    // pub total_pages: usize,
    // pub per_page: usize,
    pub total_count: usize,
}

crate::impl_base_template!(ImagesTemplate<'_>);
