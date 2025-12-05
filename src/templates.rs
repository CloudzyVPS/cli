use askama::Template;
use crate::users::CurrentUser;
use crate::api::{Region, ProductView, OsItem, ApplicationView, InstanceView};

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "regions.html")]
pub struct RegionsPageTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub regions: &'a [Region],
}

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

#[derive(Template)]
#[template(path = "os.html")]
pub struct OsCatalogTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub os_list: &'a [OsItem],
}

#[derive(Template)]
#[template(path = "applications.html")]
pub struct ApplicationsTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub apps: &'a [ApplicationView],
}

#[derive(Template)]
#[template(path = "instance_detail.html")]
pub struct InstanceDetailTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance_id: String,
    pub hostname: String,
    pub details: Vec<(String, String)>,
    pub is_disabled: bool,
}

#[derive(Template)]
#[template(path = "bulk_refund.html")]
pub struct BulkRefundTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
}

#[derive(Template)]
#[template(path = "users.html")]
pub struct UsersPageTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub users: &'a [(String, String, Vec<String>)],
}

#[derive(Template)]
#[template(path = "access.html")]
pub struct AccessPageTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub users: &'a [(String, String)],
}

#[derive(Template)]
#[template(path = "ssh_keys.html")]
pub struct SshKeysPageTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub keys: &'a [SshKeyView],
}

pub struct SshKeyView {
    pub id: i64,
    pub name: String,
    pub date_created: String,
}

#[derive(Template)]
#[template(path = "instances.html")]
pub struct InstancesPageTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instances: &'a [InstanceView],
}

#[derive(Template)]
#[template(path = "delete_instance.html")]
pub struct DeleteInstanceTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance: InstanceView,
    pub is_disabled: bool,
}

#[derive(Template)]
#[template(path = "poweron_instance.html")]
pub struct PowerOnInstanceTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance: InstanceView,
    pub is_disabled: bool,
}

#[derive(Template)]
#[template(path = "poweroff_instance.html")]
pub struct PowerOffInstanceTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance: InstanceView,
    pub is_disabled: bool,
}

#[derive(Template)]
#[template(path = "reset_instance.html")]
pub struct ResetInstanceTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance: InstanceView,
    pub is_disabled: bool,
}

#[derive(Template)]
#[template(path = "change_pass_instance.html")]
pub struct ChangePassInstanceTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance: InstanceView,
    pub new_password: Option<String>,
    pub is_disabled: bool,
}

#[derive(Template)]
#[template(path = "change_os.html")]
pub struct ChangeOsTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance: InstanceView,
    pub os_list: &'a [OsItem],
    pub is_disabled: bool,
}

#[derive(Template)]
#[template(path = "resize.html")]
pub struct ResizeTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance: InstanceView,
    pub regions: &'a [Region],
    pub is_disabled: bool,
}
