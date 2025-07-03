use std::sync::Arc;

use axum::{
    Extension, Form, Router,
    routing::{Route, get, post},
};
use deadpool_postgres::Pool;
use loki::Cache;
use loki::{PageBuilder, PageResult};
use serde::{Deserialize, Serialize};
use tera::Tera;

use crate::services::{NewsItem, NewsStore};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewsForm {
    id: Option<i32>,
    title: Option<String>,
    url: Option<String>,
    notes: Option<String>,
    hidden: Option<bool>,
    action: Option<String>,
    is_new: Option<bool>,
}

impl From<NewsForm> for NewsItem {
    fn from(value: NewsForm) -> Self {
        NewsItem::builder()
            .id(value.id)
            .title(value.title.unwrap())
            .url(value.url)
            .notes(value.notes)
            .hidden((value.hidden.unwrap_or(false)))
            .build()
    }
}

pub static NEWS_CACHE: Cache<Vec<NewsItem>> = Cache::const_always();

pub fn news_routes(database_pool: &Pool) -> Router {
    let news_store = Arc::new(
        NewsStore::builder()
            .database_pool(database_pool.clone())
            .build(),
    );

    let user_routes = Router::new()
        .route("/", get(news))
        .layer(Extension(news_store.clone()));

    let admin_routes = Router::new()
        .route("/admin", get(admin_news))
        .route("/admin/save", post(news_admin_save))
        .layer(Extension(news_store.clone()));

    user_routes.merge(admin_routes)
}

async fn news(
    Extension(page_builder): Extension<PageBuilder>,
    Extension(news_store): Extension<Arc<NewsStore>>,
) -> PageResult {
    let news = NEWS_CACHE
        .try_fetch(async move { news_store.all().await })
        .await
        .unwrap_or_default();

    tracing::debug!("--> news : {:?}", news);

    page_builder
        .html()
        .layout("templates/layout/main")
        .template("templates/content/news")
        .value("news", news)
        .send()
}

async fn admin_news(
    Extension(page_builder): Extension<PageBuilder>,
    Extension(news_store): Extension<Arc<NewsStore>>,
) -> PageResult {
    let news = news_store
        .all()
        .await
        .inspect_err(|e| tracing::error!("failed to retrieve news from database: {e}"))
        .unwrap_or_default();

    page_builder
        .html()
        .layout("templates/layout/main")
        .template("templates/content/news/list")
        .value("news", news)
        .send()
}
pub async fn news_admin_save(
    Extension(page_builder): Extension<PageBuilder>,
    Extension(news_store): Extension<Arc<NewsStore>>,
    Form(news_form): Form<NewsForm>,
) -> PageResult {
    tracing::debug!("form datra --> {:?}", news_form);

    let errors = vec![
        String::from("Location is required"),
        String::from("Title is required"),
    ];

    let mut tera = Tera::new("./template/**/*.tera").unwrap();
    tera.autoescape_on(vec![".tera"]);

    let mut context = tera::Context::new();
    context.insert("newsItem", &serde_json::Value::Null);
    context.insert("errors", &errors);

    tracing::error!(
        "tera :: -> {:?}",
        tera.render("content/news/edit.tera", &context)
    );

    page_builder
        .html()
        .layout("templates/layout/main")
        .template("templates/content/news/edit")
        .value("newsItem", news_form)
        .value("xerorrs", errors)
        .send()
}
