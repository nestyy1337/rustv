use chrono::Utc;
use sqlx::sqlite::SqlitePool;

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:./main.db".to_string());

    let pool = SqlitePool::connect(&database_url).await?;

    println!("Seeding database...");

    let users = vec![
        ("alice", "alice@example.com", "Alice Smith", true),
        ("bob", "bob@example.com", "Bob Johnson", false),
        ("charlie", "charlie@example.com", "Charlie Brown", false),
    ];

    for (username, email, display_name, is_admin) in users {
        sqlx::query(
            "INSERT OR IGNORE INTO users (username, email, password_hash, display_name, is_admin)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(username)
        .bind(email)
        .bind("$2b$12$dummy_hash_for_testing")
        .bind(display_name)
        .bind(is_admin)
        .execute(&pool)
        .await?;
    }
    println!("✓ Inserted users");

    let movies = vec![
        (
            "tt0111161",
            "The Shawshank Redemption",
            "Frank Darabont",
            1994,
            "Drama",
        ),
        (
            "tt0068646",
            "The Godfather",
            "Francis Ford Coppola",
            1972,
            "Crime",
        ),
        (
            "tt0468569",
            "The Dark Knight",
            "Christopher Nolan",
            2008,
            "Action",
        ),
        (
            "tt0167260",
            "The Lord of the Rings: The Return of the King",
            "Peter Jackson",
            2003,
            "Fantasy",
        ),
        (
            "tt0110912",
            "Pulp Fiction",
            "Quentin Tarantino",
            1994,
            "Crime",
        ),
        ("tt0137523", "Fight Club", "David Fincher", 1999, "Drama"),
        (
            "tt0109830",
            "Forrest Gump",
            "Robert Zemeckis",
            1994,
            "Drama",
        ),
        (
            "tt0120737",
            "The Lord of the Rings: The Fellowship of the Ring",
            "Peter Jackson",
            2001,
            "Fantasy",
        ),
    ];

    for (imdb_id, title, director, year, genre) in movies {
        sqlx::query(
            "INSERT OR IGNORE INTO movies (imdb_id, title, director, release_year, genre)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(imdb_id)
        .bind(title)
        .bind(director)
        .bind(year)
        .bind(genre)
        .execute(&pool)
        .await?;
    }
    println!("Inserted movies");

    let reviews = vec![
        (1, 1, "Absolutely brilliant masterpiece!", 9.5),
        (1, 2, "Classic cinema at its finest.", 9.0),
        (2, 1, "Best movie I've ever seen!", 10.0),
        (2, 3, "Heath Ledger's performance is unforgettable.", 9.8),
        (3, 4, "Epic trilogy conclusion!", 9.2),
        (3, 5, "Tarantino's genius on full display.", 8.9),
        (4, 1, "Absolutely brilliant masterpiece!", 9.5),
        (4, 2, "Classic cinema at its finest.", 9.0),
        (4, 3, "Heath Ledger's performance is unforgettable.", 9.8),
        (4, 4, "Epic trilogy conclusion!", 9.2),
        (4, 5, "Tarantino's genius on full display.", 8.9),
    ];

    for (user_id, movie_id, content, rating) in reviews {
        sqlx::query(
            "INSERT OR IGNORE INTO reviews (user_id, movie_id, content, rating)
             VALUES (?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(movie_id)
        .bind(content)
        .bind(rating)
        .execute(&pool)
        .await?;
    }
    println!("Inserted reviews");

    let watchlist = vec![
        (1, 3),
        (1, 4),
        (2, 5),
        (3, 1),
        (3, 2),
        (4, 1),
        (4, 2),
        (4, 3),
        (4, 4),
        (4, 5),
    ];

    for (user_id, movie_id) in watchlist {
        sqlx::query(
            "INSERT OR IGNORE INTO watchlist (user_id, movie_id)
             VALUES (?, ?)",
        )
        .bind(user_id)
        .bind(movie_id)
        .execute(&pool)
        .await?;
    }
    println!("Inserted watchlist entries");

    let watched = vec![
        (1, 1, Some(9.5)),
        (1, 2, Some(9.0)),
        (2, 1, Some(10.0)),
        (2, 3, Some(9.8)),
        (3, 4, Some(9.2)),
        (3, 5, None),
        (4, 1, Some(9.5)),
        (4, 2, Some(9.0)),
        (4, 3, Some(9.8)),
        (4, 4, Some(9.2)),
        (4, 5, Some(8.9)),
    ];

    for (user_id, movie_id, rating) in watched {
        sqlx::query(
            "INSERT OR IGNORE INTO watched_movies (user_id, movie_id, rating)
             VALUES (?, ?, ?)",
        )
        .bind(user_id)
        .bind(movie_id)
        .bind(rating)
        .execute(&pool)
        .await?;
    }
    println!("Inserted watched movies");

    println!("Database seeded successfully");

    Ok(())
}
