use anyhow::{Context, Result, anyhow};
use bon::Builder;
use deadpool_postgres::Pool;
use mockall::automock;
use postgres_from_row::FromRow;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, Builder)]
#[builder(on(_, required))]
pub struct NewsItem {
    pub id: Option<i64>,
    pub title: String,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub hidden: bool,
}

#[derive(Builder)]
pub struct NewsStore {
    database_pool: Pool,
}

/*
todo future:

pub trait Store<T>
where
    T: FromRow + std::fmt::Debug + Clone,
{
    async fn t_delete(&self, item: &T) -> Result<()>;
    async fn t_create(&self, item: &T) -> Result<T>;
    async fn t_update(&self, item: &T) -> Result<T>;
    async fn t_all(&self) -> Result<Vec<T>>;
}

trait Identity {
    type PrimaryKey;
    fn has_id(&self) -> bool;
}

pub trait StoreExt<T>: Store<T>
where
    T: FromRow + std::fmt::Debug + Clone + Identity,
{
    async fn t_save(&self, item: &T) -> Result<T> {
        if item.has_id() {
            self.t_update(item).await
        } else {
            self.t_create(item).await
        }
    }
}*/

/// Database agnostic storage for news items, also provides a mock for testing
#[automock]
impl NewsStore {
    pub async fn all(&self) -> Result<Vec<NewsItem>> {
        let client = self.database_pool.get().await?;
        client
            .query("SELECT * FROM news ORDER BY id", &[])
            .await
            .map_err(|e| anyhow!("failed to query news: {e}"))
            .and_then(|rows| {
                rows.iter()
                    .map(NewsItem::try_from_row)
                    .collect::<std::result::Result<Vec<NewsItem>, _>>()
                    .map_err(|e| anyhow!("failed to convert rows: {e}"))
            })
    }

    pub async fn get(&self, id: i64) -> Result<Option<NewsItem>> {
        let client = self.database_pool.get().await?;
        match client
            .query_opt("SELECT * FROM news WHERE id = $1", &[&id])
            .await
            .with_context(|| format!("failed to query news item {id}"))?
        {
            Some(row) => Ok(Some(
                NewsItem::try_from_row(&row)
                    .map_err(|e| anyhow!("failed to deserialize row: {e}"))?,
            )),
            None => Ok(None),
        }
    }

    async fn update(&self, news_item: &NewsItem) -> Result<NewsItem> {
        let client = self.database_pool.get().await?;
        client
            .execute(
                r#"
            UPDATE news SET
                title = $2,
                url = $3,
                notes = $4,
                hidden = $5
            WHERE
                id = $1
            "#,
                &[
                    &news_item.id,
                    &news_item.title,
                    &news_item.url,
                    &news_item.notes,
                    &news_item.hidden,
                ],
            )
            .await
            .with_context(|| format!("failed to insert news item = {news_item:?}"))
            .map(|_| news_item.clone())
    }

    async fn create(&self, news_item: &NewsItem) -> Result<NewsItem> {
        let client = self.database_pool.get().await?;
        let inserted_row = client
            .query_one(
                r#"
                INSERT INTO news
                    (title, hidden, notes, url)
                VALUES
                    ($1, $2, $3, $4)
                RETURNING id;
                "#,
                &[
                    &news_item.title,
                    &news_item.hidden,
                    &news_item.notes,
                    &news_item.url,
                ],
            )
            .await
            .with_context(|| {
                format!("failed to insert news item into database item={news_item:?}")
            })?;

        let mut item = news_item.clone();
        item.id = Some(inserted_row.get(0));
        Ok(item)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let client = self.database_pool.get().await?;
        client
            .execute("DELETE FROM news WHERE id = $1", &[&id])
            .await
            .with_context(|| format!("failed to delete news item id={id}"))?;
        Ok(())
    }

    pub async fn save(&self, news_item: &NewsItem) -> Result<NewsItem> {
        match news_item.id {
            Some(_id) => {
                tracing::debug!(
                    article = ?news_item,
                    "updating existing article {}",
                    news_item.id.clone().unwrap_or(0)
                );
                self.update(&news_item).await
            }
            None => {
                tracing::debug!(article = ?news_item, "creating new article");
                self.create(&news_item).await
            }
        }
    }
}

impl NewsItem {
    /// Create a new item
    pub fn for_create() -> Self {
        Self::builder()
            .id(None)
            .title(String::default())
            .notes(None)
            .url(None)
            .hidden(false)
            .build()
    }
}
