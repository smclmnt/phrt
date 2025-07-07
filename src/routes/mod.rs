use std::sync::Arc;

use axum::Router;
use axum::response::{IntoResponse, Redirect};
use axum::routing::get;
use deadpool_postgres::Pool;
use tera::Tera;

use crate::config::Config;

mod home;
mod news;

pub use news::MarkdownFilter;

pub fn create_routes(_config: &Config, database_pool: &Pool, tera: Arc<Tera>) -> Router {
    let news_routes = news::news_routes(database_pool);

    Router::new()
        .route("/", get(home::home))
        .route("/home", get(home_redirect))
        .route("/map", get(home::trip_map))
        .route("/donate", get(home::donate))
        .route("/volunteer", get(home::volunteer))
        .route("/host", get(home::host_us))
        .route("/updates", get(home::updates))
        .nest("/news", news_routes)
    /* .merge(crate::authentication::login_routes(
        database_pool.clone(),
        tera.clone(),
    ))*/
}

async fn home_redirect() -> impl IntoResponse {
    Redirect::permanent("/").into_response()
}
