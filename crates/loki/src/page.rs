use std::sync::Arc;

use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use bon::Builder;
use serde_json::{Map, Value};

use crate::{page_metadata::PageMetadata, registry::Registry};

#[derive(Builder)]
#[builder(on(_, required))]
pub struct Page {
    status_code: Option<StatusCode>,
    redirect: Option<String>,
    error_message: Option<String>,
    values: Option<Value>,
    template: Option<String>,
    layout: String,
    registry: Arc<Registry>,
}

//#[bon]
impl Page {
    /*#[builder(on(String, into))]
    pub fn new(
        #[builder(field)] value: Option<Value>,
        status_code: Option<StatusCode>,
        #[builder(with = |s: impl Into<String>| s.into())] template: Option<String>,
        layout: String,
    ) -> Self {
        Self {
            status_code: status_code,
            template: template,
            layout,
            values: value,
            ..Default::default()
        }
    }*/

    /*#[builder(finish_fn = build)]
    pub fn error(
        status_code: StatusCode,
        #[builder(with = |s: impl AsRef<str>| s.as_ref().to_owned())] error_message: Option<String>,
    ) -> Self {
        Self {
            status_code: Some(status_code),
            error_message,
            ..Default::default()
        }
    }*/

    /// Creates the metadata for this page, this combines the default, site, layout and template in
    /// that order returning the result. If any of those fail default or site will be returned
    fn create_metadata(&self) -> PageMetadata {
        let mut metadata = self.registry.default_metadata();
        let default_metadata = metadata.clone();

        if let Some(layout_metadata) = self.registry.find_metadata(&self.layout) {
            if let Err(e) = metadata.merge(&layout_metadata) {
                tracing::error!(
                    layout = ?layout_metadata,
                    metadata = ?metadata,
                    default = ?default_metadata,
                    error = ?e,
                    "failed to merge layout metadata {} into metadata returning default: {e}",
                    self.layout
                );
                return default_metadata;
            }
        }

        if let Some(template) = &self.template {
            if let Some(template_metadata) = self.registry.find_metadata(template) {
                if let Err(e) = metadata.merge(&template_metadata) {
                    tracing::error!(
                        metadata = ?metadata,
                        defaul = ?default_metadata,
                        template = ?template_metadata,
                        error = ?e,
                        "failed to merge template metadata into metadata: {e}"
                    );
                    return default_metadata;
                }
            }
        }

        metadata
    }
}

impl IntoResponse for Page {
    fn into_response(self) -> Response {
        // if we have a location issue a redirect
        if let Some(uri) = self.redirect {
            return Redirect::temporary(&uri).into_response();
        }

        // if we have a status code apply it, if its an error we have
        let status_code = self.status_code.unwrap_or(StatusCode::OK);
        if !status_code.is_success() {
            return Response::builder()
                .status(status_code)
                .header(header::CONTENT_TYPE, "text/html")
                .body(
                    self.error_message
                        .unwrap_or_else(|| status_code.to_string())
                        .into(),
                )
                .unwrap_or_else(|e| {
                    tracing::error!(
                        "failed to create error response {}: {e:?}",
                        status_code.as_u16()
                    );
                    status_code.into_response()
                });
        }

        let metadata = self.create_metadata();
        let mut values: Map<String, Value> = match self.values {
            None | Some(Value::Null) => Default::default(),
            Some(Value::Object(map)) => map.clone(),
            _ => {
                tracing::error!("expected object for values got :: {:?}", self.values);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        metadata.apply(&mut values).unwrap();
        values.insert(String::from("layout"), Value::String(self.layout.clone()));
        if let Some(template) = &self.template {
            values.insert(
                String::from("content_template"),
                Value::String(template.clone()),
            );

            let mut v = values.clone();
            v.insert(
                "errors".to_owned(),
                Value::String(String::from("------>why<--------")),
            );
            let jv = Value::Object(v);
            let x = self.registry.handlebars().render(template, &values);
            tracing::error!("------> {x:?}");
        }

        let body = self
            .registry
            .handlebars()
            .render(&self.layout, &values)
            .unwrap();

        tracing::debug!(
            layout = self.layout,
            template = ?self.template,
            values = ?values,
            "succesfully rendered page"
        );

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html")
            .body(body.into())
            .unwrap_or_else(|e| {
                tracing::error!(
                    template = ?self.template,
                    layaout = self.layout,
                    error = ?e,
                    "failed to render template"
                );
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            })
    }
}

/*
#[cfg(test)]
mod tests {
    use std::usize;

    use super::*;
    use axum::body::to_bytes;
    use serde_json::json;

    #[tokio::test]
    async fn error_response_500() {
        let response = Page::error()
            .status_code(StatusCode::INTERNAL_SERVER_ERROR)
            .build()
            .into_response();

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, response.status());
        let body = response.into_body();
        assert_eq!(
            StatusCode::INTERNAL_SERVER_ERROR.to_string().as_bytes(),
            to_bytes(body, usize::MAX).await.unwrap()
        );
    }

    #[tokio::test]
    async fn error_respnse_with_message() {
        let response = Page::error()
            .status_code(StatusCode::BAD_REQUEST)
            .error_message("invalid body")
            .build()
            .into_response();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body = response.into_body();
        assert_eq!(
            "invalid body".as_bytes(),
            to_bytes(body, usize::MAX).await.unwrap()
        );
    }

    #[test]
    fn page_values() {
        let page = Page::builder()
            .value("test", 7)
            .value("wilma", "flitstone")
            .layout("test")
            .build();

        assert_eq!(
            json!({
                "test": 7,
                "wilma": "flitstone",
            }),
            page.values.unwrap()
        );
    }
}*/
