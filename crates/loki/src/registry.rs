use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs::{DirEntry, read_dir},
    path::PathBuf,
};

use anyhow::{Context, Result, bail};
use handlebars::Handlebars;
use tracing::instrument;

use crate::page_metadata::PageMetadata;

pub struct Registry {
    page_metatdata: BTreeMap<String, PageMetadata>,
    handlebars: Handlebars<'static>,
    site_metadata: Option<String>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            page_metatdata: BTreeMap::new(),
            handlebars: Handlebars::new(),
            site_metadata: None,
        }
    }

    pub fn set_site_metadata(&mut self, metadata: Option<String>) {
        self.site_metadata = metadata
    }

    #[instrument(level = "info", skip_all, fields(directory = directory.as_ref()))]
    pub fn register_directory<S>(&mut self, directory: S) -> Result<()>
    where
        S: AsRef<str>,
    {
        let path = PathBuf::from(directory.as_ref());
        if !path.exists() || !path.is_dir() {
            bail!("either {path:?} does not exists or not a directory");
        }

        let prefix = path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or(
                directory
                    .as_ref()
                    .trim_start_matches(['.', '/'])
                    .trim_end_matches('/'),
            )
            .to_string();

        self.visit_directory(&path, &[prefix])
    }

    pub fn has_metadata<S>(&self, page: S) -> bool
    where
        S: AsRef<str>,
    {
        self.page_metatdata.contains_key(page.as_ref())
    }

    pub fn default_metadata(&self) -> PageMetadata {
        match &self.site_metadata {
            Some(site_metadata) => self
                .page_metatdata
                .get(site_metadata)
                .cloned()
                .unwrap_or_default(),
            None => PageMetadata::default(),
        }
    }

    pub fn find_metadata<S>(&self, page: S) -> Option<PageMetadata>
    where
        S: AsRef<str>,
    {
        self.page_metatdata.get(page.as_ref()).cloned()
    }

    pub fn has_template<S>(&self, page: S) -> bool
    where
        S: AsRef<str>,
    {
        self.handlebars.has_template(page.as_ref())
    }

    pub fn handlebars<'h>(&'h self) -> &'h Handlebars<'h> {
        &self.handlebars
    }

    fn visit_directory(&mut self, directory: &PathBuf, prefix: &[String]) -> Result<()> {
        for entry in read_dir(directory)
            .with_context(|| format!("failed to read directory {directory:?}"))?
        {
            let Ok(entry) = entry else {
                continue;
            };

            let path = entry.path();
            if path.is_dir() {
                let mut prefix = prefix.to_vec();
                prefix.extend(
                    path.file_stem()
                        .and_then(OsStr::to_str)
                        .map(ToOwned::to_owned),
                );
                self.visit_directory(&path.to_path_buf(), &prefix)?;
            } else if path.is_file() {
                match entry.path().extension().and_then(OsStr::to_str) {
                    Some("hbs") => {
                        let name = Self::create_name(&entry, prefix)?;
                        tracing::info!(
                            path = ?entry.path().canonicalize().unwrap_or_else(|_| entry.path()),
                            "registering template '{name}'"
                        );
                        self.handlebars
                            .register_template_file(&name, &path)
                            .with_context(|| format!("failed to register template '{name}]"))?;
                    }
                    Some("json") => {
                        let name = Self::create_name(&entry, prefix)?;
                        tracing::info!(
                            path = ?entry.path().canonicalize().unwrap_or_else(|_| entry.path()),
                            "registering page metadata '{name}'"
                        );
                        let page_metadata = PageMetadata::try_from(&entry.path())
                            .with_context(|| format!("failed to load page metadata '{name}"))?;
                        self.page_metatdata.insert(name, page_metadata);
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn create_name(entry: &DirEntry, prefix: &[String]) -> Result<String> {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(OsStr::to_str) else {
            bail!("entry {:?} has no file stem", entry)
        };

        Ok(prefix
            .iter()
            .cloned()
            .chain(Some(stem.to_owned()))
            .collect::<Vec<String>>()
            .join("/"))
    }
}

/*

fn register_directory(
    directory: &PathBuf,
    prefix: &Vec<String>,
    handlebaers: &mut Handlebars,
) -> Result<()> {
    for entry in read_dir(directory).with_context(|| "failed to read directory")? {
        match &entry {
            Ok(entry) => {
                let path = entry.path();
                if entry.path().is_dir() {
                    let mut prefix = prefix.clone();
                    if let Some(path) = path.file_stem() {
                        prefix.push(path.to_string_lossy().to_ascii_lowercase());
                    }
                    register_directory(&entry.path(), &prefix, handlebaers)?;
                } else {
                    register_template(&entry, prefix, handlebaers)?;
                }
            }
            Err(e) => {
                tracing::warn!("failed to get directory entry: {}", e.to_string());
            }
        }
    }

    Ok(())
}

fn register_template(
    dir_entry: &DirEntry,
    prefix: &Vec<String>,
    handlebaers: &mut Handlebars,
) -> Result<()> {
    let path = dir_entry.path();
    let Some(file_stem) = path.file_stem() else {
        bail!("uanble to determine file stem: {dir_entry:?}");
    };

    let file_stem = file_stem.to_string_lossy().to_ascii_lowercase();
    let mut template_name = prefix.clone();
    template_name.push(file_stem);
    let template_name = template_name.join("-");

    tracing::debug!("registered template '{}' -> {:?}", template_name, path);
    handlebaers
        .register_template_file(&template_name, path)
        .with_context(|| format!("failed to register template: {dir_entry:?}"))?;

    Ok(())
}
*/
