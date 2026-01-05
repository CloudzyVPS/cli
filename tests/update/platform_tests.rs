/// Tests for platform detection
use zy::update::Platform;

#[test]
fn test_current_platform() {
    let platform = Platform::current();
    
    // Should detect something
    assert!(!platform.target.is_empty());
    assert!(!platform.os.is_empty());
    assert!(!platform.arch.is_empty());
    
    println!("Detected platform: {:?}", platform);
}

#[test]
fn test_platform_extension_windows() {
    // We can't guarantee we're on Windows, but we can test the logic
    let platform = Platform::current();
    
    if platform.os == "windows" {
        assert_eq!(platform.extension, Some(".exe".to_string()));
    } else {
        assert_eq!(platform.extension, None);
    }
}

#[test]
fn test_to_target_triple() {
    let platform = Platform::current();
    let triple = platform.to_target_triple();
    
    assert!(!triple.is_empty());
    assert_eq!(triple, platform.target);
}

#[test]
fn test_supported_platforms() {
    // Test all 5 officially supported platforms
    let test_cases = vec![
        (
            "x86_64-unknown-linux-gnu",
            "linux",
            "x86_64",
            None,
        ),
        (
            "aarch64-unknown-linux-gnu",
            "linux",
            "aarch64",
            None,
        ),
        (
            "x86_64-apple-darwin",
            "macos",
            "x86_64",
            None,
        ),
        (
            "aarch64-apple-darwin",
            "macos",
            "aarch64",
            None,
        ),
        (
            "x86_64-pc-windows-msvc",
            "windows",
            "x86_64",
            Some(".exe".to_string()),
        ),
    ];
    
    for (target, os, arch, extension) in test_cases {
        let platform = Platform {
            target: target.to_string(),
            os: os.to_string(),
            arch: arch.to_string(),
            extension,
        };
        
        assert_eq!(platform.target, target);
        assert_eq!(platform.os, os);
        assert_eq!(platform.arch, arch);
        
        // All 5 platforms should be supported
        assert!(platform.is_supported().is_ok());
    }
}

#[test]
fn test_unsupported_platform() {
    let unsupported = Platform {
        target: "arm-unknown-linux-gnueabihf".to_string(),
        os: "linux".to_string(),
        arch: "arm".to_string(),
        extension: None,
    };
    
    let result = unsupported.is_supported();
    assert!(result.is_err());
}

#[test]
fn test_current_platform_is_supported() {
    let platform = Platform::current();
    
    // The current platform should be supported if we're running on one of the 5 targets
    let result = platform.is_supported();
    
    // Log the result for debugging
    match &result {
        Ok(_) => println!("Current platform {} is supported", platform.target),
        Err(e) => println!("Current platform {} is not supported: {}", platform.target, e),
    }
    
    // We can't assert this will always succeed since we might be running on an unsupported platform
    // But we can verify the error type is correct
    if result.is_err() {
        use zy::update::UpdateError;
        match result.unwrap_err() {
            UpdateError::UnsupportedPlatform(_) => {
                // Expected error type
            }
            _ => panic!("Expected UnsupportedPlatform error"),
        }
    }
}

#[test]
fn test_platform_target_mapping() {
    let platform = Platform::current();
    
    // Verify that the detected target matches expected patterns
    let supported_targets = [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ];
    
    if supported_targets.contains(&platform.target.as_str()) {
        println!("Running on supported platform: {}", platform.target);
        
        // Verify OS and arch are consistent with target
        if platform.target.contains("linux") {
            assert_eq!(platform.os, "linux");
        } else if platform.target.contains("darwin") {
            assert_eq!(platform.os, "macos");
        } else if platform.target.contains("windows") {
            assert_eq!(platform.os, "windows");
        }
        
        if platform.target.starts_with("x86_64") {
            assert_eq!(platform.arch, "x86_64");
        } else if platform.target.starts_with("aarch64") {
            assert_eq!(platform.arch, "aarch64");
        }
    }
}
