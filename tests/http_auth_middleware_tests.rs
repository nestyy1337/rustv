mod common;

use std::{collections::HashMap, sync::Arc};

use backend::shared::test_utils::setup_test_app;
use chrono::{Duration, Utc};
use reqwest::{
    Client, StatusCode, Url,
    cookie::{CookieStore, Jar},
};

#[tokio::test]
async fn test1() {
    let (address, _) = setup_test_app().await.expect("Failed to set up test app");

    let cookie_jar = Arc::new(Jar::default());
    let client = Client::builder()
        .cookie_provider(cookie_jar.clone())
        .build()
        .unwrap();

    let res = client.get(url("/", &address)).send().await.unwrap();
    assert_eq!(
        *res.url(),
        url("/login?next=%2F", &address),
        "Expected redirect to /login after accessing root without authentication"
    );
    assert_eq!(res.status(), StatusCode::OK);

    let res = login(&client, "ferris", "hunter42", &address).await;
    assert_eq!(
        htmx_redirect_header(&res).unwrap_or(""),
        "/",
        "Expected redirect to / after successful login"
    );
    assert_eq!(res.status(), StatusCode::OK);

    let cookies = cookie_jar
        .cookies(&url("/", &address))
        .expect("A cookie should be set");
    assert!(
        cookies.to_str().unwrap_or("").contains("id="),
        "Expected 'id' cookie to be set after successful login"
    );

    let res = client.post(url("/logout", &address)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        cookie_jar.cookies(&url("/", &address)).iter().len(),
        1,
        "Expected 'id' cookie to be removed"
    );
}

#[tokio::test]
async fn logout_is_idempotent() {
    let (address, _) = setup_test_app().await.expect("Failed to set up test app");

    let cookie_jar = Arc::new(Jar::default());
    let client = Client::builder()
        .cookie_provider(cookie_jar.clone())
        .build()
        .unwrap();
    login(&client, "ferris", "hunter42", &address).await;

    let res1 = client.post(url("/logout", &address)).send().await.unwrap();
    let res2 = client.post(url("/logout", &address)).send().await.unwrap();

    // Both should succeed without errors
    assert_eq!(res1.status(), StatusCode::OK);
    assert_eq!(res2.status(), StatusCode::OK);
}

#[tokio::test]
async fn authenticated_user_can_access_protected_routes() {
    let (addr, _) = setup_test_app().await.unwrap();
    let cookie_jar = Arc::new(Jar::default());
    let client = Client::builder()
        .cookie_provider(cookie_jar.clone())
        .build()
        .unwrap();
    login(&client, "ferris", "hunter42", &addr).await;
    let result = client.get(url("/protected", &addr)).send().await;
    assert!(result.is_ok());
    let unwraped_result = result.unwrap();
    assert_eq!(unwraped_result.status(), StatusCode::OK);
    assert_ne!(*unwraped_result.url(), url("/login", &addr));
}

#[tokio::test]
async fn session_persists_through_requests() {
    let (addr, _) = setup_test_app().await.unwrap();
    let cookie_jar = Arc::new(Jar::default());
    let client = Client::builder()
        .cookie_provider(cookie_jar.clone())
        .build()
        .unwrap();
    login(&client, "ferris", "hunter42", &addr).await;
    let result = client.get(url("/protected", &addr)).send().await;
    assert!(result.is_ok());
    let unwraped_result = result.unwrap();
    assert_eq!(unwraped_result.status(), StatusCode::OK);
    assert_ne!(*unwraped_result.url(), url("/login", &addr));

    let result2 = client.get(url("/protected", &addr)).send().await;
    assert!(result2.is_ok());
    let unwraped_result = result2.unwrap();
    assert_eq!(unwraped_result.status(), StatusCode::OK);
    assert_ne!(*unwraped_result.url(), url("/login", &addr));
}

