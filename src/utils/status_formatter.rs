pub fn format_status(status: &str) -> String {
    match status.to_lowercase().as_str() {
        "preparing_disk" => "Preparing Disk".to_string(),
        "initializing" => "Initializing".to_string(),
        "shutdown" => "Shutdown".to_string(),
        "active" => "Active".to_string(),
        _ => status.to_string(),
    }
}

