use anyhow::{Context, Result, anyhow};
use bon::Builder;
use deadpool_postgres::Pool;
use mockall::automock;
use postgres_from_row::FromRow;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, Builder)]
#[builder(on(_, required))]
pub struct NewsItem {
    pub id: Option<i32>,
    pub title: String,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub hidden: bool,
}

#[derive(Builder)]
pub struct NewsStore {
    database_pool: Pool,
}

/// Database agnsotic storage for news items, also provides a mock for testing
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

    pub async fn get(&self, id: u32) -> Result<Option<NewsItem>> {
        let client = self.database_pool.get().await?;
        match client
            .query_opt("SELECT * FROM news WHERE id = $1", &[&id])
            .await
            .with_context(|| format!("failed to query news item {id}"))?
        {
            Some(row) => Ok(Some(
                NewsItem::try_from_row(&row)
                    .map_err(|e| anyhow!("failed to deserialzie row: {e}"))?,
            )),
            None => Ok(None),
        }
    }

    pub async fn update(&self, news_item: &NewsItem) -> Result<NewsItem> {
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

    pub async fn create(&self, news_item: &NewsItem) -> Result<NewsItem> {
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
                format!("failed to isnert news item into database item={news_item:?}")
            })?;

        let mut item = news_item.clone();
        item.id = Some(inserted_row.get(0));
        Ok(item)
    }

    pub async fn delete(&self, id: i32) -> Result<()> {
        let client = self.database_pool.get().await?;
        client
            .execute("DELETE FROM news WHERE id = $1", &[&id])
            .await
            .with_context(|| format!("failed to delete news item id={id}"))?;
        Ok(())
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
