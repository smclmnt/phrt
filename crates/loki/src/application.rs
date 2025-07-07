use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use axum::{Extension, Router};
use bon::bon;
use tower_http::services::ServeDir;

use crate::{page_builder::PageBuilder, registry::Registry};

pub struct Application {}

#[bon]
impl Application {
    #[builder(finish_fn = finish)]
    pub async fn run(
        #[builder(field)] assets: Vec<String>,
        #[builder(field)] templates: Vec<String>,
        #[builder(field)] routes: Router,
        port: u16,
    ) -> Result<()> {
        let mut registry = Registry::new();
        templates
            .iter()
            .map(|template_dir| registry.register_directory(template_dir))
            .collect::<std::result::Result<Vec<()>, _>>()
            .with_context(|| "failed to register templates")?;

        let routes = assets.into_iter().try_fold(routes, |routes, asset_dir| {
            let asset_path = PathBuf::from(asset_dir);
            if !asset_path.exists() {
                bail!("Unable to locate assets {asset_path:?}");
            }

            let Some(assets_path) = asset_path.file_stem() else {
                bail!("unable to locate name of the assets dir {asset_path:?}");
            };

            let asset_path = asset_path.canonicalize().unwrap_or(asset_path.clone());
            tracing::info!("adding assets dir /{assets_path:?} --> {asset_path:?}");
            let assets_service = ServeDir::new("./assets");
            Ok(routes.nest_service("/assets", assets_service))
        })?;

        let routes = routes.layer(Extension(PageBuilder::new(registry)));
        crate::server::Server::serve(port, routes).await?;
        Ok(())
    }
}

impl<S> ApplicationRunBuilder<S>
where
    S: application_run_builder::State,
{
    pub fn assets<A>(mut self, asset: A) -> Self
    where
        A: Into<String>,
    {
        self.assets.push(asset.into());
        self
    }

    pub fn templates<T>(mut self, templates: T) -> Self
    where
        T: Into<String>,
    {
        self.templates.push(templates.into());
        self
    }

    pub fn extension<E>(mut self, extension: E) -> Self
    where
        E: Clone + Send + Sync + 'static,
    {
        self.routes = self.routes.layer(Extension(extension));
        self
    }

    pub fn routes(mut self, routes: Router) -> Self {
        self.routes = self.routes.merge(routes);
        self
    }
}
