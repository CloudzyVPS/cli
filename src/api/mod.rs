// Atomic API modules
pub mod client;
pub mod regions;
pub mod products;
pub mod operating_systems;
pub mod instances;
pub mod ssh_keys;
pub mod snapshots;
pub mod applications;
pub mod floating_ips;
pub mod iso;
pub mod images;

// Re-export commonly used functions
pub use client::api_call;
pub use regions::load_regions;
pub use products::load_products;
pub use operating_systems::load_os_list;
pub use instances::{load_instances_for_user, PaginatedInstances};
pub use ssh_keys::{load_ssh_keys, load_ssh_keys_paginated, get_ssh_key, PaginatedSshKeys};
pub use snapshots::{
    load_snapshots, create_snapshot, get_snapshot, delete_snapshot, restore_snapshot,
    SnapshotView,
};
pub use applications::{load_applications, Application};
pub use floating_ips::{
    load_floating_ips, create_floating_ips, update_floating_ip, release_floating_ip,
    FloatingIpView,
};
pub use iso::{load_isos, download_iso, get_iso, delete_iso, IsoView};
pub use images::{load_images, download_image, get_image, delete_image, ImageView};
