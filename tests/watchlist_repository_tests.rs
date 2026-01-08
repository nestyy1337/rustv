use backend::{
    repositories::{users::UserRepository, watchlist::WatchlistRepository},
    shared::test_utils::{setup_test_app, test_user},
};

use crate::common::fixtures::movie;

use crate::common::fixtures::insert_movie;

mod common;

#[tokio::test]
async fn test_add_watchlist_successful() {
    let (_addr, state) = setup_test_app().await.unwrap();
    let dummy_user = test_user();
    let movie = insert_movie(&state.pool).await;

    let real_user = UserRepository::find_by_username(&state.pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();

    let result = WatchlistRepository::add(&state.pool, real_user.id, movie.id).await;
    assert!(result.is_ok());
    let check = sqlx::query!(
        "SELECT id FROM watchlist WHERE user_id = ? AND movie_id = ?",
        real_user.id,
        movie.id
    )
    .fetch_one(&state.pool)
    .await;
    assert!(check.is_ok());
}

#[tokio::test]
async fn test_add_watchlist_nonexistant_movie() {
    let (_addr, state) = setup_test_app().await.unwrap();
    let dummy_user = test_user();
    let unadded_movie = movie();

    let real_user = UserRepository::find_by_username(&state.pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();

    let result = WatchlistRepository::add(&state.pool, real_user.id, unadded_movie.id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_add_watchlist_nonexistant_user() {
    let (_addr, state) = setup_test_app().await.unwrap();
    let movie = insert_movie(&state.pool).await;

    let result = WatchlistRepository::add(&state.pool, 999, movie.id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_delete_watchlisted_movie_success() {
    let (_addr, state) = setup_test_app().await.unwrap();
    let movie = insert_movie(&state.pool).await;
    let dummy_user = test_user();
    let real_user = UserRepository::find_by_username(&state.pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();

    let _ = WatchlistRepository::add(&state.pool, real_user.id, movie.id).await;

    let result = WatchlistRepository::delete(&state.pool, real_user.id, movie.id).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_delete_watchlisted_movie_nonexistant_user() {
    let (_addr, state) = setup_test_app().await.unwrap();
    let movie = insert_movie(&state.pool).await;
    let dummy_user = test_user();
    let real_user = UserRepository::find_by_username(&state.pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();

    let _ = WatchlistRepository::add(&state.pool, real_user.id, movie.id).await;

    let result = WatchlistRepository::delete(&state.pool, 999, movie.id).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_delete_watchlisted_movie_nonexistant_movie() {
    let (_addr, state) = setup_test_app().await.unwrap();
    let movie = insert_movie(&state.pool).await;
    let dummy_user = test_user();
    let real_user = UserRepository::find_by_username(&state.pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();

    let _ = WatchlistRepository::add(&state.pool, real_user.id, movie.id).await;

    let result = WatchlistRepository::delete(&state.pool, real_user.id, 999).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_is_watchlisted_anywhere_yes() {
    let (_addr, state) = setup_test_app().await.unwrap();
    let movie = insert_movie(&state.pool).await;
    let dummy_user = test_user();
    let real_user = UserRepository::find_by_username(&state.pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();

    let _ = WatchlistRepository::add(&state.pool, real_user.id, movie.id).await;

    let result = WatchlistRepository::is_watchlisted_anywhere(&state.pool, movie.id).await;
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[tokio::test]
async fn test_is_watchlisted_anywhere_yes_two() {
    let (_addr, state) = setup_test_app().await.unwrap();
    let movie = insert_movie(&state.pool).await;
    let dummy_user = test_user();
    let mut dummy_user2 = test_user();
    dummy_user2.username = "test999".to_string();
    dummy_user2.email = "test@gmail.com".to_string();

    let real_user = UserRepository::find_by_username(&state.pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();
    let real_user2 = UserRepository::add_user(&dummy_user2, "test".to_string(), &state.pool)
        .await
        .unwrap();

    let _ = WatchlistRepository::add(&state.pool, real_user.id, movie.id).await;
    let _ = WatchlistRepository::add(&state.pool, real_user2.id, movie.id).await;

    let result = WatchlistRepository::is_watchlisted_anywhere(&state.pool, movie.id).await;
    assert!(result.is_ok());
    assert!(result.unwrap());

    let _ = WatchlistRepository::delete(&state.pool, real_user2.id, movie.id).await;
    let result = WatchlistRepository::is_watchlisted_anywhere(&state.pool, movie.id).await;
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[tokio::test]
async fn test_is_watchlisted_anywhere_nope() {
    let (_addr, state) = setup_test_app().await.unwrap();
    let movie = insert_movie(&state.pool).await;
    let dummy_user = test_user();

    let _real_user = UserRepository::find_by_username(&state.pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();

    let result = WatchlistRepository::is_watchlisted_anywhere(&state.pool, movie.id).await;
    assert!(result.is_ok());
    assert!(!result.unwrap());
}
