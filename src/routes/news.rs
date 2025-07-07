use std::{str::FromStr, sync::Arc};

use axum::{
    Extension, Form, Router,
    body::Body,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use deadpool_postgres::Pool;
use loki::Cache;
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};
use tracing::instrument;

use crate::services::{NewsItem, NewsStore};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewsForm {
    id: Option<i64>,
    title: Option<String>,
    url: Option<String>,
    notes: Option<String>,
    hidden: Option<bool>,
    action: Option<String>,
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
            .id(value.id)
            .title(value.title.unwrap_or_default())
            .url(value.url)
            .notes(value.notes)
            .hidden(value.hidden.is_some_and(|hidden| hidden))
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
        .route("/admin", get(list_news))
        .route("/admin", post(manage_mews))
        .layer(Extension(news_store.clone()));

    user_routes.merge(admin_routes)
}

fn tera_response<T>(tera: &Tera, template: T, context: &Context) -> Response
where
    T: AsRef<str>,
{
    match tera.render(template.as_ref(), context) {
        Ok(html) => {
            tracing::debug!(
                context = ?context,
                "rendering template '{}'",
                template.as_ref()
            );
            match Response::builder()
                .header(header::CONTENT_TYPE, "text/html")
                .body(Body::from(html))
            {
                Ok(response) => response,
                Err(e) => {
                    tracing::error!(
                        error = ?e,
                        context = ?context,
                        template = template.as_ref(),
                        "failed to generate response: {e}"
                    );
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        Err(e) => {
            tracing::error!(
                error = ?e,
                context = ?context,
                template = template.as_ref(),
                "{e}"
            );
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn news(
    Extension(news_store): Extension<Arc<NewsStore>>,
    Extension(tera): Extension<Arc<Tera>>,
) -> std::result::Result<Response, StatusCode> {
    let news = NEWS_CACHE
        .try_fetch(async move {
            news_store.all().await.map(|articles| {
                articles
                    .into_iter()
                    .filter(|article| !article.hidden)
                    .collect::<Vec<NewsItem>>()
            })
        })
        .await
        .unwrap_or_default();

    let mut context = Context::new();
    context.insert("news", &news);

    Ok(tera_response(&tera, "content/news.tera", &context))
}

async fn list_news(
    Extension(news_store): Extension<Arc<NewsStore>>,
    Extension(tera): Extension<Arc<Tera>>,
) -> std::result::Result<Response, StatusCode> {
    let news = news_store.all().await.unwrap_or_default();
    let mut context = Context::new();
    context.insert("articles", &news);
    Ok(tera_response(&tera, "content/news/list.tera", &context))
}

#[instrument(level = "info", skip(news_store, tera))]
pub async fn manage_mews(
    Extension(news_store): Extension<Arc<NewsStore>>,
    Extension(tera): Extension<Arc<Tera>>,
    Form(news_form): Form<NewsForm>,
) -> Response {
    let mut context = tera::Context::new();

    match news_form.action.as_ref().map(String::as_str) {
        Some("preview") => {
            let preview_item: NewsItem = news_form.into();
            context.insert("preview_item", &preview_item);
            context.insert("newsItem", &preview_item);
        }
        Some("save") => {
            let news_item: NewsItem = news_form.clone().into();
            context.insert("newsItem", &news_item);
            let errors = news_form.validate();

            if errors.is_some() {
                context.insert("errors", &errors);
            } else {
                match news_store.save(&news_form.into()).await {
                    Ok(news_item) => {
                        tracing::info!(
                            article = ?news_item,
                            "saved news article"
                        );
                        NEWS_CACHE.clear().await;
                        return Redirect::to("/news/admin").into_response();
                    }
                    Err(e) => {
                        context.insert("error_message", &e.to_string());
                    }
                }
            }
        }
        Some("create") => {
            context.insert("newsItem", &NewsItem::for_create());
        }
        Some("update") => {
            let Some(id) = news_form.id else {
                tracing::error!("request was missing article identifier");
                return StatusCode::NOT_FOUND.into_response();
            };

            match news_store.get(id).await {
                Ok(Some(news_item)) => context.insert("newsItem", &news_item),
                Ok(None) => {
                    tracing::error!("a request for invalid article '{id}' was submitted");
                    return StatusCode::NOT_FOUND.into_response();
                }
                Err(e) => {
                    tracing::error!("failed to retrieve article {id}: {e}");
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
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
                    NEWS_CACHE.clear().await;
                    return Redirect::to("/news/admin").into_response();
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

    tera_response(&tera, "content/news/edit.tera", &context)
}

pub struct MarkdownFilter;

impl tera::Filter for MarkdownFilter {
    fn filter(
        &self,
        value: &tera::Value,
        _args: &std::collections::HashMap<String, tera::Value>,
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
