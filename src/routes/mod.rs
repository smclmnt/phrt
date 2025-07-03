use axum::Router;
use axum::response::{IntoResponse, Redirect};
use axum::routing::get;
use deadpool_postgres::Pool;

use crate::config::Config;

mod home;
mod news;

pub fn create_routes(_config: &Config, database_pool: &Pool) -> Router {
    let news_rotues = news::news_routes(database_pool);

    Router::new()
        .route("/", get(home::home))
        .route("/home", get(home_redirect))
        .nest("/news", news_rotues)
}

async fn home_redirect() -> impl IntoResponse {
    Redirect::permanent("/").into_response()
}