#[tokio::test]
async fn invalid_session_fails_auth() {
    let (addr, _) = setup_test_app().await.unwrap();
    let cookie_jar = Arc::new(Jar::default());
    let client = Client::builder()
        .cookie_provider(cookie_jar.clone())
        .build()
        .unwrap();

    cookie_jar.add_cookie_str("id=invalid_cookie", &url("/", &addr));

    let result = client.get(url("/protected", &addr)).send().await.unwrap();
    assert_eq!(result.url().path(), "/login");
}
#[tokio::test]
async fn concurrent_requests_with_same_session_work() {
    let (address, _) = setup_test_app().await.unwrap();
    let cookie_jar = Arc::new(Jar::default());
    let client = Client::builder()
        .cookie_provider(cookie_jar.clone())
        .build()
        .unwrap();

    login(&client, "ferris", "hunter42", &address).await;

    let mut handles = vec![];
    for _ in 0..5 {
        let client = client.clone();
        let addr = address.clone();
        handles.push(tokio::spawn(async move {
            client.get(url("/protected", &addr)).send().await
        }));
    }

    for handle in handles {
        let res = handle.await.unwrap().unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn expires_inactive_sessions() {
    let (address, state) = setup_test_app().await.expect("Failed to set up test app");

    let cookie_jar = Arc::new(Jar::default());
    let client = Client::builder()
        .cookie_provider(cookie_jar.clone())
        .build()
        .unwrap();

    let _ = login(&client, "ferris", "hunter42", &address).await;

    let cookies = cookie_jar
        .cookies(&url("/", &address))
        .expect("A cookie should be set")
        .to_str()
        .expect("Cookie should be valid")
        .to_string();

    let id = cookies
        .split(';')
        .map(|s| s.trim())
        .find(|s| s.starts_with("id="))
        .and_then(|s| s.strip_prefix("id="))
        .expect("Expected 'id' cookie to be set")
        .to_string();

    let _ = sqlx::query("UPDATE tower_sessions SET expiry_date = ? WHERE id = ?")
        .bind(Utc::now().timestamp() - Duration::minutes(10).num_seconds())
        .bind(&id)
        .execute(&state.pool)
        .await
        .unwrap();

    let res = client
        .get(url("/protected", &address))
        .send()
        .await
        .unwrap();

    assert_eq!(*res.url(), url("/login?next=%2Fprotected", &address));
}

#[tokio::test]
async fn login_creates_session_with_new_id() {
    let (address, _) = setup_test_app().await.unwrap();
    let cookie_jar = Arc::new(Jar::default());
    let client = Client::builder()
        .cookie_provider(cookie_jar.clone())
        .build()
        .unwrap();

    client.get(url("/", &address)).send().await.unwrap();
    login(&client, "ferris", "hunter42", &address).await;

    let session_id = extract_session_id(&cookie_jar, &address);
    assert!(!session_id.is_empty(), "Login should create session");
}

fn url(path: &str, base_address: &str) -> Url {
    let formatted_url = if path.starts_with('/') {
        format!("{base_address}{path}")
    } else {
        format!("{base_address}/{path}")
    };
    formatted_url.parse().unwrap()
}

fn htmx_redirect_header(res: &reqwest::Response) -> Option<&str> {
    res.headers()
        .get("HX-REDIRECT")
        .and_then(|value| value.to_str().ok())
}

async fn login(
    client: &Client,
    username: &str,
    password: &str,
    base_address: &str,
) -> reqwest::Response {
    let mut form = HashMap::new();
    form.insert("username", username);
    form.insert("password", password);
    client
        .post(url("/login", base_address))
        .form(&form)
        .send()
        .await
        .unwrap()
}

fn extract_session_id(cookie_jar: &Arc<Jar>, address: &str) -> String {
    cookie_jar
        .cookies(&url("/", address))
        .expect("Cookie should exist")
        .to_str()
        .expect("Cookie should be valid")
        .split('=')
        .nth(1)
        .expect("Should have value")
        .split(';')
        .next()
        .expect("Should have ID")
        .to_string()
}
