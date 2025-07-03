use std::{str::FromStr, sync::Arc};

use axum::{
    Extension, Form, Router,
    http::Uri,
    response::{ErrorResponse, IntoResponse, Redirect, Response},
    routing::{get, post},
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

impl NewsForm {
    pub fn validate(&self) -> Option<Vec<String>> {
        let mut result = Vec::new();

        match self.title.as_ref() {
            Some(title) if title.is_empty() => {
                result.push("A title is required".to_owned());
            }
            None => {
                result.push("A title is required for each news article".to_owned());
            }
            _ => {}
        };

        match self.url.as_ref() {
            Some(url) if url.is_empty() => {
                result.push("A location for news article is required".to_owned())
            }
            Some(url) => match Uri::from_str(&url) {
                Ok(_) => {}
                Err(e) => result.push(format!("The provided location is invalid '{e}'")),
            },
            None => result.push("A location for news article is required".to_owned()),
            _ => {}
        };

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}

impl From<NewsForm> for NewsItem {
    fn from(value: NewsForm) -> Self {
        NewsItem::builder()
            .id(None)
            .title(value.title.unwrap())
            .url(value.url)
            .notes(value.notes)
            .hidden(value.hidden.unwrap_or(false))
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
) -> Response {
    tracing::debug!(
        form = ?news_form,
        "processing news article change"
    );

    let mut tera = Tera::new("./templates/**/*.tera").unwrap();
    tera.autoescape_on(vec![".tera"]);
    tera.register_filter("markdown", MarkdownFilter);

    let mut context = tera::Context::new();
    let news_item: NewsItem = news_form.clone().into();
    context.insert("newsItem", &news_item);

    match news_form.action.as_ref().map(String::as_str) {
        Some("preview") => {
            context.insert("preview_item", &news_item);
        }
        Some("save") if news_form.id.is_some() => {
            let errors = news_form.validate();
            if errors.is_some() {
                context.insert("errors", &errors);
            } else {
                match news_store.update(&news_form.into()).await {
                    Ok(news_item) => {
                        tracing::info!(
                            article = ?news_item,
                            "updated news item"
                        );
                        return Redirect::to("/news/admin").into_response();
                    }
                    Err(e) => {
                        context.insert("error_message", &e.to_string());
                    }
                }
            }
        }
        Some("create") => {
            let errors = news_form.validate();
            if errors.is_some() {
                context.insert("errors", &errors);
            } else {
                match news_store.create(&news_form.into()).await {
                    Ok(news_item) => {
                        tracing::info!(
                            article = ?news_item,
                            "created news item"
                        );
                        return Redirect::to("/news/admin").into_response();
                    }
                    Err(e) => {
                        context.insert("error_message", &e.to_string());
                    }
                }
            }
        }
        Some("delete") if news_form.id.is_some() => {
            match news_store.delete(news_form.id.unwrap_or(0)).await {
                Ok(_) => {
                    tracing::info!(
                        article_id = ?news_form.id,
                        "deleted news article {}",
                        news_form.id.unwrap_or_default()
                    );
                }
                Err(e) => {
                    context.insert("error_message", &e.to_string());
                }
            }
        }
        _ => {
            tracing::warn!(
                form = ?news_form,
                "an unknown action ({}) was passed update",
                news_form.action.clone().unwrap_or_default()
            );
        }
    }

    let tera = tera.render("content/news/edit.tera", &context);

    tracing::debug!(
        context = ?context,
        "tera :: -> {:?}",
        "rendered news article edit template"
    );

    page_builder.raw_html(tera.unwrap()).into_response()
}

struct MarkdownFilter;

impl tera::Filter for MarkdownFilter {
    fn filter(
        &self,
        value: &tera::Value,
        args: &std::collections::HashMap<String, tera::Value>,
    ) -> tera::Result<tera::Value> {
        match value {
            serde_json::Value::String(markdown_text) => {
                Ok(serde_json::Value::String(markdown::to_html(&markdown_text)))
            }
            _ => Err(tera::Error::msg(format!(
                "markdown can only be used on string values: {value:?}"
            ))),
        }
    }

    fn is_safe(&self) -> bool {
        true
    }
}
