use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Deserialize, Debug, Clone, Default)]
pub struct PageMetadata {
    title: Option<String>,
    description: Option<String>,
    keywords: Vec<String>,
}

impl PageMetadata {
    pub fn apply(&self, value: &mut Map<String, Value>) -> Result<()> {
        if let Some(title) = self.title.clone() {
            value.insert(String::from("title"), Value::String(title));
        }

        if let Some(description) = self.description.clone() {
            value.insert(String::from("description"), Value::String(description));
        }

        if self.keywords.is_empty() {
            value.insert(String::from("keywords"), Value::Null);
        } else {
            let keywords = self.keywords.join(",");
            value.insert(String::from("keywords"), Value::String(keywords));
        }

        Ok(())
    }

    pub fn merge(&mut self, other: &Self) -> Result<()> {
        self.title = match (&self.title, &other.title) {
            (Some(title), None) => Some(title.clone()),
            (None, Some(title)) => Some(title.clone()),
            (Some(title), Some(sub_title)) => Some(format!("{sub_title} - {title}")),
            _ => None,
        };

        self.description = match (&self.description, &other.description) {
            (Some(descr), None) => Some(descr.to_owned()),
            (Some(descr), Some(other_descr)) => {
                let mut other_descr = other_descr.clone();
                other_descr.push(' ');
                other_descr.push_str(&descr);
                Some(other_descr)
            }
            (None, Some(descr)) => Some(descr.to_owned()),
            _ => None,
        };

        if !other.keywords.is_empty() {
            let other_keywords = other
                .keywords
                .iter()
                .map(String::as_str)
                .map(str::to_lowercase)
                .collect::<Vec<String>>();

            let mut keywords = self
                .keywords
                .iter()
                .map(String::as_str)
                .map(str::to_lowercase)
                .collect::<Vec<String>>();

            keywords.extend(other_keywords);
            keywords.sort();
            keywords.dedup();
            self.keywords = keywords;
        }

        Ok(())
    }
}

impl TryFrom<&Path> for PageMetadata {
    type Error = anyhow::Error;

    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        let mut path = PathBuf::from(value);
        path.set_extension("json");

        return if path.exists() {
            tracing::debug!("laoding page metadata {path:?}");
            let reader = std::fs::File::open(&path)
                .with_context(|| format!("failed to open file {path:?}"))?;
            Ok(serde_json::from_reader(reader)
                .with_context(|| format!("failed to parse metadata {path:?}"))?)
        } else {
            Ok(Self::default())
        };
    }
}

impl TryFrom<&PathBuf> for PageMetadata {
    type Error = anyhow::Error;

    fn try_from(value: &PathBuf) -> Result<Self, Self::Error> {
        Self::try_from(value.as_path())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn simple() {}

    #[test]
    fn other() {}
}
