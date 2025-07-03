use std::{net::SocketAddr, time::Duration};

use anyhow::{Context, Result};
use axum::{Router, http::Request};
use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
};
use tower::ServiceBuilder;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};
use tracing::Span;

pub struct Server;

impl Server {
    pub async fn serve(port: u16, app: Router) -> Result<()> {
        let app = Self::apply_layers(app);

        let address = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = TcpListener::bind(address)
            .await
            .with_context(|| format!("failed to bind to {address}"))?;

        tracing::info!(
            "serving traffic on {:?}",
            listener.local_addr().unwrap_or(address)
        );

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(Self::monitor_shutdown())
        .await?;

        Ok(())
    }

    fn apply_layers(app: Router) -> Router {
        app.layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(|request: &Request<_>| match request.method().as_str() {
                            "GET" => tracing::debug_span!("http_get"),
                            "POST" => tracing::debug_span!("http_post"),
                            "PATCH" => tracing::debug_span!("http_patch"),
                            "OPTIONS" => tracing::debug_span!("http_options"),
                            method => tracing::debug_span!("http", method = ?method),
                        })
                        .on_request(|request: &Request<_>, _span: &Span| {
                            tracing::debug!("request {} '{}'", request.method(), request.uri());
                        }),
                )
                .layer(TimeoutLayer::new(Duration::from_secs(45))),
        )
    }

    /// Moinitors external shutdown sources to close the server
    async fn monitor_shutdown() {
        let sigterm = async {
            signal(SignalKind::terminate())
                .expect("unable to install a termination handler")
                .recv()
                .await
        };

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("received ctrl+c shutting donw");
            },
            _ = sigterm => {
                tracing::info!("received SIGTERM shutting down");
            }
        }
    }
}
