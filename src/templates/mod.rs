// Base template trait for inheritance
pub mod base_template;
pub use base_template::BaseTemplate;

// Individual template files
pub mod login_template;
pub mod instance_detail_template;
pub mod users_page_template;
pub mod user_detail_template;
pub mod access_page_template;
pub mod about_template;
pub mod confirmation_template;
pub mod ssh_keys_page_template;
pub mod ssh_key_detail_template;
pub mod instances_page_template;
pub mod change_pass_instance_template;
pub mod change_os_instance_template;
pub mod resize_template;
pub mod coming_soon_template;
pub mod snapshots_template;
pub mod snapshot_detail_template;
pub mod floating_ips_template;
pub mod isos_template;
pub mod images_template;
pub mod backups_template;
pub mod workspaces_template;
pub mod workspace_detail_template;
pub mod workspace_instances_template;
pub mod permissions_template;

// Wizard templates (now in templates/)
pub mod step1_template;
pub mod step2_template;
pub mod step3_fixed_template;
pub mod step3_custom_template;
pub mod step4_template;
pub mod step5_template;
pub mod step6_template;
pub mod step7_template;
pub mod step8_template;

// Re-export all templates
pub use login_template::LoginTemplate;
pub use instance_detail_template::InstanceDetailTemplate;
pub use users_page_template::UsersPageTemplate;
pub use user_detail_template::UserDetailTemplate;
pub use access_page_template::AccessPageTemplate;
pub use about_template::AboutTemplate;
pub use confirmation_template::ConfirmationTemplate;
pub use ssh_keys_page_template::SshKeysPageTemplate;
pub use ssh_key_detail_template::SshKeyDetailTemplate;
pub use instances_page_template::InstancesPageTemplate;
pub use change_pass_instance_template::ChangePassInstanceTemplate;
pub use change_os_instance_template::ChangeOsInstanceTemplate;
pub use resize_template::ResizeTemplate;
pub use coming_soon_template::ComingSoonTemplate;
pub use snapshots_template::SnapshotsTemplate;
pub use snapshot_detail_template::SnapshotDetailTemplate;
pub use floating_ips_template::FloatingIpsTemplate;
pub use isos_template::IsosTemplate;
pub use images_template::ImagesTemplate;
pub use backups_template::BackupsTemplate;
pub use workspaces_template::WorkspacesTemplate;
pub use workspace_detail_template::WorkspaceDetailTemplate;
pub use workspace_instances_template::WorkspaceInstancesTemplate;
pub use permissions_template::PermissionsTemplate;

// Wizard templates
pub use step1_template::Step1Template;
pub use step2_template::Step2Template;
pub use step3_fixed_template::Step3FixedTemplate;
pub use step3_custom_template::Step3CustomTemplate;
pub use step4_template::Step4Template;
pub use step5_template::Step5Template;
pub use step6_template::Step6Template;
pub use step7_template::Step7Template;
pub use step8_template::Step8Template;

// Type aliases for shorter names used in main.rs
pub type UsersTemplate<'a> = UsersPageTemplate<'a>;
pub type InstancesTemplate<'a> = InstancesPageTemplate<'a>;
pub type AccessTemplate<'a> = AccessPageTemplate<'a>;
pub type SshKeysTemplate<'a> = SshKeysPageTemplate<'a>;
