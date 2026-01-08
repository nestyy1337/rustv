use askama::Template;

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorPageData {
    pub error_message: String,
    pub csrf_token: String,
}

impl ErrorPageData {
    pub fn new(error_message: String, csrf_token: String) -> Self {
        Self {
            error_message,
            csrf_token,
        }
    }
}
