/// Tests for version parsing and comparison
use zy::update::Version;

#[test]
fn test_parse_simple_version() {
    let v = Version::parse("1.0.0").unwrap();
    assert_eq!(v.major, 1);
    assert_eq!(v.minor, 0);
    assert_eq!(v.patch, 0);
    assert_eq!(v.pre_release, None);
}

#[test]
fn test_parse_with_v_prefix() {
    let v = Version::parse("v2.1.3").unwrap();
    assert_eq!(v.major, 2);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 3);
    assert_eq!(v.pre_release, None);
}

#[test]
fn test_parse_with_alpha_prerelease() {
    let v = Version::parse("1.0.0-alpha").unwrap();
    assert_eq!(v.major, 1);
    assert_eq!(v.minor, 0);
    assert_eq!(v.patch, 0);
    assert_eq!(v.pre_release, Some("alpha".to_string()));
}

#[test]
fn test_parse_with_beta_prerelease() {
    let v = Version::parse("1.0.0-beta.1").unwrap();
    assert_eq!(v.major, 1);
    assert_eq!(v.minor, 0);
    assert_eq!(v.patch, 0);
    assert_eq!(v.pre_release, Some("beta.1".to_string()));
}

#[test]
fn test_parse_with_rc_prerelease() {
    let v = Version::parse("1.2.3-rc.1").unwrap();
    assert_eq!(v.major, 1);
    assert_eq!(v.minor, 2);
    assert_eq!(v.patch, 3);
    assert_eq!(v.pre_release, Some("rc.1".to_string()));
}

#[test]
fn test_parse_with_v_prefix_and_prerelease() {
    let v = Version::parse("v1.2.3-rc.1").unwrap();
    assert_eq!(v.major, 1);
    assert_eq!(v.minor, 2);
    assert_eq!(v.patch, 3);
    assert_eq!(v.pre_release, Some("rc.1".to_string()));
}

#[test]
fn test_parse_invalid_too_few_components() {
    let result = Version::parse("1.0");
    assert!(result.is_err());
}

#[test]
fn test_parse_invalid_too_many_components() {
    let result = Version::parse("1.0.0.0");
    assert!(result.is_err());
}

#[test]
fn test_parse_invalid_non_numeric() {
    let result = Version::parse("a.b.c");
    assert!(result.is_err());
}

#[test]
fn test_current_version() {
    let current = Version::current();
    // Should not panic and should be valid
    assert!(current.major >= 0);
    assert!(current.minor >= 0);
    assert!(current.patch >= 0);
}

#[test]
fn test_is_newer_than_major() {
    let v1 = Version::parse("1.0.0").unwrap();
    let v2 = Version::parse("2.0.0").unwrap();
    
    assert!(v2.is_newer_than(&v1));
    assert!(!v1.is_newer_than(&v2));
}

#[test]
fn test_is_newer_than_minor() {
    let v1 = Version::parse("1.0.0").unwrap();
    let v2 = Version::parse("1.1.0").unwrap();
    
    assert!(v2.is_newer_than(&v1));
    assert!(!v1.is_newer_than(&v2));
}

#[test]
fn test_is_newer_than_patch() {
    let v1 = Version::parse("1.0.0").unwrap();
    let v2 = Version::parse("1.0.1").unwrap();
    
    assert!(v2.is_newer_than(&v1));
    assert!(!v1.is_newer_than(&v2));
}

#[test]
fn test_is_newer_than_prerelease_vs_stable() {
    let stable = Version::parse("1.0.0").unwrap();
    let beta = Version::parse("1.0.0-beta").unwrap();
    
    // Stable is newer than pre-release
    assert!(stable.is_newer_than(&beta));
    assert!(!beta.is_newer_than(&stable));
}

#[test]
fn test_is_newer_than_equal() {
    let v1 = Version::parse("1.0.0").unwrap();
    let v2 = Version::parse("1.0.0").unwrap();
    
    assert!(!v1.is_newer_than(&v2));
    assert!(!v2.is_newer_than(&v1));
}

#[test]
fn test_is_newer_than_complex() {
    let v1_0_0 = Version::parse("1.0.0").unwrap();
    let v1_0_1 = Version::parse("1.0.1").unwrap();
    let v1_2_0 = Version::parse("1.2.0").unwrap();
    let v2_0_0 = Version::parse("2.0.0").unwrap();
    
    assert!(v1_0_1.is_newer_than(&v1_0_0));
    assert!(v1_2_0.is_newer_than(&v1_0_1));
    assert!(v2_0_0.is_newer_than(&v1_2_0));
    
    assert!(!v1_0_0.is_newer_than(&v1_0_1));
    assert!(!v1_0_1.is_newer_than(&v1_2_0));
    assert!(!v1_2_0.is_newer_than(&v2_0_0));
}

#[test]
fn test_version_display() {
    let v1 = Version::parse("1.0.0").unwrap();
    assert_eq!(v1.to_string(), "1.0.0");
    
    let v2 = Version::parse("1.0.0-beta.1").unwrap();
    assert_eq!(v2.to_string(), "1.0.0-beta.1");
    
    let v3 = Version::parse("v2.1.3-rc.2").unwrap();
    assert_eq!(v3.to_string(), "2.1.3-rc.2");
}
