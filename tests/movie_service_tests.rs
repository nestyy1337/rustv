mod common;

use backend::{
    handlers::movies::DeleteWatchlistPayload, models::movie::WatchedMovie,
    services::movies::MovieService, shared::test_utils::setup_test_app,
};
use time::OffsetDateTime;

use crate::common::{
    client::TestClient,
    fixtures::{insert_movie, insert_movie_custom, insert_watched_movie, tmdb_movie},
};

#[tokio::test]
async fn test_find_watched_movie() {
    let (addr, pool) = setup_test_app().await.unwrap();

    let user = backend::shared::test_utils::test_user();

    let none = MovieService::find_watched_movies_by_username(&user, &pool).await;
    assert!(none.is_ok());
    println!("None: {:?}", none);
    assert!(none.unwrap().is_none());

    insert_movie(&pool).await;
    let movie = insert_watched_movie(&pool, user.id, 1).await;

    let found = MovieService::find_watched_movies_by_username(&user, &pool).await;
    assert!(found.is_ok());
    let found = found.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().len(), 1);
}

#[tokio::test]
async fn test_is_watchlisted() {
    let (addr, pool) = setup_test_app().await.unwrap();

    let user = backend::shared::test_utils::test_user();

    let not_watchlisted = MovieService::is_watchlisted(user.id, 1, &pool).await;
    assert!(not_watchlisted.is_ok());
    assert!(!not_watchlisted.unwrap());
    let now = OffsetDateTime::now_utc();

    insert_movie(&pool).await;
    sqlx::query!(
        "INSERT INTO watchlist (user_id, movie_id, added_at) VALUES (?, ?, ?)",
        user.id,
        1,
        now
    )
    .execute(&pool)
    .await
    .unwrap();

    let is_watchlisted = MovieService::is_watchlisted(user.id, 1, &pool).await;
    assert!(is_watchlisted.is_ok());
    let is_watchlisted = is_watchlisted.unwrap();
    assert_eq!(is_watchlisted, true);

    let is_watchlisted_other = MovieService::is_watchlisted(user.id, 2, &pool).await;
    assert!(is_watchlisted_other.is_ok());
    let is_watchlisted_other = is_watchlisted_other.unwrap();
    assert_eq!(is_watchlisted_other, false);

    let data = DeleteWatchlistPayload { movie_id: 1 };
    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();
    let res = client
        .delete("/api/watchlist")
        .await
        .json(&data)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let after_delete = MovieService::is_watchlisted(user.id, 1, &pool).await;
    assert!(after_delete.is_ok());
    let after_delete = after_delete.unwrap();
    assert_eq!(after_delete, false);
}

#[tokio::test]
async fn test_is_watched() {
    let (addr, pool) = setup_test_app().await.unwrap();

    let user = backend::shared::test_utils::test_user();

    let not_watched = MovieService::is_watched(user.id, 1, &pool).await;
    assert!(not_watched.is_ok());
    assert!(!not_watched.unwrap());

    insert_movie(&pool).await;
    let movie = insert_watched_movie(&pool, user.id, 1).await;

    let is_watched = MovieService::is_watched(user.id, 1, &pool).await;
    assert!(is_watched.is_ok());
    let is_watched = is_watched.unwrap();
    assert_eq!(is_watched, true);

    let is_watched_other = MovieService::is_watched(user.id, 2, &pool).await;
    assert!(is_watched_other.is_ok());
    let is_watched_other = is_watched_other.unwrap();
    assert_eq!(is_watched_other, false);

    let client = TestClient::new(&addr);
    client.login_default().await.unwrap();
    let res = client
        .delete(format!("/api/watched/{}/{}", user.username, movie.movie_id).as_str())
        .await
        .json(&movie)
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    assert_eq!(res.status().as_u16(), 200);
    let after_delete = MovieService::is_watched(user.id, 1, &pool).await;
    assert!(after_delete.is_ok());
    let after_delete = after_delete.unwrap();
    assert_eq!(after_delete, false);
}

