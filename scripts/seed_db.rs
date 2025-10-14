use std::collections::HashMap;

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

    let user_ids: Vec<(i64, String)> = sqlx::query_as("SELECT id, username FROM users")
        .fetch_all(&pool)
        .await?;

    let username_to_id: HashMap<&str, i64> = user_ids
        .iter()
        .map(|(id, username)| (username.as_str(), *id))
        .collect();

    let movies = vec![
        (
            "tt0111161",
            "The Shawshank Redemption",
            "Frank Darabont",
            1994,
            "Drama",
            true,
        ),
        (
            "tt0068646",
            "The Godfather",
            "Francis Ford Coppola",
            1972,
            "Crime",
            true,
        ),
        (
            "tt0468569",
            "The Dark Knight",
            "Christopher Nolan",
            2008,
            "Action",
            true,
        ),
        (
            "tt0167260",
            "The Lord of the Rings: The Return of the King",
            "Peter Jackson",
            2003,
            "Fantasy",
            true,
        ),
        (
            "tt0110912",
            "Pulp Fiction",
            "Quentin Tarantino",
            1994,
            "Crime",
            true,
        ),
        (
            "tt0137523",
            "Fight Club",
            "David Fincher",
            1999,
            "Drama",
            true,
        ),
        (
            "tt0109830",
            "Forrest Gump",
            "Robert Zemeckis",
            1994,
            "Drama",
            true,
        ),
        (
            "tt0120737",
            "The Lord of the Rings: The Fellowship of the Ring",
            "Peter Jackson",
            2001,
            "Fantasy",
            true,
        ),
        ("tt0443706", "Zodiac", "David Fincher", 2007, "Crime", true),
    ];

    for (imdb_id, title, director, year, genre, available) in movies {
        sqlx::query(
            "INSERT OR IGNORE INTO movies (imdb_id, title, director, release_year, genre, available)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(imdb_id)
        .bind(title)
        .bind(director)
        .bind(year)
        .bind(genre)
        .bind(available)
        .execute(&pool)
        .await?;
    }
    println!("Inserted movies");

    let movie_ids: Vec<(i64, String)> = sqlx::query_as("SELECT id, imdb_id FROM movies")
        .fetch_all(&pool)
        .await?;

    let imdb_to_id: HashMap<&str, i64> = movie_ids
        .iter()
        .map(|(id, imdb_id)| (imdb_id.as_str(), *id))
        .collect();

    let reviews = vec![
        (
            username_to_id["alice"],
            imdb_to_id["tt0111161"],
            "Absolutely brilliant masterpiece!",
            9.5,
        ),
        (
            username_to_id["alice"],
            imdb_to_id["tt0068646"],
            "Classic cinema at its finest.",
            9.0,
        ),
        (
            username_to_id["bob"],
            imdb_to_id["tt0111161"],
            "Best movie I've ever seen!",
            10.0,
        ),
        (
            username_to_id["bob"],
            imdb_to_id["tt0468569"],
            "Heath Ledger's performance is unforgettable.",
            9.8,
        ),
        (
            username_to_id["charlie"], // Fixed typo
            imdb_to_id["tt0167260"],
            "Epic trilogy conclusion!",
            9.2,
        ),
        (
            username_to_id["charlie"], // Fixed typo
            imdb_to_id["tt0110912"],
            "Tarantino's genius on full display.",
            8.9,
        ),
        (
            username_to_id["alice"],
            imdb_to_id["tt0111161"],
            "Absolutely brilliant masterpiece!",
            9.5,
        ),
        (
            username_to_id["alice"],
            imdb_to_id["tt0468569"],
            "Classic cinema at its finest.",
            9.0,
        ),
        (
            username_to_id["bob"],
            imdb_to_id["tt0167260"],
            "Heath Ledger's performance is unforgettable.",
            9.8,
        ),
        (
            username_to_id["charlie"], // Fixed typo
            imdb_to_id["tt0167260"],
            "Epic trilogy conclusion!",
            9.2,
        ),
        (
            username_to_id["charlie"], // Fixed typo
            imdb_to_id["tt0110912"],
            "Tarantino's genius on full display.",
            8.9,
        ),
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
        (username_to_id["alice"], imdb_to_id["tt0468569"]),
        (username_to_id["alice"], imdb_to_id["tt0167260"]),
        (username_to_id["bob"], imdb_to_id["tt0110912"]),
        (username_to_id["charlie"], imdb_to_id["tt0111161"]),
        (username_to_id["charlie"], imdb_to_id["tt0068646"]),
        (username_to_id["alice"], imdb_to_id["tt0111161"]),
        (username_to_id["alice"], imdb_to_id["tt0068646"]),
        (username_to_id["alice"], imdb_to_id["tt0468569"]),
        (username_to_id["alice"], imdb_to_id["tt0167260"]),
        (username_to_id["alice"], imdb_to_id["tt0110912"]),
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
        (username_to_id["alice"], imdb_to_id["tt0111161"], Some(9.5)),
        (username_to_id["alice"], imdb_to_id["tt0068646"], Some(9.0)),
        (username_to_id["bob"], imdb_to_id["tt0111161"], Some(10.0)),
        (username_to_id["bob"], imdb_to_id["tt0468569"], Some(9.8)),
        (
            username_to_id["charlie"],
            imdb_to_id["tt0167260"],
            Some(9.2),
        ),
        (username_to_id["charlie"], imdb_to_id["tt0110912"], None),
        (username_to_id["alice"], imdb_to_id["tt0111161"], Some(9.5)),
        (username_to_id["alice"], imdb_to_id["tt0068646"], Some(9.0)),
        (username_to_id["alice"], imdb_to_id["tt0468569"], Some(9.8)),
        (username_to_id["alice"], imdb_to_id["tt0167260"], Some(9.2)),
        (username_to_id["alice"], imdb_to_id["tt0110912"], Some(8.9)),
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
