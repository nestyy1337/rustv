
CREATE TABLE tmdb_movies (
    adult INTEGER NOT NULL,
    backdrop_path TEXT,
    budget INTEGER NOT NULL,
    genres TEXT NOT NULL,
    homepage TEXT,
    id INTEGER PRIMARY KEY,
    imdb_id TEXT UNIQUE,
    origin_country TEXT NOT NULL,
    original_language TEXT NOT NULL,
    original_title TEXT NOT NULL,
    overview TEXT NOT NULL,
    popularity REAL NOT NULL,
    poster_path TEXT,
    production_company TEXT NOT NULL,
    release_date TEXT NOT NULL,
    revenue INTEGER NOT NULL,
    runtime INTEGER,
    status TEXT NOT NULL,
    tagline TEXT,
    title TEXT NOT NULL,
    video INTEGER NOT NULL,
    vote_average REAL NOT NULL,
    vote_count INTEGER NOT NULL
);

