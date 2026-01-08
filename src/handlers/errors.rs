use crate::{shared::error::Error, views::errors::ErrorPageData};
use askama::Template;
use axum::response::Html;

pub fn render_error(error: Error) -> String {
    let error_data = ErrorPageData::new(Error::to_string(&error), String::new());
    error_data
        .render()
        .unwrap_or_else(|_| "Error rendering page".to_string())
}

pub fn fallback_404() -> Html<String> {
    let error_data = ErrorPageData::new(
        "Requested resource doesn't exist".to_string(),
        String::new(),
    );
    error_data
        .render()
        .unwrap_or_else(|_| "Error rendering page".to_string())
        .into()
}
