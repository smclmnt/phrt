pub mod application;
pub mod cache;
pub mod page;
mod page_builder;
mod page_metadata;
mod registry;
pub mod server;

pub use application::Application;
pub use cache::Cache;
pub use page::Page;
pub use page_builder::PageBuilder;
pub use page_builder::PageError;

pub type PageResult = std::result::Result<Page, PageError>;
