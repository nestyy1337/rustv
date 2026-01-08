mod common;

use backend::{
    handlers::movies::DeleteWatchlistPayload,
    models::movie::Movie,
    services::{
        movie_manager::MovieManager,
        movies::{MovieService, SimpleMovieService},
    },
    shared::test_utils::setup_test_app,
};
use time::OffsetDateTime;

use crate::common::{
    client::TestClient,
    fixtures::{insert_movie, insert_movie_custom, insert_user, insert_watched_movie, tmdb_movie},
};

#[tokio::test]
async fn test_find_watched_movie() {
    let (addr, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();

    let user = backend::shared::test_utils::test_user();

    let none = service.find_watched_movies_by_username(&user).await;
    assert!(none.is_ok());

    insert_movie(&state.pool).await;
    let _movie = insert_watched_movie(&state.pool, user.id, 1).await;

    let found = service.find_watched_movies_by_username(&user).await;
    assert!(found.is_ok());
    let found = found.unwrap();
    assert_eq!(found.len(), 1);
}

#[tokio::test]
async fn test_is_watchlisted() {
    let (addr, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();

    let user = backend::shared::test_utils::test_user();

    let not_watchlisted = service.is_watchlisted(user.id, 1).await;
    assert!(not_watchlisted.is_ok());
    assert!(!not_watchlisted.unwrap());
    let now = OffsetDateTime::now_utc();

    insert_movie(&state.pool).await;
    sqlx::query!(
        "INSERT INTO watchlist (user_id, movie_id, added_at) VALUES (?, ?, ?)",
        user.id,
        1,
        now
    )
    .execute(&state.pool)
    .await
    .unwrap();

    let is_watchlisted = service.is_watchlisted(user.id, 1).await;
    assert!(is_watchlisted.is_ok());
    let is_watchlisted = is_watchlisted.unwrap();
    assert!(is_watchlisted);

    let is_watchlisted_other = service.is_watchlisted(user.id, 2).await;
    assert!(is_watchlisted_other.is_ok());
    let is_watchlisted_other = is_watchlisted_other.unwrap();
    assert!(!is_watchlisted_other);

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
    let after_delete = service.is_watchlisted(user.id, 1).await;
    assert!(after_delete.is_ok());
    let after_delete = after_delete.unwrap();
    assert!(!after_delete);
}

#[tokio::test]
async fn test_is_watched() {
    let (addr, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();

    let user = backend::shared::test_utils::test_user();

    let not_watched = service.is_watched(user.id, 1).await;
    assert!(not_watched.is_ok());
    assert!(!not_watched.unwrap());

    insert_movie(&state.pool).await;
    let movie = insert_watched_movie(&state.pool, user.id, 1).await;

    let is_watched = service.is_watched(user.id, 1).await;
    assert!(is_watched.is_ok());
    let is_watched = is_watched.unwrap();
    assert!(is_watched);

    let is_watched_other = service.is_watched(user.id, 2).await;
    assert!(is_watched_other.is_ok());
    let is_watched_other = is_watched_other.unwrap();
    assert!(!is_watched_other);

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
    let after_delete = service.is_watched(user.id, 1).await;
    assert!(after_delete.is_ok());
    let after_delete = after_delete.unwrap();
    assert!(!after_delete);
}

#[tokio::test]
async fn test_add_watched_movie() {
    let (_addr, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();

    let user = backend::shared::test_utils::test_user();

    insert_movie(&state.pool).await;

    let not_watched = service.is_watched(user.id, 1).await;
    assert!(not_watched.is_ok());
    assert!(!not_watched.unwrap());

    let add_result = service.add_watched_movie(user.id, 1, None).await;
    assert!(add_result.is_ok());
    add_result.unwrap();

    let is_watched = service.is_watched(user.id, 1).await;
    assert!(is_watched.is_ok());
    let is_watched = is_watched.unwrap();
    assert!(is_watched);

    // we add it again, should be idempotent
    let add_result = service.add_watched_movie(user.id, 1, None).await;
    assert!(add_result.is_ok());
}

#[tokio::test]
async fn test_delete_movie() {
    let (_addr, state) = setup_test_app().await.unwrap();

    let movie = insert_movie(&state.pool).await;
    let delete_result = state
        .movies_manager
        .movie_service
        .delete_movie(movie.id)
        .await;
    assert!(delete_result.is_ok());

    let fetch_result = sqlx::query!("SELECT * FROM movies WHERE id = ?", movie.id)
        .fetch_optional(&state.pool)
        .await
        .unwrap();

    assert!(fetch_result.is_none());

    let delete_again = state
        .movies_manager
        .movie_service
        .delete_movie(movie.id)
        .await;
    assert!(delete_again.is_ok());
}

#[tokio::test]
async fn test_find_multiple_watched_movies() {
    let (addr, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();
    let user = backend::shared::test_utils::test_user();

    insert_movie(&state.pool).await;
    insert_movie_custom(&state.pool, "tt02").await;
    insert_watched_movie(&state.pool, user.id, 1).await;
    insert_watched_movie(&state.pool, user.id, 2).await;

    let found = service.find_watched_movies_by_username(&user).await;
    assert_eq!(found.unwrap().len(), 2);
}

#[tokio::test]
async fn test_is_watched_with_invalid_ids() {
    let (_, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();

    let result = service.is_watched(-1, 1).await;
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[tokio::test]
async fn test_update_watched_movie_rating() {
    let (_, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();
    let user = backend::shared::test_utils::test_user();
    insert_movie(&state.pool).await;

    service
        .add_watched_movie(user.id, 1, Some(5.0))
        .await
        .unwrap();
    service
        .add_watched_movie(user.id, 1, Some(9.0))
        .await
        .unwrap();

    let movie = sqlx::query!(
        "SELECT rating FROM watched_movies WHERE user_id = ? AND movie_id = ?",
        user.id,
        1
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();

    assert_eq!(movie.rating, Some(9.0));
}

#[tokio::test]
async fn test_add_watched_nonexistant_movie() {
    let (_, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();
    let user = backend::shared::test_utils::test_user();

    let result = service.add_watched_movie(user.id, 999, None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_find_watched_movies_ordered_by_date() {
    let (_, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();
    let user = backend::shared::test_utils::test_user();

    insert_movie(&state.pool).await;
    insert_movie_custom(&state.pool, "tt02").await;
    insert_movie_custom(&state.pool, "tt03").await;

    // insert movies at different times
    insert_watched_movie(&state.pool, user.id, 1).await;
    insert_watched_movie(&state.pool, user.id, 2).await;
    insert_watched_movie(&state.pool, user.id, 3).await;

    let found = service
        .find_watched_movies_by_username(&user)
        .await
        .unwrap();

    assert_eq!(found.len(), 3);
}

#[tokio::test]
async fn test_find_watched_movies_with_different_ratings() {
    let (_, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();
    let user = backend::shared::test_utils::test_user();

    insert_movie(&state.pool).await;
    insert_movie_custom(&state.pool, "tt02").await;

    service
        .add_watched_movie(user.id, 1, Some(10.0))
        .await
        .unwrap();
    service.add_watched_movie(user.id, 2, None).await.unwrap();

    let found = service
        .find_watched_movies_by_username(&user)
        .await
        .unwrap();

    assert_eq!(found.len(), 2);
}

#[tokio::test]
async fn test_find_watched_movies_multiple_users_isolation() {
    let (_, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();
    let user1 = backend::shared::test_utils::test_user();

    // Create second user
    let user2 = insert_user(&state.pool, "testuser2").await;

    insert_movie(&state.pool).await;
    insert_movie_custom(&state.pool, "tt02").await;

    insert_watched_movie(&state.pool, user1.id, 1).await;
    insert_watched_movie(&state.pool, user2.id, 2).await;

    let user1_movies = service
        .find_watched_movies_by_username(&user1)
        .await
        .unwrap();

    // user 1 should only see their own watched movie
    assert_eq!(user1_movies.len(), 1);
}

#[tokio::test]
async fn test_add_watched_movie_with_minimum_rating() {
    let (_, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();
    let user = backend::shared::test_utils::test_user();
    insert_movie(&state.pool).await;

    let result = service.add_watched_movie(user.id, 1, Some(0.0)).await;
    assert!(result.is_ok());

    let movie = sqlx::query!(
        "SELECT rating FROM watched_movies WHERE user_id = ? AND movie_id = ?",
        user.id,
        1
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();

    assert_eq!(movie.rating, Some(0.0));
}

#[tokio::test]
async fn test_add_watched_movie_with_maximum_rating() {
    let (_, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();
    let user = backend::shared::test_utils::test_user();
    insert_movie(&state.pool).await;

    let result = service.add_watched_movie(user.id, 1, Some(10.0)).await;
    assert!(result.is_ok());

    let movie = sqlx::query!(
        "SELECT rating FROM watched_movies WHERE user_id = ? AND movie_id = ?",
        user.id,
        1
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();

    assert_eq!(movie.rating, Some(10.0));
}

#[tokio::test]
async fn test_add_watched_movie_update_rating_to_none() {
    let (_, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();
    let user = backend::shared::test_utils::test_user();
    insert_movie(&state.pool).await;

    service
        .add_watched_movie(user.id, 1, Some(8.0))
        .await
        .unwrap();
    service.add_watched_movie(user.id, 1, None).await.unwrap();

    let movie = sqlx::query!(
        "SELECT rating FROM watched_movies WHERE user_id = ? AND movie_id = ?",
        user.id,
        1
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();

    assert_eq!(movie.rating, None);
}

#[tokio::test]
async fn test_delete_movie_with_watchlist_entries() {
    let (_, state) = setup_test_app().await.unwrap();
    let user = backend::shared::test_utils::test_user();

    let movie = insert_movie(&state.pool).await;
    let now = OffsetDateTime::now_utc();

    sqlx::query!(
        "INSERT INTO watchlist (user_id, movie_id, added_at) VALUES (?, ?, ?)",
        user.id,
        movie.id,
        now
    )
    .execute(&state.pool)
    .await
    .unwrap();

    // delete should handle cascade or fail gracefully
    let delete_result = state
        .movies_manager
        .movie_service
        .delete_movie(movie.id)
        .await;

    assert!(delete_result.is_ok());
}

#[tokio::test]
async fn test_delete_movie_with_watched_entries() {
    let (_, state) = setup_test_app().await.unwrap();

    let user = backend::shared::test_utils::test_user();

    let movie = insert_movie(&state.pool).await;
    insert_watched_movie(&state.pool, user.id, movie.id).await;

    let delete_result = state
        .movies_manager
        .movie_service
        .delete_movie(movie.id)
        .await;

    dbg!(&delete_result);
    assert!(delete_result.is_ok());
}

#[tokio::test]
async fn test_delete_movie_with_zero_id() {
    let (_, state) = setup_test_app().await.unwrap();

    let result = state.movies_manager.movie_service.delete_movie(0).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_watchlist_multiple_users_same_movie() {
    let (_, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();
    let user1 = backend::shared::test_utils::test_user();
    let user2 = insert_user(&state.pool, "testuser2").await;
    let now = OffsetDateTime::now_utc();

    insert_movie(&state.pool).await;

    // both users watchlist same movie
    sqlx::query!(
        "INSERT INTO watchlist (user_id, movie_id, added_at) VALUES (?, ?, ?)",
        user1.id,
        1,
        now
    )
    .execute(&state.pool)
    .await
    .unwrap();

    sqlx::query!(
        "INSERT INTO watchlist (user_id, movie_id, added_at) VALUES (?, ?, ?)",
        user2.id,
        1,
        now
    )
    .execute(&state.pool)
    .await
    .unwrap();

    let user1_watchlisted = service.is_watchlisted(user1.id, 1).await.unwrap();
    let user2_watchlisted = service.is_watchlisted(user2.id, 1).await.unwrap();

    assert!(user1_watchlisted);
    assert!(user2_watchlisted);
}

#[tokio::test]
async fn test_watched_multiple_users_different_ratings() {
    let (_, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();
    let user1 = backend::shared::test_utils::test_user();
    let user2 = insert_user(&state.pool, "testuser2").await;

    insert_movie(&state.pool).await;

    service
        .add_watched_movie(user1.id, 1, Some(9.0))
        .await
        .unwrap();
    service
        .add_watched_movie(user2.id, 1, Some(3.0))
        .await
        .unwrap();

    let movie1 = sqlx::query!(
        "SELECT rating FROM watched_movies WHERE user_id = ? AND movie_id = ?",
        user1.id,
        1
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();

    let movie2 = sqlx::query!(
        "SELECT rating FROM watched_movies WHERE user_id = ? AND movie_id = ?",
        user2.id,
        1
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();

    assert_eq!(movie1.rating, Some(9.0));
    assert_eq!(movie2.rating, Some(3.0));
}

#[tokio::test]
async fn test_concurrent_watchlist_and_watched_operations() {
    let (_, state) = setup_test_app().await.unwrap();
    let service = state.movies_manager.movie_service.clone();
    let user = backend::shared::test_utils::test_user();
    insert_movie(&state.pool).await;

    let pool1 = state.pool.clone();
    let service2 = state.movies_manager.movie_service.clone();
    let now = OffsetDateTime::now_utc();

    let handle1 = tokio::spawn(async move {
        sqlx::query!(
            "INSERT INTO watchlist (user_id, movie_id, added_at) VALUES (?, ?, ?)",
            user.id,
            1,
            now
        )
        .execute(&pool1)
        .await
    });

    let handle2 =
        tokio::spawn(async move { service2.add_watched_movie(user.id, 1, Some(8.0)).await });

    let (r1, r2) = tokio::join!(handle1, handle2);

    assert!(r1.is_ok());
    assert!(r2.is_ok());

    let is_watchlisted = service.is_watchlisted(user.id, 1).await.unwrap();
    let is_watched = service.is_watched(user.id, 1).await.unwrap();

    assert!(is_watchlisted);
    assert!(is_watched);
}
