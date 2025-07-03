use std::fmt::Debug;
use std::sync::Arc;

use crate::registry::Registry;
use crate::{Page, PageResult};
use axum::body::Body;
use axum::http::{Response, StatusCode, header};
use axum::response::IntoResponse;
use serde::Serialize;
use serde_json::{Map, Value};
use thiserror::Error;

#[derive(Clone)]
pub struct PageBuilder {
    registry: Arc<Registry>,
}

impl PageBuilder {
    pub(crate) fn new(registry: Registry) -> Self {
        Self {
            registry: Arc::new(registry),
        }
    }

    pub fn html(self) -> HtmlPageBuilder {
        HtmlPageBuilder::new(self.registry.clone())
    }

    pub fn raw_html<H>(&self, html: H) -> Response<Body>
    where
        H: Into<String>,
    {
        let result = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html")
            .body(Body::from(html.into()));

        match result {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("failed to generate page from html: {e}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

/*impl<S> PageBuilderHtmlBuilder<S>
where
    S: page_builder_html_builder::State,
{
    pub fn value<K, V>(mut self, name: K, value: V) -> Self
    where
        K: Into<String>,
        V: serde::Serialize,
    {
        let values = self
            .value
            .get_or_insert(Value::Object(Default::default()))
            .as_object_mut()
            .expect("expected serde_json::Object::Value");

        values.insert(
            name.into(),
            serde_json::to_value(value).expect("failed to serialzie value"),
        );

        self
    }
}*/

pub struct HtmlPageBuilder {
    registry: Arc<Registry>,
    page_values: Map<String, Value>,
    status: Option<StatusCode>,
    layout: Option<String>,
    template: Option<String>,
}

impl Debug for HtmlPageBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HtmlPage")
            .field("layout", &self.layout)
            .field("template", &self.template)
            .field("status", &self.status)
            .field("values", &self.page_values)
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum PageError {
    #[error("Page are required to provide a lyout")]
    NoLayout,
    #[error("The provided layout ({0}) as invalid")]
    LayoutNotFound(String),
    #[error("The template {1} was not found for a page using layout {1}")]
    TemplateNotFound(String, String),
}

impl IntoResponse for PageError {
    fn into_response(self) -> axum::response::Response {
        tracing::error!(
            error = self.to_string(),
            "generating an internal server error page"
        );
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(self.to_string().into())
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
    }
}

impl HtmlPageBuilder {
    fn new(registry: Arc<Registry>) -> Self {
        Self {
            registry: registry,
            page_values: Map::new(),
            status: None,
            layout: None,
            template: None,
        }
    }

    pub fn status(mut self, status: StatusCode) -> Self {
        self.status.replace(status);
        self
    }

    pub fn layout<L>(mut self, layout: L) -> Self
    where
        L: Into<String>,
    {
        self.layout.replace(layout.into());
        self
    }

    pub fn template<T>(mut self, template: T) -> Self
    where
        T: Into<String>,
    {
        self.template.replace(template.into());
        self
    }

    pub fn value<S, V>(mut self, name: S, value: V) -> Self
    where
        V: Serialize,
        S: Into<String>,
    {
        let value = serde_json::to_value(&value).expect("failed to serialize value");
        self.page_values.insert(name.into(), value);
        self
    }

    pub fn try_value<V, S>(
        mut self,
        name: S,
        value: V,
    ) -> std::result::Result<Self, serde_json::Error>
    where
        V: Serialize,
        S: Into<String>,
    {
        let value = serde_json::to_value(&value)?;
        self.page_values.insert(name.into(), value);
        Ok(self)
    }

    pub fn send(self) -> PageResult {
        let values = if self.page_values.is_empty() {
            None
        } else {
            Some(Value::Object(self.page_values))
        };

        let Some(layout) = &self.layout else {
            tracing::error!("pages must have a valid layout");
            return Err(PageError::NoLayout);
        };

        if !self.registry.has_metadata(layout) {
            tracing::error!("layout '{layout}' was not found");
            return Err(PageError::LayoutNotFound(layout.clone()));
        }

        if let Some(template) = &self.template {
            if !self.registry.has_template(template) {
                tracing::error!("page references an invalid template '{template}");
                return Err(PageError::TemplateNotFound(
                    layout.clone(),
                    template.clone(),
                ));
            }
        }

        Ok(Page::builder()
            .registry(self.registry)
            .status_code(self.status)
            .layout(layout.clone())
            .template(self.template)
            .values(values)
            .redirect(None)
            .error_message(None)
            .build())
    }
}
