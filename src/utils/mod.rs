// URL handling utilities
pub mod url_encoding;
pub mod url_parser;
pub mod url_builder;
pub mod query_string;

// Parsing utilities
pub mod parse_flag;
pub mod parse_int;
pub mod parse_int_list;

// JSON utilities
pub mod json_converter;

// Security utilities
pub mod validation;
pub mod security;

// Re-export all utilities for convenient access
pub use url_encoding::parse_urlencoded_body;
pub use url_parser::hostname_from_url;
pub use url_builder::absolute_url;
pub use query_string::build_query_string;
pub use parse_flag::parse_flag;
pub use parse_int::parse_optional_int;
pub use parse_int_list::parse_int_list;
pub use json_converter::value_to_short_string;
pub use validation::validate_username;
#[allow(unused_imports)]
pub use validation::{validate_password, sanitize_string};
pub use security::{validate_file_permissions, validate_api_token, is_development_mode};
