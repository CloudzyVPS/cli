use crate::config::PASSWORD_MIN_LENGTH;

/// Validates a password meets security requirements
#[allow(dead_code)]
pub fn validate_password(password: &str) -> Result<(), String> {
    if password.len() < PASSWORD_MIN_LENGTH {
        return Err(format!("Password must be at least {} characters long", PASSWORD_MIN_LENGTH));
    }
    
    // Check for at least one lowercase letter
    if !password.chars().any(|c| c.is_ascii_lowercase()) {
        return Err("Password must contain at least one lowercase letter".to_string());
    }
    
    // Check for at least one uppercase letter
    if !password.chars().any(|c| c.is_ascii_uppercase()) {
        return Err("Password must contain at least one uppercase letter".to_string());
    }
    
    // Check for at least one digit
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err("Password must contain at least one digit".to_string());
    }
    
    Ok(())
}

/// Validates a username meets security requirements
pub fn validate_username(username: &str) -> Result<(), String> {
    if username.is_empty() {
        return Err("Username cannot be empty".to_string());
    }
    
    if username.len() < 3 {
        return Err("Username must be at least 3 characters long".to_string());
    }
    
    if username.len() > 32 {
        return Err("Username must not exceed 32 characters".to_string());
    }
    
    // Check for valid characters (alphanumeric, underscore, hyphen)
    if !username.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err("Username can only contain letters, numbers, underscores, and hyphens".to_string());
    }
    
    // Must start with a letter
    if !username.chars().next().unwrap().is_alphabetic() {
        return Err("Username must start with a letter".to_string());
    }
    
    Ok(())
}

/// Sanitize user input to prevent XSS and injection attacks
#[allow(dead_code)]
pub fn sanitize_string(input: &str) -> String {
    // HTML entity encoding for basic XSS prevention
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
        .replace('/', "&#x2F;")
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_password_valid() {
        assert!(validate_password("ValidPass123").is_ok());
    }
    
    #[test]
    fn test_validate_password_too_short() {
        assert!(validate_password("Short1").is_err());
    }
    
    #[test]
    fn test_validate_password_no_uppercase() {
        assert!(validate_password("lowercase123").is_err());
    }
    
    #[test]
    fn test_validate_password_no_digit() {
        assert!(validate_password("NoDigits").is_err());
    }
    
    #[test]
    fn test_validate_username_valid() {
        assert!(validate_username("john_doe").is_ok());
        assert!(validate_username("user123").is_ok());
    }
    
    #[test]
    fn test_validate_username_too_short() {
        assert!(validate_username("ab").is_err());
    }
    
    #[test]
    fn test_validate_username_invalid_chars() {
        assert!(validate_username("user@domain").is_err());
    }
    
    #[test]
    fn test_sanitize_string() {
        assert_eq!(sanitize_string("<script>alert('xss')</script>"), 
                   "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;&#x2F;script&gt;");
    }
}
