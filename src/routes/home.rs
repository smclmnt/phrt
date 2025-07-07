use std::sync::Arc;

use crate::authentication::TeraTemplate;
use axum::{Extension, response::IntoResponse};
use tera::Tera;

pub async fn home(Extension(tera): Extension<Arc<Tera>>) -> impl IntoResponse {
    TeraTemplate::builder()
        .tera(&tera)
        .template("content/home.tera")
        .build()
}

pub async fn volunteer(Extension(tera): Extension<Arc<Tera>>) -> impl IntoResponse {
    TeraTemplate::builder()
        .tera(&tera)
        .template("content/volunteer.tera")
        .build()
}

pub async fn donate(Extension(tera): Extension<Arc<Tera>>) -> impl IntoResponse {
    TeraTemplate::builder()
        .tera(&tera)
        .template("content/donate.tera")
        .build()
}

pub async fn host_us(Extension(tera): Extension<Arc<Tera>>) -> impl IntoResponse {
    TeraTemplate::builder()
        .tera(&tera)
        .template("content/host_us.tera")
        .build()
}

pub async fn trip_map(Extension(tera): Extension<Arc<Tera>>) -> impl IntoResponse {
    TeraTemplate::builder()
        .tera(&tera)
        .template("content/map.tera")
        .build()
}

pub async fn updates(Extension(tera): Extension<Arc<Tera>>) -> impl IntoResponse {
    TeraTemplate::builder()
        .tera(&tera)
        .template("content/updates.tera")
        .build()
}
