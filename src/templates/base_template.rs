use crate::models::CurrentUser;

/// Base template trait providing common properties for all templates.
/// This eliminates redundant field definitions across templates.
pub trait BaseTemplate {
    fn current_user(&self) -> &Option<CurrentUser>;
    fn api_hostname(&self) -> &str;
    fn base_url(&self) -> &str;
    fn flash_messages(&self) -> &Vec<String>;
    fn has_flash_messages(&self) -> bool;
}

/// Macro to implement BaseTemplate for a struct with standard fields
#[macro_export]
macro_rules! impl_base_template {
    ($struct_name:ty) => {
        impl BaseTemplate for $struct_name {
            fn current_user(&self) -> &Option<CurrentUser> {
                &self.current_user
            }
            fn api_hostname(&self) -> &str {
                &self.api_hostname
            }
            fn base_url(&self) -> &str {
                &self.base_url
            }
            fn flash_messages(&self) -> &Vec<String> {
                &self.flash_messages
            }
            fn has_flash_messages(&self) -> bool {
                self.has_flash_messages
            }
        }
    };
}
