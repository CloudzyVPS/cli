// Atomic API modules
pub mod client;
pub mod regions;
pub mod products;
pub mod operating_systems;
pub mod applications;
pub mod instances;
pub mod ssh_keys;

// Re-export commonly used functions
pub use client::api_call;
pub use regions::load_regions;
pub use products::load_products;
pub use operating_systems::load_os_list;
pub use applications::load_applications;
pub use instances::{load_instances_for_user, PaginatedInstances};
pub use ssh_keys::load_ssh_keys;
