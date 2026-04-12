## Examplary toy project to bridge the gap between oversimplied guides and more 'real-work' software.

Rustv is a simple platform to watch *legally shared* videos from torrent sites online.
Built with Axum, Sqlite, sqlx, FFmpeg and libgrqbit under the hood.

I advise to read [zero2prod](https://www.zero2prod.com/index.html) as a fenomenal start to designing apps with Rust. I've used this project to expand on design problems when implementing solutions a bit larger than just a simple guide.
This is just a toy project that's missing a lot of stuff to be production-ready.

### Storage backends

Movie storage is abstracted behind the `MovieStorage` trait with two implementations, selected via mutually exclusive feature flags:

- **`fs`** — local filesystem storage (`NaiveMovieStorage`). Converted videos and posters live under `./movies/`.
- **`s3`** — AWS S3 storage (`S3MovieStorage`). Converted videos are uploaded to S3 and served via presigned URLs.

```sh
cargo build --features fs   # local filesystem (default with no flags won't compile)
cargo build --features s3   # S3-backed storage
```

Both features cannot be enabled simultaneously — the build will fail with a compile error.

### Coming soon
- Idempotency.
- Move entirely into Actor concurrency scheme according to [Yoshua's blog](https://blog.yoshuawuyts.com/tree-structured-concurrency/) and remove any parentless tokio::spawns.
