use axum::Extension;
use loki::{PageBuilder, PageResult};

pub async fn home(Extension(page_builder): Extension<PageBuilder>) -> PageResult {
    page_builder
        .html()
        .layout("templates/layout/main")
        .template("templates/content/home")
        .send()
}
