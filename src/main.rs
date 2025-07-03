use anyhow::Context;
use axum::Extension;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::{config::Config, database::initialize_database};

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
        .with_target(true)
        .with_env_filter(EnvFilter::from_default_env())
        .compact()
        .try_init()
        .expect("failed to create formatting subscriber");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Config::parse();
    setup_tracing(&args);

    tracing::debug!(config = ?args, "starting new server");
    let database_pool = initialize_database(&args)
        .await
        .with_context(|| "failed to initalize the database")?;

    let app = routes::create_routes(&args, &database_pool).layer(Extension(database_pool.clone()));

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
