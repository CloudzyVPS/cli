use zy::update::GitHubClient;

#[test]
fn test_client_creation() {
    let client = GitHubClient::new("CloudzyVPS".to_string(), "cli".to_string());
    assert_eq!(client.repo_owner, "CloudzyVPS");
    assert_eq!(client.repo_name, "cli");
}
