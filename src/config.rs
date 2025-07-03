use clap::Parser;

#[derive(Debug, Parser)]
pub struct Config {
    #[arg(long, default_value_t = false, env = "NO_ANSI")]
    pub no_ansi: bool,
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,
    #[arg(long, env = "MAX_CONNECTIONS", default_value_t = 5)]
    pub max_connections: u32,
    #[arg(long, env = "PORT", default_value_t = 80)]
    pub port: u16,
    #[arg(long, env = "TEMPLATES", default_value_t = String::from("./templates"))]
    pub templates: String,
    #[arg(long, env = "ASSET_DIR", default_value_t = String::from("./assets"))]
    pub asset_dir: String,

    /// Causes the application to invoke a full reset on the datbase, revert everything
    /// then reapply the migrations, this *CAN* cause data loss
    #[arg(long, default_value_t = false)]
    pub reset_datbase: bool,

    /// Cause the server to revert the database by the specified number of revisions before
    /// executing the migration, can be useful for recovery
    #[arg(long)]
    pub revert_database: Option<usize>,
}
