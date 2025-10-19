use crate::common::fixtures::{insert_movie, insert_movie_custom};
use backend::handlers::watchlist;
use backend::models::movie::Movie;
use backend::repositories::users::UserRepository;
use backend::repositories::watchlist::WatchlistRepository;
use backend::shared::error::Error;
use backend::shared::test_utils::test_user;
use backend::{services::watchlist::WatchlistService, shared::test_utils::setup_test_app};
use chrono::format::OffsetFormat;
use time::OffsetDateTime;

mod common;

#[tokio::test]
async fn get_empty_watchlist_succeeds() {
    let (_addr, pool) = setup_test_app().await.unwrap();
    let dummy_user = test_user();
    let real_user = UserRepository::find_by_username(&pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();

    let result = WatchlistService::get_user_watchlist(&pool, &real_user.username).await;

    assert!(result.is_ok());
    let (_profile, watchlist) = result.unwrap();
    assert!(watchlist.is_empty());
}

#[tokio::test]
async fn get_watchlist_returns_added_movie() {
    let (_addr, pool) = setup_test_app().await.unwrap();
    let dummy_user = test_user();
    let real_user = UserRepository::find_by_username(&pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();
    let movie = insert_movie(&pool).await;

    let now = OffsetDateTime::now_utc();
    sqlx::query!(
        "INSERT INTO watchlist (user_id, movie_id, added_at) VALUES (?, ?, ?)",
        real_user.id,
        movie.id,
        now
    )
    .execute(&pool)
    .await
    .unwrap();

    let result = WatchlistService::get_user_watchlist(&pool, &real_user.username).await;

    assert!(result.is_ok());
    let (_profile, watchlist) = result.unwrap();
    assert_eq!(watchlist.len(), 1);
}

#[tokio::test]
async fn get_watchlist_returns_multiple_movies() {
    let (_addr, pool) = setup_test_app().await.unwrap();
    let dummy_user = test_user();
    let real_user = UserRepository::find_by_username(&pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();

    let movie1 = insert_movie(&pool).await;
    let movie2 = insert_movie_custom(&pool, "tt02").await;
    let movie3 = insert_movie_custom(&pool, "tt03").await;

    let now = OffsetDateTime::now_utc();
    for movie_id in [movie1.id, movie2.id, movie3.id] {
        sqlx::query!(
            "INSERT INTO watchlist (user_id, movie_id, added_at) VALUES (?, ?, ?)",
            real_user.id,
            movie_id,
            now
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    let result = WatchlistService::get_user_watchlist(&pool, &real_user.username).await;

    assert!(result.is_ok());
    let (_profile, watchlist) = result.unwrap();
    assert_eq!(watchlist.len(), 3);
}

#[tokio::test]
async fn get_watchlist_for_nonexistent_user_fails() {
    let (_addr, pool) = setup_test_app().await.unwrap();

    let result = WatchlistService::get_user_watchlist(&pool, "nonexistent_user").await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::Status(status) if status.as_u16() == 404));
}

#[tokio::test]
async fn add_movie_creates_watchlist_entry() {
    let (_addr, pool) = setup_test_app().await.unwrap();
    let dummy_user = test_user();
    let real_user = UserRepository::find_by_username(&pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();
    let movie = insert_movie(&pool).await;

    let result = WatchlistService::add_watchlsited_movie(real_user.id, movie.id, &pool).await;

    assert!(result.is_ok());

    let entry = sqlx::query!("SELECT id FROM watchlist WHERE user_id = ?", real_user.id)
        .fetch_one(&pool)
        .await;
    assert!(entry.is_ok());
}

#[tokio::test]
async fn add_duplicate_movie_returns_error() {
    let (_addr, pool) = setup_test_app().await.unwrap();
    let dummy_user = test_user();
    let real_user = UserRepository::find_by_username(&pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();
    let movie = insert_movie(&pool).await;

    // add once - should succeed
    let result1 = WatchlistService::add_watchlsited_movie(real_user.id, movie.id, &pool).await;
    assert!(result1.is_ok());

    // add again - should fail
    let result2 = WatchlistService::add_watchlsited_movie(real_user.id, movie.id, &pool).await;

    assert!(result2.is_err());
    assert!(matches!(result2.unwrap_err(), Error::Generic(_)));
}

#[tokio::test]
async fn add_multiple_different_movies_succeeds() {
    let (_addr, pool) = setup_test_app().await.unwrap();
    let dummy_user = test_user();
    let real_user = UserRepository::find_by_username(&pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();

    let movie1 = insert_movie(&pool).await;
    let movie2 = insert_movie_custom(&pool, "tt02").await;
    let movie3 = insert_movie_custom(&pool, "tt03").await;

    for movie in [movie1, movie2, movie3] {
        let result = WatchlistService::add_watchlsited_movie(real_user.id, movie.id, &pool).await;
        assert!(result.is_ok());
    }

    let watchlist = WatchlistService::get_user_watchlist(&pool, &real_user.username).await;
    assert_eq!(watchlist.unwrap().1.len(), 3);
}

#[tokio::test]
async fn add_movie_with_nonexistent_movie_id_fails() {
    let (_addr, pool) = setup_test_app().await.unwrap();
    let dummy_user = test_user();
    let real_user = UserRepository::find_by_username(&pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();

    let nonexistent_movie_id = 99999;
    let result =
        WatchlistService::add_watchlsited_movie(real_user.id, nonexistent_movie_id, &pool).await;

    assert!(result.is_err());
    // should be a database error due to FK constraint
}

#[tokio::test]
async fn add_movie_with_nonexistent_user_id_fails() {
    let (_addr, pool) = setup_test_app().await.unwrap();
    let movie = insert_movie(&pool).await;

    let nonexistent_user_id = 99999;
    let result =
        WatchlistService::add_watchlsited_movie(nonexistent_user_id, movie.id, &pool).await;

    assert!(result.is_err());
    // Should be a database error due to FK constraint
}

// === remove_from_watchlist tests ===

#[tokio::test]
async fn remove_movie_deletes_watchlist_entry() {
    let (_addr, pool) = setup_test_app().await.unwrap();
    let dummy_user = test_user();
    let real_user = UserRepository::find_by_username(&pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();
    let movie = insert_movie(&pool).await;

    let now = OffsetDateTime::now_utc();
    sqlx::query!(
        "INSERT INTO watchlist (user_id, movie_id, added_at) VALUES (?, ?, ?)",
        real_user.id,
        movie.id,
        now
    )
    .execute(&pool)
    .await
    .unwrap();

    let result = WatchlistService::remove_from_watchlist(real_user.id, movie.id, &pool).await;

    assert!(result.is_ok());

    let check = WatchlistService::get_user_watchlist(&pool, &real_user.username).await;
    assert!(check.is_ok());
    assert_eq!(check.unwrap().1.len(), 0);
}

#[tokio::test]
async fn remove_nonexistent_entry_succeeds_idempotently() {
    let (_addr, pool) = setup_test_app().await.unwrap();
    let dummy_user = test_user();
    let real_user = UserRepository::find_by_username(&pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();
    let movie = insert_movie(&pool).await;

    // remove entry that doesnt exist
    let result = WatchlistService::remove_from_watchlist(real_user.id, movie.id, &pool).await;

    // should succeed (DELETE with no matches is valid SQL)
    assert!(result.is_ok());
}

#[tokio::test]
async fn remove_one_movie_keeps_others() {
    let (_addr, pool) = setup_test_app().await.unwrap();
    let dummy_user = test_user();
    let real_user = UserRepository::find_by_username(&pool, &dummy_user.username)
        .await
        .unwrap()
        .unwrap();

    let movie1 = insert_movie(&pool).await;
    let movie2 = insert_movie_custom(&pool, "tt02").await;

    let now = OffsetDateTime::now_utc();
    for movie_id in [movie1.id, movie2.id] {
        sqlx::query!(
            "INSERT INTO watchlist (user_id, movie_id, added_at) VALUES (?, ?, ?)",
            real_user.id,
            movie_id,
            now
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    let result = WatchlistService::remove_from_watchlist(real_user.id, movie1.id, &pool).await;
    assert!(result.is_ok());

    let watchlist = WatchlistService::get_user_watchlist(&pool, &real_user.username).await;
    let (_, movies) = watchlist.unwrap();
    assert_eq!(movies.len(), 1);
}
