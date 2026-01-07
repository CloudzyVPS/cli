// Atomic API modules
pub mod client;
pub mod regions;
pub mod products;
pub mod operating_systems;
pub mod instances;
pub mod ssh_keys;
pub mod snapshots;

// Re-export commonly used functions
pub use client::api_call;
pub use regions::load_regions;
pub use products::load_products;
pub use operating_systems::load_os_list;
pub use instances::{load_instances_for_user, PaginatedInstances};
pub use ssh_keys::{load_ssh_keys, load_ssh_keys_paginated, PaginatedSshKeys};
pub use snapshots::{
    load_snapshots, create_snapshot, get_snapshot, delete_snapshot, restore_snapshot,
    SnapshotView, PaginatedSnapshots,
};
