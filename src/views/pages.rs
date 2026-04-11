use askama::Template;

use crate::models::{movie::Movie, users::UserProfile};

#[derive(Template)]
#[template(path = "frontpage.html")]
pub struct FrontPageData {
    user_profile: UserProfile,
    movies: Vec<Movie>,
    pub csrf_token: String,
}

impl FrontPageData {
    #[must_use]
    pub fn new(user_profile: UserProfile, movies: Vec<Movie>, csrf_token: String) -> Self {
        Self {
            user_profile,
            movies,
            csrf_token,
        }
    }
}
