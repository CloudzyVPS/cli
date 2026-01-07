//! Checksum verification functionality

use super::error::UpdateError;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

/// Parse a SHA256SUMS.txt file
///
/// Expected format: `<hash>  <filename>` (two spaces between hash and filename)
/// or `<hash> <filename>` (one space, also acceptable)
///
/// # Arguments
///
/// * `content` - The content of the SHA256SUMS.txt file
///
/// # Returns
///
/// A HashMap mapping filename to expected SHA256 hash
///
/// # Examples
///
/// ```
/// use zy::update::checksum::parse_checksums;
///
/// let content = "abc123def456  zy-1.0.0-x86_64-unknown-linux-gnu\n";
/// let checksums = parse_checksums(content).unwrap();
/// assert_eq!(checksums.get("zy-1.0.0-x86_64-unknown-linux-gnu"), Some(&"abc123def456".to_string()));
/// ```
pub fn parse_checksums(content: &str) -> Result<HashMap<String, String>, UpdateError> {
    let mut checksums = HashMap::new();
    
    for line in content.lines() {
        let line = line.trim();
        
        // Skip empty lines
        if line.is_empty() {
            continue;
        }
        
        // Split by whitespace (handles both single and double space)
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        if parts.len() < 2 {
            tracing::warn!("Skipping invalid checksum line: {}", line);
            continue;
        }
        
        let hash = parts[0].to_lowercase();
        let filename = parts[1..].join(" "); // In case filename has spaces
        
        // Validate hash format (SHA256 is 64 hex characters)
        if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            tracing::warn!("Skipping invalid hash format: {}", hash);
            continue;
        }
        
        checksums.insert(filename, hash);
    }
    
    Ok(checksums)
}

/// Calculate the SHA256 hash of a file
///
/// # Arguments
///
/// * `path` - Path to the file to hash
///
/// # Returns
///
/// The SHA256 hash as a lowercase hex string
///
/// # Errors
///
/// Returns an error if the file cannot be read
///
/// # Examples
///
/// ```no_run
/// use zy::update::checksum::calculate_file_hash;
/// use std::path::Path;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let hash = calculate_file_hash(Path::new("/path/to/binary")).await?;
/// println!("Hash: {}", hash);
/// # Ok(())
/// # }
/// ```
pub async fn calculate_file_hash(path: &Path) -> Result<String, UpdateError> {
    tracing::info!("Calculating SHA256 hash for: {:?}", path);
    
    let content = tokio::fs::read(path).await?;
    
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let result = hasher.finalize();
    
    let hash = hex::encode(result);
    
    tracing::debug!("Calculated hash: {}", hash);
    
    Ok(hash)
}

/// Verify that a file's hash matches the expected hash
///
/// # Arguments
///
/// * `path` - Path to the file to verify
/// * `expected_hash` - The expected SHA256 hash (hex string)
///
/// # Returns
///
/// `Ok(())` if the hash matches, `Err(UpdateError::ChecksumMismatch)` if not
///
/// # Examples
///
/// ```no_run
/// use zy::update::checksum::verify_file_hash;
/// use std::path::Path;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// verify_file_hash(
///     Path::new("/path/to/binary"),
///     "abc123def456..."
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn verify_file_hash(path: &Path, expected_hash: &str) -> Result<(), UpdateError> {
    let actual_hash = calculate_file_hash(path).await?;
    let expected_hash = expected_hash.to_lowercase();
    
    if actual_hash == expected_hash {
        tracing::info!("Checksum verification successful");
        Ok(())
    } else {
        tracing::error!(
            "Checksum mismatch: expected {}, got {}",
            expected_hash,
            actual_hash
        );
        Err(UpdateError::ChecksumMismatch {
            expected: expected_hash,
            actual: actual_hash,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_checksums_double_space() {
        // Use valid 64-character hex hashes
        let content = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  file1.txt\nd7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592  file2.exe\n";
        let checksums = parse_checksums(content).unwrap();
        
        assert_eq!(checksums.len(), 2);
        assert_eq!(checksums.get("file1.txt"), Some(&"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string()));
        assert_eq!(checksums.get("file2.exe"), Some(&"d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592".to_string()));
    }
    
    #[test]
    fn test_parse_checksums_single_space() {
        let content = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 file1.txt\n";
        let checksums = parse_checksums(content).unwrap();
        
        assert_eq!(checksums.len(), 1);
        assert_eq!(checksums.get("file1.txt"), Some(&"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string()));
    }
    
    #[test]
    fn test_parse_checksums_empty_lines() {
        let content = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  file1.txt\n\nd7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592  file2.txt\n\n";
        let checksums = parse_checksums(content).unwrap();
        
        assert_eq!(checksums.len(), 2);
    }
    
    #[test]
    fn test_parse_checksums_invalid_hash() {
        // Hash too short
        let content = "abc123  file1.txt\n";
        let checksums = parse_checksums(content).unwrap();
        assert_eq!(checksums.len(), 0);
        
        // Non-hex characters
        let content2 = "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg  file1.txt\n";
        let checksums2 = parse_checksums(content2).unwrap();
        assert_eq!(checksums2.len(), 0);
    }
    
    #[test]
    fn test_parse_checksums_real_format() {
        // Real format from GitHub releases
        let content = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  zy-1.0.0-x86_64-unknown-linux-gnu\n";
        let checksums = parse_checksums(content).unwrap();
        
        assert_eq!(checksums.len(), 1);
        assert_eq!(
            checksums.get("zy-1.0.0-x86_64-unknown-linux-gnu"),
            Some(&"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string())
        );
    }
    
    #[tokio::test]
    async fn test_calculate_file_hash() {
        // Create a temporary file with known content
        use std::io::Write;
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(b"Hello, world!").unwrap();
        drop(file);
        
        let hash = calculate_file_hash(&file_path).await.unwrap();
        
        // SHA256 of "Hello, world!" is known
        assert_eq!(hash, "315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3");
    }
    
    #[tokio::test]
    async fn test_verify_file_hash_success() {
        // Create a temporary file
        use std::io::Write;
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(b"Hello, world!").unwrap();
        drop(file);
        
        // Verify with correct hash
        let result = verify_file_hash(
            &file_path,
            "315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3"
        ).await;
        
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_verify_file_hash_failure() {
        // Create a temporary file
        use std::io::Write;
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(b"Hello, world!").unwrap();
        drop(file);
        
        // Verify with incorrect hash
        let result = verify_file_hash(
            &file_path,
            "0000000000000000000000000000000000000000000000000000000000000000"
        ).await;
        
        assert!(result.is_err());
        match result {
            Err(UpdateError::ChecksumMismatch { .. }) => {},
            _ => panic!("Expected ChecksumMismatch error"),
        }
    }
}
