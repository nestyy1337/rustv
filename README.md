## Examplary toy project to bridge the gap between oversimplied guides and more 'real-work' software.

Rustv is a simple platform to watch *legally shared* videos from torrent sites online.
Built with Axum, Sqlite, sqlx, FFmpeg and libgrqbit under the hood.

I advise to read  [zero2prod](https://www.zero2prod.com/index.html) as a fenomenal start to designing apps with Rust. I've used this project to expand on design problems when implementing solutions a bit larger than just a simple guide.
This is just a toy project that's missing a lot of stuff to be production-ready.

Few key changes coming soon:
Implement actual S3 Storage implementation of MovieStorage trait.
Add feature flag to properly handle building solution for S3/Local FS.
Idempotency.
Move entirely into Actor concurrency scheme according to [Yoshua's blog](https://blog.yoshuawuyts.com/tree-structured-concurrency/) and remove any parentless tokio::spawns.
