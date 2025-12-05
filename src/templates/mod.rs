// Base template trait for inheritance
pub mod base_template;
pub use base_template::BaseTemplate;

// Individual template files
pub mod login_template;
pub mod regions_page_template;
pub mod products_page_template;
pub mod os_catalog_template;
pub mod applications_template;
pub mod instance_detail_template;
pub mod bulk_refund_template;
pub mod users_page_template;
pub mod access_page_template;
pub mod ssh_keys_page_template;
pub mod instances_page_template;
pub mod delete_instance_template;
pub mod power_on_instance_template;
pub mod power_off_instance_template;
pub mod reset_instance_template;
pub mod change_pass_instance_template;
pub mod change_os_template;
pub mod resize_template;

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
pub use regions_page_template::RegionsPageTemplate;
pub use products_page_template::ProductsPageTemplate;
pub use os_catalog_template::OsCatalogTemplate;
pub use applications_template::ApplicationsTemplate;
pub use instance_detail_template::InstanceDetailTemplate;
pub use bulk_refund_template::BulkRefundTemplate;
pub use users_page_template::UsersPageTemplate;
pub use access_page_template::AccessPageTemplate;
pub use ssh_keys_page_template::SshKeysPageTemplate;
pub use instances_page_template::InstancesPageTemplate;
pub use delete_instance_template::DeleteInstanceTemplate;
pub use power_on_instance_template::PowerOnInstanceTemplate;
pub use power_off_instance_template::PowerOffInstanceTemplate;
pub use reset_instance_template::ResetInstanceTemplate;
pub use change_pass_instance_template::ChangePassInstanceTemplate;
pub use change_os_template::ChangeOsTemplate;
pub use resize_template::ResizeTemplate;

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
