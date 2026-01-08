use backend::shared::test_utils::setup_test_app;
use reqwest::StatusCode;
use serde_json::json;

use crate::common::{
    client::TestClient,
    fixtures::{insert_movie, movie},
};

mod common;

#[tokio::test]
async fn test_add_watchlisted_movie_success() {
    let (addr, state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;

    let result = client
        .post("/api/watchlist")
        .json(&json!({"movie_id": movie.id}))
        .send()
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_add_watchlisted_movie_non_existant_movie() {
    let (addr, _state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let _test_movie = movie();

    let result = client
        .post("/api/watchlist")
        .json(&json!({"movie_id": 992352999}))
        .send()
        .await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::BAD_REQUEST);
}

//TODO: weird
#[tokio::test]
async fn test_add_watchlisted_movie_added_twice() {
    let (addr, state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;

    let result = client
        .post("/api/watchlist")
        .json(&json!({"movie_id": movie.id}))
        .send()
        .await;

    let result2 = client
        .post("/api/watchlist")
        .json(&json!({"movie_id": movie.id}))
        .send()
        .await;

    assert!(result.is_ok());
    assert!(result2.is_ok());

    let unwrapped_result = result.unwrap();
    let unwrapped_result2 = result2.unwrap();

    assert_eq!(unwrapped_result.status(), StatusCode::OK);
    assert_eq!(unwrapped_result2.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_delete_watchlisted_movie_success() {
    let (addr, state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();
    let movie = insert_movie(&state.pool).await;
    let _ = client
        .post("/api/watchlist")
        .json(&json!({"movie_id": movie.id}))
        .send()
        .await
        .unwrap();

    let result = client
        .delete("/api/watchlist")
        .await
        .json(&json!({"movie_id": movie.id}))
        .send()
        .await;
    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_delete_watchlisted_movie_not_in_watchlist() {
    let (addr, state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();
    let movie = insert_movie(&state.pool).await;
    let result = client
        .delete("/api/watchlist")
        .await
        .json(&json!({"movie_id": movie.id}))
        .send()
        .await;
    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_delete_watchlisted_movie_non_existant_movie() {
    let (addr, _pool) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();
    let result = client
        .delete("/api/watchlist")
        .await
        .json(&json!({"movie_id": 9999}))
        .send()
        .await;
    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);
}
