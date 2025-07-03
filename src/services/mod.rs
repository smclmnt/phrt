mod news_store;

pub use news_store::{NewsItem, NewsStore};

#[cfg(test)]
pub use news_store::MockNewsStore;
