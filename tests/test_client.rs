use clickup_cli::client::ClickUpClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn test_client(server: &MockServer) -> ClickUpClient {
    ClickUpClient::new("pk_test_token", 30)
        .unwrap()
        .with_base_url(&server.uri())
}

#[tokio::test]
async fn test_get_sends_auth_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/user"))
        .and(header("Authorization", "pk_test_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"user": {}})))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server).await;
    let result = client.get("/v2/user").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_401_returns_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/user"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_json(serde_json::json!({"err": "Token invalid"})),
        )
        .mount(&server)
        .await;

    let client = test_client(&server).await;
    let result = client.get("/v2/user").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 2);
}

#[tokio::test]
async fn test_404_returns_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/task/bad_id"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(serde_json::json!({"err": "Task not found"})),
        )
        .mount(&server)
        .await;

    let client = test_client(&server).await;
    let result = client.get("/v2/task/bad_id").await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().exit_code(), 3);
}

#[tokio::test]
async fn test_429_returns_rate_limited() {
    let server = MockServer::start().await;
    // Return 429 on both attempts (initial + one retry)
    Mock::given(method("GET"))
        .and(path("/v2/task/123"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_json(serde_json::json!({"err": "Rate limited"})),
        )
        .mount(&server)
        .await;

    let client = test_client(&server).await;
    let result = client.get("/v2/task/123").await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().exit_code(), 4);
}

#[tokio::test]
async fn test_post_sends_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v2/list/123/task"))
        .and(header("Content-Type", "application/json"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "abc"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server).await;
    let body = serde_json::json!({"name": "Test Task"});
    let result = client.post("/v2/list/123/task", &body).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_500_returns_server_error() {
    let server = MockServer::start().await;
    // Return 500 on all retries (max_retries=3 means up to 4 attempts for 5xx)
    Mock::given(method("GET"))
        .and(path("/v2/user"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"err": "Internal"})),
        )
        .mount(&server)
        .await;

    let client = test_client(&server).await;
    let result = client.get("/v2/user").await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().exit_code(), 5);
}
