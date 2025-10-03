use askama::Template;

use crate::models::{movie::Movie, users::UserProfile};

#[derive(Template)]
#[template(path = "frontpage.html")]
pub struct FrontPageData {
    user_profile: UserProfile,
    movies: Vec<Movie>,
}

impl FrontPageData {
    pub fn new(user_profile: UserProfile, movies: Vec<Movie>) -> Self {
        Self {
            user_profile,
            movies,
        }
    }
}
