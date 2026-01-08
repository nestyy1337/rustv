mod common;

use backend::{
    models::movie::Movie,
    repositories::{users::UserRepository, watchlist::WatchlistRepository},
    shared::test_utils::{setup_test_app, test_user},
};
use reqwest::StatusCode;
use serde_json::json;

use crate::common::{
    client::TestClient,
    fixtures::{insert_movie, insert_movie_custom, insert_watched_movie},
};

#[tokio::test]
async fn test_get_movie_details_404() {
    let (addr, _state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let result = client.get(format!("/api/movie/{}", 1).as_str()).await;
    dbg!(&result);
    assert!(result.is_ok());
    let unwraped_result = result.unwrap();
    assert_eq!(unwraped_result.status(), StatusCode::NOT_FOUND)
}

#[tokio::test]
async fn test_get_movie_details_found() {
    let (addr, state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;

    let result = client
        .get(format!("/api/movie/{}", movie.id).as_str())
        .await;
    assert!(result.is_ok());
    let unwraped_result = result.unwrap();
    assert_eq!(unwraped_result.status(), StatusCode::OK);
    let json: Movie = serde_json::from_str(&unwraped_result.text().await.unwrap()).unwrap();
    assert_eq!(json, movie);
}

#[tokio::test]
async fn test_search_movie_not_found() {
    let (addr, _state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let result = client
        .get(format!("/api/movie/search/{}", "shaw").as_str())
        .await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);

    let searched_movies: Vec<Movie> =
        serde_json::from_str(&unwrapped_result.text().await.unwrap()).unwrap();
    assert_eq!(searched_movies, vec![]);
}

#[tokio::test]
async fn test_search_movie_found() {
    let (addr, state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;

    let result = client
        .get(format!("/api/movie/search/{}", &movie.title).as_str())
        .await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);

    let searched_movies: Vec<Movie> =
        serde_json::from_str(&unwrapped_result.text().await.unwrap()).unwrap();
    assert_eq!(searched_movies, vec![movie]);
}

#[tokio::test]
async fn test_search_movie_found_cases_regex() {
    let (addr, state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;

    let result = client
        .get(format!("/api/movie/search/{}", &movie.title.to_lowercase()).as_str())
        .await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);

    let searched_movies: Vec<Movie> =
        serde_json::from_str(&unwrapped_result.text().await.unwrap()).unwrap();
    assert_eq!(searched_movies, vec![movie.clone()]);

    let result = client
        .get(format!("/api/movie/search/{}", &movie.title.to_uppercase()).as_str())
        .await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);

    let searched_movies: Vec<Movie> =
        serde_json::from_str(&unwrapped_result.text().await.unwrap()).unwrap();
    assert_eq!(searched_movies, vec![movie.clone()]);

    let result = client
        .get(&format!("/api/movie/search/{}", &movie.title.split_at(2).1))
        .await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);

    let searched_movies: Vec<Movie> =
        serde_json::from_str(&unwrapped_result.text().await.unwrap()).unwrap();
    assert_eq!(searched_movies, vec![movie.clone()]);
}

