/// Formats a raw status string into a human-readable display format.
///
/// This function converts status values from their API representation (e.g., "preparing_disk")
/// into user-friendly display strings (e.g., "Preparing Disk").
///
/// # Arguments
///
/// * `status` - A string slice containing the raw status value
///
/// # Returns
///
/// A formatted String suitable for display to users. If the status is not recognized,
/// it returns the original status unchanged.
///
/// # Examples
///
/// ```ignore
/// let formatted = format_status("preparing_disk");
/// assert_eq!(formatted, "Preparing Disk");
///
/// let formatted = format_status("active");
/// assert_eq!(formatted, "Active");
/// ```
pub fn format_status(status: &str) -> String {
    match status.to_lowercase().as_str() {
        "preparing_disk" => "Preparing Disk".to_string(),
        "initializing" => "Initializing".to_string(),
        "shutdown" => "Shutdown".to_string(),
        "active" => "Active".to_string(),
        _ => status.to_string(),
    }
}