#[tokio::test]
async fn test_add_watched_movie() {
    let (_addr, pool) = setup_test_app().await.unwrap();

    let user = backend::shared::test_utils::test_user();

    insert_movie(&pool).await;

    let not_watched = MovieService::is_watched(user.id, 1, &pool).await;
    assert!(not_watched.is_ok());
    assert!(!not_watched.unwrap());

    let add_result = MovieService::add_watched_movie(user.id, 1, None, &pool).await;
    assert!(add_result.is_ok());
    let _added = add_result.unwrap();

    let is_watched = MovieService::is_watched(user.id, 1, &pool).await;
    assert!(is_watched.is_ok());
    let is_watched = is_watched.unwrap();
    assert_eq!(is_watched, true);

    // we add it again, should be idempotent
    let add_result = MovieService::add_watched_movie(user.id, 1, None, &pool).await;
    assert!(add_result.is_ok());
}

#[tokio::test]
async fn test_add_movie_from_tmdb() {
    let (_addr, pool) = setup_test_app().await.unwrap();

    let tmdb_movie = tmdb_movie();
    let add_result = MovieService::add_movie(&tmdb_movie, &pool).await;
    assert!(add_result.is_ok());

    // adding again should fail due to unique constraint on imdb_id
    let add_result = MovieService::add_movie(&tmdb_movie, &pool).await;
    assert!(add_result.is_err());

    let movie = sqlx::query!("SELECT * FROM movies WHERE imdb_id = ?", tmdb_movie.imdb_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(movie.title, tmdb_movie.title);
}

#[tokio::test]
async fn test_delete_movie() {
    let (_addr, pool) = setup_test_app().await.unwrap();

    let movie = insert_movie(&pool).await;
    let delete_result = MovieService::delete_movie(movie.id, &pool).await;
    assert!(delete_result.is_ok());

    let fetch_result = sqlx::query!("SELECT * FROM movies WHERE id = ?", movie.id)
        .fetch_optional(&pool)
        .await
        .unwrap();

    assert!(fetch_result.is_none());

    let delete_again = MovieService::delete_movie(movie.id, &pool).await;
    dbg!(&delete_again);
    assert!(delete_again.is_err());
}

#[tokio::test]
async fn test_find_multiple_watched_movies() {
    let (addr, pool) = setup_test_app().await.unwrap();
    let user = backend::shared::test_utils::test_user();

    insert_movie(&pool).await;
    insert_movie_custom(&pool, "tt02").await;
    insert_watched_movie(&pool, user.id, 1).await;
    insert_watched_movie(&pool, user.id, 2).await;

    let found = MovieService::find_watched_movies_by_username(&user, &pool).await;
    assert_eq!(found.unwrap().unwrap().len(), 2);
}

#[tokio::test]
async fn test_is_watched_with_invalid_ids() {
    let (_, pool) = setup_test_app().await.unwrap();

    let result = MovieService::is_watched(-1, 1, &pool).await;
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[tokio::test]
async fn test_update_watched_movie_rating() {
    let (_, pool) = setup_test_app().await.unwrap();
    let user = backend::shared::test_utils::test_user();
    insert_movie(&pool).await;

    MovieService::add_watched_movie(user.id, 1, Some(5), &pool)
        .await
        .unwrap();
    MovieService::add_watched_movie(user.id, 1, Some(9), &pool)
        .await
        .unwrap();

    let movie = sqlx::query!(
        "SELECT rating FROM watched_movies WHERE user_id = ? AND movie_id = ?",
        user.id,
        1
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(movie.rating, Some(9.0));
}

#[tokio::test]
async fn test_concurrent_add_watched_movie() {
    let (_, pool) = setup_test_app().await.unwrap();
    let user = backend::shared::test_utils::test_user();
    insert_movie(&pool).await;

    let handle1 = {
        let pool = pool.clone();
        tokio::spawn(
            async move { MovieService::add_watched_movie(user.id, 1, Some(7), &pool).await },
        )
    };

    let handle2 = {
        let pool = pool.clone();
        tokio::spawn(
            async move { MovieService::add_watched_movie(user.id, 1, Some(8), &pool).await },
        )
    };

    let (r1, r2) = tokio::join!(handle1, handle2);

    assert!(r1.is_ok());
    assert!(r2.is_ok());

    let movie = sqlx::query!(
        "SELECT rating FROM watched_movies WHERE user_id = ? AND movie_id = ?",
        user.id,
        1
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(movie.rating == Some(8.0));
}

#[tokio::test]
async fn test_add_watched_nonexistant_movie() {
    let (_, pool) = setup_test_app().await.unwrap();
    let user = backend::shared::test_utils::test_user();

    let result = MovieService::add_watched_movie(user.id, 999, None, &pool).await;
    assert!(result.is_err());
}