#[tokio::test]
async fn test_search_default_empty_found() {
    let (addr, state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;
    let movie2 = insert_movie_custom(&state.pool, "tt003").await;

    let result = client.get("/api/movie/search/").await;
    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();

    let searched_movies: Vec<Movie> =
        serde_json::from_str(&unwrapped_result.text().await.unwrap()).unwrap();
    assert_eq!(searched_movies, vec![movie.clone(), movie2.clone()]);
}

#[tokio::test]
async fn test_search_default_empty_non_existant_movies() {
    let (addr, _state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    // let movie = insert_movie(&pool).await;
    // let movie2 = insert_movie_custom(&pool, "tt003").await;

    let result = client.get("/api/movie/search/").await;
    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();

    let searched_movies: Vec<Movie> =
        serde_json::from_str(&unwrapped_result.text().await.unwrap()).unwrap();
    assert_eq!(searched_movies, vec![]);
}

#[tokio::test]
async fn test_delete_watchlisted_movie_success() {
    let (addr, state) = setup_test_app().await.unwrap();
    let user = test_user();
    let real_user = UserRepository::find_by_username(&state.pool, &user.username)
        .await
        .unwrap()
        .unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;
    WatchlistRepository::add(&state.pool, real_user.id, movie.id)
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
async fn test_delete_watchlisted_movie_non_existant_movie() {
    let (addr, state) = setup_test_app().await.unwrap();
    let user = test_user();
    let _real_user = UserRepository::find_by_username(&state.pool, &user.username)
        .await
        .unwrap()
        .unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let result = client
        .delete("/api/watchlist")
        .await
        .json(&json!({"movie_id": 999}))
        .send()
        .await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_delete_watchlisted_movie_for_other_user() {
    let (addr, state) = setup_test_app().await.unwrap();
    let mut user = test_user();
    user.username = "test999".to_string();
    user.email = "test999@gmail.com".to_string();
    let _user = UserRepository::add_user(&user, "test".to_string(), &state.pool)
        .await
        .unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let result = client
        .delete("/api/watchlist")
        .await
        .json(&json!({"movie_id": 999}))
        .send()
        .await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_delete_watchlisted_movie_concurrent() {
    use futures::future::join_all;

    let (addr, state) = setup_test_app().await.unwrap();
    let user = test_user();
    let real_user = UserRepository::find_by_username(&state.pool, &user.username)
        .await
        .unwrap()
        .unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;
    WatchlistRepository::add(&state.pool, real_user.id, movie.id)
        .await
        .unwrap();

    let mut handles = vec![];
    for _ in 0..5 {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            client
                .delete("/api/watchlist")
                .await
                .json(&json!({"movie_id": movie.id}))
                .send()
                .await
        });
        handles.push(handle);
    }
    let results: Vec<_> = join_all(handles)
        .await
        .into_iter()
        .map(|h| h.unwrap())
        .collect();
    assert!(results.iter().all(|r| r.is_ok()))
}

#[tokio::test]
async fn test_get_watched_movies_page_success() {
    let (addr, state) = setup_test_app().await.unwrap();
    let user = test_user();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;
    insert_watched_movie(&state.pool, user.id, movie.id).await;

    let result = client.get(&format!("/watched/{}", user.username)).await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_get_watched_movies_page_unauthorized() {
    let (addr, _state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);

    let result = client.get("/watched/someuser").await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    println!("PROBLEM: {:?}", unwrapped_result);
    assert_eq!(unwrapped_result.status(), StatusCode::OK);
    assert_eq!(
        format!("{}/login?next=%2Fwatched%2Fsomeuser", &addr),
        unwrapped_result.url().to_string()
    );
}

#[tokio::test]
async fn test_get_watched_movies_page_401() {
    let (addr, _state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let result = client.get("/watched/differentuser").await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_delete_watched_movie_success() {
    let (addr, state) = setup_test_app().await.unwrap();
    let user = test_user();
    let _real_user = UserRepository::find_by_username(&state.pool, &user.username)
        .await
        .unwrap()
        .unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;
    insert_watched_movie(&state.pool, user.id, movie.id).await;

    let result = client
        .delete(&format!("/api/watched/{}/{}", user.username, movie.id))
        .await
        .send()
        .await;
    println!("Result: {:?}", result);

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_delete_watched_movie_404() {
    let (addr, state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;

    let result = client
        .delete(&format!("/api/watched/differentuser/{}", movie.id))
        .await
        .send()
        .await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_add_watched_movie_success() {
    let (addr, state) = setup_test_app().await.unwrap();
    let user = test_user();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;

    let result = client
        .post(&format!("/api/watched/{}/{}", user.username, movie.id))
        .send()
        .await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_add_watched_movie_404() {
    let (addr, state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;

    let result = client
        .post(&format!("/api/watched/differentuser/{}", movie.id))
        .send()
        .await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_profile_page_success() {
    let (addr, _state) = setup_test_app().await.unwrap();
    let user = test_user();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let result = client.get(&format!("/profile/{}", user.username)).await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_get_profile_page_unauthorized() {
    let (addr, _state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);

    let result = client.get("/profile/someuser").await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    println!("PROBLEM: {:?}", unwrapped_result);
    assert_eq!(unwrapped_result.status(), StatusCode::OK);
    assert_eq!(
        unwrapped_result.url().to_string(),
        format!("{}/login?next=%2Fprofile%2Fsomeuser", &addr)
    )
}

#[tokio::test]
async fn test_get_watchlist_page_success() {
    let (addr, _state) = setup_test_app().await.unwrap();
    let user = test_user();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let result = client.get(&format!("/watchlist/{}", user.username)).await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_add_watchlist_movie_success() {
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
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_get_poster_from_file() {
    let (addr, state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;

    let poster_path = format!("movies/{}_poster.jpg", movie.id);
    tokio::fs::write(&poster_path, b"fake_image_data")
        .await
        .ok();

    let result = client.get(&format!("/api/movie/{}/poster", movie.id)).await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);

    tokio::fs::remove_file(&poster_path).await.ok();
}

#[tokio::test]
async fn test_get_poster_movie_not_found() {
    let (addr, _state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let result = client.get("/api/movie/999/poster").await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_request_movie_unauthorized() {
    let (addr, _state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);

    let result = client
        .post("/api/movie/request")
        .json(&json!({"movie_id": 1}))
        .send()
        .await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_delete_requested_movie_success() {
    let (addr, state) = setup_test_app().await.unwrap();
    let user = test_user();
    let real_user = UserRepository::find_by_username(&state.pool, &user.username)
        .await
        .unwrap()
        .unwrap();
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();

    let movie = insert_movie(&state.pool).await;
    WatchlistRepository::add(&state.pool, real_user.id, movie.id)
        .await
        .unwrap();

    let result = client
        .delete("/api/movie/request")
        .await
        .json(&json!({"movie_id": movie.id}))
        .send()
        .await;

    assert!(result.is_ok());
    let unwrapped_result = result.unwrap();
    assert_eq!(unwrapped_result.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_search_tmdb_by_title() {
    let (addr, _state) = setup_test_app().await.unwrap();
    let client = TestClient::new(&addr);

    let result = client.get("/api/movie/tmdb/search/Inception").await;

    assert!(result.is_ok());
}
