use askama::Template;
use crate::models::CurrentUser;
use crate::api::BackupProfileView;

#[derive(Template)]
#[template(path = "backups.html")]
pub struct BackupsTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub backups: &'a [BackupProfileView],
}

crate::impl_base_template!(BackupsTemplate<'_>);
