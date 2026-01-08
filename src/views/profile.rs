use askama::Template;

use crate::models::users::UserProfile;

#[derive(Template)]
#[template(path = "profile.html")]
pub struct ProfilePageData {
    pub profile: UserProfile,
    pub csrf_token: String,
}
