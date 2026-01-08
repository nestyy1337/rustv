use std::sync::Arc;

use axum::{extract::State, response::Html};

use crate::{app::AppState, shared::error::Error};

pub async fn get_all_metrics(State(state): State<Arc<AppState>>) -> Result<Html<String>, Error> {
    let movie_manager_state = state.metrics_service.report_movie_manager_state().await;
    let system_metrics = state.metrics_service.report_system_metrics();
    let download_manager_state = state.metrics_service.report_download_manager_state().await;

    let combined_report = format!(
        "<h1>System Metrics Report</h1>\
         <h2>Movie Manager State</h2><pre>{}</pre>\
         <h2>System Metrics</h2><pre>{}</pre>\
         <h2>Download Manager State</h2><pre>{}</pre>",
        movie_manager_state, system_metrics, download_manager_state
    );

    Ok(Html(combined_report))
}
