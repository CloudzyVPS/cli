/// Tests for asset selection and parsing
use zy::update::{Asset, Platform, asset::{parse_asset_name, select_asset_for_platform}};

#[test]
fn test_parse_asset_name_linux_x64() {
    let result = parse_asset_name("zy-1.0.1-x86_64-unknown-linux-gnu");
    assert_eq!(
        result,
        Some(("1.0.1".to_string(), "x86_64-unknown-linux-gnu".to_string()))
    );
}

#[test]
fn test_parse_asset_name_linux_arm64() {
    let result = parse_asset_name("zy-1.0.1-aarch64-unknown-linux-gnu");
    assert_eq!(
        result,
        Some(("1.0.1".to_string(), "aarch64-unknown-linux-gnu".to_string()))
    );
}

#[test]
fn test_parse_asset_name_macos_x64() {
    let result = parse_asset_name("zy-1.0.1-x86_64-apple-darwin");
    assert_eq!(
        result,
        Some(("1.0.1".to_string(), "x86_64-apple-darwin".to_string()))
    );
}

#[test]
fn test_parse_asset_name_macos_arm64() {
    let result = parse_asset_name("zy-1.0.1-aarch64-apple-darwin");
    assert_eq!(
        result,
        Some(("1.0.1".to_string(), "aarch64-apple-darwin".to_string()))
    );
}

#[test]
fn test_parse_asset_name_windows() {
    let result = parse_asset_name("zy-1.0.1-x86_64-pc-windows-msvc.exe");
    assert_eq!(
        result,
        Some(("1.0.1".to_string(), "x86_64-pc-windows-msvc".to_string()))
    );
}

#[test]
fn test_parse_asset_name_with_beta_prerelease() {
    let result = parse_asset_name("zy-1.0.0-beta.1-x86_64-unknown-linux-gnu");
    assert_eq!(
        result,
        Some(("1.0.0-beta.1".to_string(), "x86_64-unknown-linux-gnu".to_string()))
    );
}

#[test]
fn test_parse_asset_name_with_alpha_prerelease() {
    let result = parse_asset_name("zy-2.0.0-alpha-aarch64-apple-darwin");
    assert_eq!(
        result,
        Some(("2.0.0-alpha".to_string(), "aarch64-apple-darwin".to_string()))
    );
}

#[test]
fn test_parse_asset_name_with_rc_prerelease() {
    let result = parse_asset_name("zy-1.5.0-rc.2-x86_64-pc-windows-msvc.exe");
    assert_eq!(
        result,
        Some(("1.5.0-rc.2".to_string(), "x86_64-pc-windows-msvc".to_string()))
    );
}

#[test]
fn test_parse_asset_name_invalid_no_prefix() {
    let result = parse_asset_name("other-1.0.1-x86_64-unknown-linux-gnu");
    assert_eq!(result, None);
}

#[test]
fn test_parse_asset_name_invalid_too_short() {
    let result = parse_asset_name("zy-1.0.1");
    assert_eq!(result, None);
}

#[test]
fn test_parse_asset_name_invalid_format() {
    let result = parse_asset_name("invalid-name");
    assert_eq!(result, None);
}

#[test]
fn test_parse_asset_name_checksum_file() {
    let result = parse_asset_name("SHA256SUMS.txt");
    assert_eq!(result, None);
}

#[test]
fn test_select_asset_linux_x64() {
    let assets = vec![
        Asset {
            name: "zy-1.0.1-x86_64-unknown-linux-gnu".to_string(),
            download_url: "https://example.com/linux-x64".to_string(),
            size: 1024,
            content_type: "application/octet-stream".to_string(),
        },
        Asset {
            name: "zy-1.0.1-aarch64-unknown-linux-gnu".to_string(),
            download_url: "https://example.com/linux-arm64".to_string(),
            size: 1024,
            content_type: "application/octet-stream".to_string(),
        },
        Asset {
            name: "SHA256SUMS.txt".to_string(),
            download_url: "https://example.com/checksums".to_string(),
            size: 512,
            content_type: "text/plain".to_string(),
        },
    ];

    let platform = Platform {
        target: "x86_64-unknown-linux-gnu".to_string(),
        os: "linux".to_string(),
        arch: "x86_64".to_string(),
        extension: None,
    };

    let result = select_asset_for_platform(&assets, &platform).unwrap();
    assert_eq!(result.name, "zy-1.0.1-x86_64-unknown-linux-gnu");
    assert_eq!(result.download_url, "https://example.com/linux-x64");
}

