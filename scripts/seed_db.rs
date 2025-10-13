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
        ("tt0443706", "Zodiac", "David Fincher", 2007, "Crime"),
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

    // we need to get the real ids of the movies
    let movie_ids: Vec<(i64, String)> = sqlx::query_as("SELECT id, imdb_id FROM movies")
        .fetch_all(&pool)
        .await?;
    let ids = movie_ids
        .into_iter()
        .filter_map(|r| r.0.into())
        .collect::<Vec<i64>>();

    let reviews = vec![
        (1, ids[0], "Absolutely brilliant masterpiece!", 9.5),
        (1, ids[1], "Classic cinema at its finest.", 9.0),
        (2, ids[0], "Best movie I've ever seen!", 10.0),
        (
            2,
            ids[2],
            "Heath Ledger's performance is unforgettable.",
            9.8,
        ),
        (3, ids[3], "Epic trilogy conclusion!", 9.2),
        (3, ids[4], "Tarantino's genius on full display.", 8.9),
        (4, ids[0], "Absolutely brilliant masterpiece!", 9.5),
        (4, ids[2], "Classic cinema at its finest.", 9.0),
        (
            4,
            ids[3],
            "Heath Ledger's performance is unforgettable.",
            9.8,
        ),
        (4, ids[3], "Epic trilogy conclusion!", 9.2),
        (4, ids[4], "Tarantino's genius on full display.", 8.9),
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
        (1, ids[2]),
        (1, ids[3]),
        (2, ids[4]),
        (3, ids[0]),
        (3, ids[1]),
        (4, ids[0]),
        (4, ids[1]),
        (4, ids[2]),
        (4, ids[3]),
        (4, ids[4]),
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
        (1, ids[0], Some(9.5)),
        (1, ids[1], Some(9.0)),
        (2, ids[0], Some(10.0)),
        (2, ids[2], Some(9.8)),
        (3, ids[3], Some(9.2)),
        (3, ids[4], None),
        (4, ids[0], Some(9.5)),
        (4, ids[1], Some(9.0)),
        (4, ids[2], Some(9.8)),
        (4, ids[3], Some(9.2)),
        (4, ids[4], Some(8.9)),
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
