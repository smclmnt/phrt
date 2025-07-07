//#![feature(duration_constructors)]

use std::sync::Arc;

use anyhow::Context;
use axum::Extension;
use clap::Parser;
use tera::Tera;
use tracing_subscriber::EnvFilter;

use crate::{config::Config, database::initialize_database};

mod authentication;
mod config;
mod database;
mod migrations;
mod routes;
mod services;

fn setup_tracing(args: &Config) {
    tracing_subscriber::fmt()
        .with_ansi(!args.no_ansi)
        .with_level(true)
        .with_thread_ids(true)
        .with_target(cfg!(debug_assertions))
        .with_env_filter(EnvFilter::from_default_env())
        .compact()
        .try_init()
        .expect("failed to create formatting subscriber");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    #[cfg(debug_assertions)]
    dotenvy::dotenv().ok();
    let args = Config::parse();
    setup_tracing(&args);

    tracing::debug!(config = ?args, "starting new server");
    let database_pool = initialize_database(&args)
        .await
        .with_context(|| "failed to initialize the database")?;

    let mut tera = Tera::new(&format!("{}/**/*.tera", args.templates))
        .inspect_err(|e| {
            tracing::error!(
                directory = args.templates,
                "failed to register templates: {e}"
            );
        })
        .with_context(|| "failed to register templates")?;

    tera.autoescape_on(vec![".tera"]);
    tera.register_filter("markdown", routes::MarkdownFilter);

    let app = routes::create_routes(&args, &database_pool, Arc::new(tera.clone()))
        .layer(Extension(Arc::new(tera)))
        .layer(Extension(database_pool.clone()));

    loki::Application::run()
        .port(args.port)
        .extension(database_pool.clone())
        .assets(&args.asset_dir)
        .templates(&args.templates)
        .routes(app)
        .finish()
        .await?;

    database_pool.close();

    Ok(())
}
