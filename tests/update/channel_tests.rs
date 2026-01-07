use zy::update::Channel;

#[test]
fn test_from_version_stable() {
    assert_eq!(Channel::from_version("1.0.0"), Channel::Stable);
    assert_eq!(Channel::from_version("v1.0.0"), Channel::Stable);
    assert_eq!(Channel::from_version("2.1.3"), Channel::Stable);
}

#[test]
fn test_from_version_alpha() {
    assert_eq!(Channel::from_version("1.0.0-alpha"), Channel::Alpha);
    assert_eq!(Channel::from_version("v1.0.0-alpha.1"), Channel::Alpha);
    assert_eq!(Channel::from_version("1.0.0-ALPHA"), Channel::Alpha);
}

#[test]
fn test_from_version_beta() {
    assert_eq!(Channel::from_version("1.0.0-beta"), Channel::Beta);
    assert_eq!(Channel::from_version("v1.0.0-beta.2"), Channel::Beta);
    assert_eq!(Channel::from_version("1.0.0-BETA"), Channel::Beta);
}

#[test]
fn test_from_version_rc() {
    assert_eq!(Channel::from_version("1.0.0-rc"), Channel::ReleaseCandidate);
    assert_eq!(Channel::from_version("v1.0.0-rc.1"), Channel::ReleaseCandidate);
    assert_eq!(Channel::from_version("1.0.0-RC"), Channel::ReleaseCandidate);
}

#[test]
fn test_should_include_prerelease() {
    assert!(!Channel::Stable.should_include_prerelease());
    assert!(Channel::Alpha.should_include_prerelease());
    assert!(Channel::Beta.should_include_prerelease());
    assert!(Channel::ReleaseCandidate.should_include_prerelease());
}