#[test]
fn test_select_asset_macos_arm64() {
    let assets = vec![
        Asset {
            name: "zy-1.0.1-x86_64-apple-darwin".to_string(),
            download_url: "https://example.com/macos-x64".to_string(),
            size: 2048,
            content_type: "application/octet-stream".to_string(),
        },
        Asset {
            name: "zy-1.0.1-aarch64-apple-darwin".to_string(),
            download_url: "https://example.com/macos-arm64".to_string(),
            size: 2048,
            content_type: "application/octet-stream".to_string(),
        },
    ];

    let platform = Platform {
        target: "aarch64-apple-darwin".to_string(),
        os: "macos".to_string(),
        arch: "aarch64".to_string(),
        extension: None,
    };

    let result = select_asset_for_platform(&assets, &platform).unwrap();
    assert_eq!(result.name, "zy-1.0.1-aarch64-apple-darwin");
    assert_eq!(result.download_url, "https://example.com/macos-arm64");
}

#[test]
fn test_select_asset_windows() {
    let assets = vec![
        Asset {
            name: "zy-1.0.1-x86_64-unknown-linux-gnu".to_string(),
            download_url: "https://example.com/linux".to_string(),
            size: 1024,
            content_type: "application/octet-stream".to_string(),
        },
        Asset {
            name: "zy-1.0.1-x86_64-pc-windows-msvc.exe".to_string(),
            download_url: "https://example.com/windows".to_string(),
            size: 2048,
            content_type: "application/octet-stream".to_string(),
        },
    ];

    let platform = Platform {
        target: "x86_64-pc-windows-msvc".to_string(),
        os: "windows".to_string(),
        arch: "x86_64".to_string(),
        extension: Some(".exe".to_string()),
    };

    let result = select_asset_for_platform(&assets, &platform).unwrap();
    assert_eq!(result.name, "zy-1.0.1-x86_64-pc-windows-msvc.exe");
    assert_eq!(result.download_url, "https://example.com/windows");
}

#[test]
fn test_select_asset_not_found() {
    let assets = vec![Asset {
        name: "zy-1.0.1-x86_64-unknown-linux-gnu".to_string(),
        download_url: "https://example.com/linux".to_string(),
        size: 1024,
        content_type: "application/octet-stream".to_string(),
    }];

    let platform = Platform {
        target: "arm-unknown-linux-gnueabihf".to_string(),
        os: "linux".to_string(),
        arch: "arm".to_string(),
        extension: None,
    };

    let result = select_asset_for_platform(&assets, &platform);
    assert!(result.is_err());
}

#[test]
fn test_select_asset_wrong_extension() {
    let assets = vec![
        Asset {
            name: "zy-1.0.1-x86_64-pc-windows-msvc".to_string(), // Missing .exe
            download_url: "https://example.com/windows-no-ext".to_string(),
            size: 2048,
            content_type: "application/octet-stream".to_string(),
        },
        Asset {
            name: "zy-1.0.1-x86_64-pc-windows-msvc.exe".to_string(),
            download_url: "https://example.com/windows".to_string(),
            size: 2048,
            content_type: "application/octet-stream".to_string(),
        },
    ];

    let platform = Platform {
        target: "x86_64-pc-windows-msvc".to_string(),
        os: "windows".to_string(),
        arch: "x86_64".to_string(),
        extension: Some(".exe".to_string()),
    };

    // Should select the one with .exe
    let result = select_asset_for_platform(&assets, &platform).unwrap();
    assert_eq!(result.name, "zy-1.0.1-x86_64-pc-windows-msvc.exe");
}

#[test]
fn test_select_asset_skips_checksums() {
    let assets = vec![
        Asset {
            name: "SHA256SUMS.txt".to_string(),
            download_url: "https://example.com/checksums".to_string(),
            size: 512,
            content_type: "text/plain".to_string(),
        },
        Asset {
            name: "zy-1.0.1-x86_64-unknown-linux-gnu".to_string(),
            download_url: "https://example.com/linux".to_string(),
            size: 1024,
            content_type: "application/octet-stream".to_string(),
        },
    ];

    let platform = Platform {
        target: "x86_64-unknown-linux-gnu".to_string(),
        os: "linux".to_string(),
        arch: "x86_64".to_string(),
        extension: None,
    };

    let result = select_asset_for_platform(&assets, &platform).unwrap();
    assert_eq!(result.name, "zy-1.0.1-x86_64-unknown-linux-gnu");
}
