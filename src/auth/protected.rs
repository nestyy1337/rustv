use askama::Template;
use axum::{
    Router,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
};
use axum_messages::{Message, Messages};

// #[derive(Template)]
// #[template(path = "protected.html")]
// struct ProtectedTemplate<'a> {
//     messages: Vec<Message>,
//     username: &'a str,
// }
//
// pub fn router() -> Router<()> {
//     Router::new().route("/", get(self::get::protected))
// }
//
// mod get {
//     use crate::app::AuthSession;
//
//     use super::*;
//
//     pub async fn protected(auth_session: AuthSession, messages: Messages) -> impl IntoResponse {
//         match auth_session.user.as_ref() {
//             Some(user) => Html(
//                 ProtectedTemplate {
//                     messages: messages.into_iter().collect(),
//                     username: &user.username,
//                 }
//                 .render()
//                 .unwrap(),
//             )
//             .into_response(),
//
//             None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
//         }
//     }
// }
