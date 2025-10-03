use askama::Template;

use crate::models::{movie::Watchlist, users::UserProfile};

#[derive(Template)]
#[template(path = "watchlist.html")]
pub struct WatchlistView {
    pub profile: UserProfile,
    pub watchlist: Vec<Watchlist>,
}

impl WatchlistView {
    pub fn new(profile: UserProfile, watchlist: Vec<Watchlist>) -> Self {
        Self { profile, watchlist }
    }
}
