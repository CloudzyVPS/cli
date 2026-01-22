use crate::models::CurrentUser;

/// Base template trait providing common properties for all templates.
/// This eliminates redundant field definitions across templates.
/// 
/// # Macro Usage
/// 
/// This trait is implemented automatically by the `impl_base_template!` macro for all template structs.
/// The compiler does not detect this macro-based usage, which is why this trait definition exists.
/// 
/// The macro provides automatic implementations for standard template fields:
/// - current_user: Current authenticated user information
/// - api_hostname: Hostname extracted from API base URL
/// - base_url: Public base URL for the application
/// - flash_messages: List of flash messages to display
/// - has_flash_messages: Boolean indicating if flash messages exist
/// 
/// Without this trait, each template would need to duplicate these common field definitions.
#[allow(dead_code)] // Used by impl_base_template! macro
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
    // For structs with lifetimes
    ($struct_name:ident<'_>) => {
        impl $crate::templates::BaseTemplate for $struct_name<'_> {
            fn current_user(&self) -> &Option<$crate::models::CurrentUser> {
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
    // For structs without lifetimes
    ($struct_name:ident) => {
        impl $crate::templates::BaseTemplate for $struct_name {
            fn current_user(&self) -> &Option<$crate::models::CurrentUser> {
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
