mod common;

use crate::common::fixtures::{insert_movie, insert_movie_custom, movie, tmdb_movie};
use backend::{repositories::movies::MovieRepository, shared::test_utils::setup_test_app};

#[tokio::test]
async fn test_get_movie_by_id() {
    let (_addr, state) = setup_test_app().await.unwrap();

    let movie = insert_movie(&state.pool).await;

    let fetched_movie = MovieRepository::get_movie_by_id(movie.id, &state.pool).await;
    assert!(fetched_movie.is_ok());
    let fetched_movie = fetched_movie.unwrap();
    assert!(fetched_movie.is_some());
    let fetched_movie = fetched_movie.unwrap();
    assert_eq!(fetched_movie.id, movie.id);
    assert_eq!(fetched_movie.title, movie.title);

    let non_existent_movie = MovieRepository::get_movie_by_id(9999, &state.pool).await;
    assert!(non_existent_movie.is_ok());
    assert!(non_existent_movie.unwrap().is_none());
}

#[tokio::test]
async fn test_find_watched_movies_by_username_no_movies() {
    let (_addr, state) = setup_test_app().await.unwrap();

    let user = backend::shared::test_utils::test_user();

    let movies = MovieRepository::find_watched_movies_by_username(&user, state.pool.clone()).await;
    assert!(movies.is_ok());
    let movies = movies.unwrap();
    assert!(movies.is_some());
    assert!(movies.unwrap().is_empty());
}

#[tokio::test]
async fn test_find_watched_movies_by_username_with_movies() {
    let (_addr, state) = setup_test_app().await.unwrap();
    let user = backend::shared::test_utils::test_user();
    insert_movie(&state.pool).await;
    let now = time::OffsetDateTime::now_utc();
    let _ = sqlx::query!(
        "INSERT INTO watched_movies (user_id, movie_id, watched_at, rating) VALUES (?, ?, ?, ?)",
        user.id,
        1,
        now,
        5
    )
    .execute(&state.pool)
    .await;
    let movies = MovieRepository::find_watched_movies_by_username(&user, state.pool.clone()).await;
    assert!(movies.is_ok());
    let movies = movies.unwrap();
    assert!(movies.is_some());
    assert_eq!(movies.unwrap().len(), 1);
}

#[tokio::test]
async fn test_delete_watched_movie() {
    let (_addr, state) = setup_test_app().await.unwrap();
    let user = backend::shared::test_utils::test_user();
    insert_movie(&state.pool).await;
    let now = time::OffsetDateTime::now_utc();
    let _ = sqlx::query!(
        "INSERT INTO watched_movies (user_id, movie_id, watched_at, rating) VALUES (?, ?, ?, ?)",
        user.id,
        1,
        now,
        5
    )
    .execute(&state.pool)
    .await;

    let result = MovieRepository::delete_watched_movie(user.id, 1, &state.pool).await;
    assert!(result.is_ok());

    let movies = MovieRepository::find_watched_movies_by_username(&user, state.pool.clone()).await;
    assert!(movies.is_ok());
    let movies = movies.unwrap();
    assert!(movies.is_some());
    assert!(movies.unwrap().is_empty());
}

#[tokio::test]
async fn test_delete_watched_movie_non_existent() {
    let (_addr, state) = setup_test_app().await.unwrap();
    let user = backend::shared::test_utils::test_user();
    let result = MovieRepository::delete_watched_movie(user.id, 9999, &state.pool).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_top10_latest_movies() {
    let (_addr, state) = setup_test_app().await.unwrap();

    for i in 0..15 {
        let mut movie = insert_movie_custom(&state.pool, format!("tt{}", i).as_str()).await;
        movie.release_year = 2000 + i;
        let _ = sqlx::query!(
            "UPDATE movies SET release_year = ? WHERE id = ?",
            movie.release_year,
            movie.id
        )
        .execute(&state.pool)
        .await;
    }

    let movies = MovieRepository::get_top10_latest_movies(&state.pool).await;
    assert!(movies.is_ok());
    let movies = movies.unwrap();
    assert_eq!(movies.len(), 10);
    assert_eq!(movies[0].release_year, 2014);
    assert_eq!(movies[9].release_year, 2005);
}

#[tokio::test]
async fn test_get_top10_latest_movies_no_movies() {
    let (_addr, state) = setup_test_app().await.unwrap();

    let movies = MovieRepository::get_top10_latest_movies(&state.pool).await;
    assert!(movies.is_ok());
    let movies = movies.unwrap();
    assert!(movies.is_empty());
}

#[tokio::test]
async fn test_search_movie_by_title() {
    let (_addr, state) = setup_test_app().await.unwrap();
    let movie1 = insert_movie_custom(&state.pool, "tt001").await;
    let movie2 = insert_movie_custom(&state.pool, "tt002").await;
    let movie3 = insert_movie_custom(&state.pool, "tt003").await;
    let _ = sqlx::query!(
        "UPDATE movies SET title = ? WHERE id = ?",
        "The Great Adventure",
        movie1.id
    )
    .execute(&state.pool)
    .await;

    let _ = sqlx::query!(
        "UPDATE movies SET title = ? WHERE id = ?",
        "Adventure in the Mountains",
        movie2.id
    )
    .execute(&state.pool)
    .await;

    let _ = sqlx::query!(
        "UPDATE movies SET title = ? WHERE id = ?",
        "Romantic Comedy",
        movie3.id
    )
    .execute(&state.pool)
    .await;

    let results = MovieRepository::search_movie_by_title(&state.pool, "Adventure").await;
    assert!(results.is_ok());
    let results = results.unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|m| m.id == movie1.id));
    assert!(results.iter().any(|m| m.id == movie2.id));

    let no_results = MovieRepository::search_movie_by_title(&state.pool, "Sci-Fi").await;
    assert!(no_results.is_ok());
    let no_results = no_results.unwrap();
    assert!(no_results.is_empty());
}

#[tokio::test]
async fn test_add_movie() {
    let (_addr, state) = setup_test_app().await.unwrap();

    // First insert tmdb_movie to satisfy foreign key constraint
    let tmdb_movie = tmdb_movie();
    MovieRepository::insert_tmdb_movie(&tmdb_movie, &state.pool)
        .await
        .expect("Failed to insert tmdb movie");

    let movie = movie();

    let id = MovieRepository::add_movie(&movie, &state.pool).await;
    assert!(id.is_ok());
    let id = id.unwrap();
    assert!(id > 0);
}

#[tokio::test]
async fn test_get_movie_by_imdb_id() {
    let (_addr, state) = setup_test_app().await.unwrap();

    let movie = insert_movie(&state.pool).await;

    let fetched_movie = MovieRepository::get_movie_by_imdb_id(&state.pool, &movie.imdb_id).await;
    assert!(fetched_movie.is_ok());
    let fetched_movie = fetched_movie.unwrap();
    assert!(fetched_movie.is_some());
    let fetched_movie = fetched_movie.unwrap();
    assert_eq!(fetched_movie.id, movie.id);
    assert_eq!(fetched_movie.imdb_id, movie.imdb_id);

    let non_existent_movie =
        MovieRepository::get_movie_by_imdb_id(&state.pool, "nonexistent").await;
    assert!(non_existent_movie.is_ok());
    assert!(non_existent_movie.unwrap().is_none());
}
