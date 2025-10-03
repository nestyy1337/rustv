use sqlx::{Pool, Sqlite};

use crate::{
    models::users::User, repositories::movies::MovieRepository, shared::error::Error,
    views::movie::WatchedMovieDetailed,
};

pub struct MovieService;

impl MovieService {
    pub async fn find_watched_movies_by_username(
        user: &User,
        pool: Pool<Sqlite>,
    ) -> Result<Option<Vec<WatchedMovieDetailed>>, Error> {
        let movies = MovieRepository::find_watched_movies_by_username(user, pool.clone()).await;

        tracing::debug!(
            "Fetched watched movies for user {}: {:?}",
            user.username,
            movies
        );

        let movies = match movies {
            Ok(movies) => movies,
            Err(e) => {
                tracing::error!("Database error: {}", e);
                return Err(Error::DatabaseError(e));
            }
        };

        Ok(movies)
    }
}
