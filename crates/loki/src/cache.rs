use std::fmt::Debug;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::sync::Mutex;

pub struct Cache<T> {
    ttl: Duration,
    cache: Mutex<(DateTime<Utc>, Option<T>)>,
}

///
/// Implements a simple cache with a TTL as well as the ability to clear, should be
/// used to prevert refetching data when it's fairly static especially when the cahnges
/// can clear the cache.
///
impl<T> Cache<T>
where
    T: Clone + Debug,
{
    pub const fn const_once() -> Self {
        Self {
            ttl: Duration::ZERO,
            cache: Mutex::const_new((DateTime::<Utc>::MAX_UTC, None)),
        }
    }

    pub const fn const_always() -> Self {
        Self {
            ttl: Duration::ZERO,
            cache: Mutex::const_new((DateTime::<Utc>::MIN_UTC, None)),
        }
    }

    pub const fn const_ttl(ttl: Duration) -> Self {
        Self {
            ttl,
            cache: Mutex::const_new((DateTime::<Utc>::MIN_UTC, None)),
        }
    }

    pub async fn expired(&self) -> bool {
        let cache = self.cache.lock().await;
        Utc::now() > cache.0
    }

    pub async fn clear(&self) {
        let mut cache = self.cache.lock().await;
        cache.1.take();
    }

    pub async fn try_fetch<F, R, E>(&self, op: F) -> Result<T, E>
    where
        R: Into<T>,
        F: Future<Output = Result<R, E>> + 'static,
    {
        let mut cache = self.cache.lock().await;
        if Utc::now() < cache.0 && cache.1.is_some() {
            return Ok(unsafe { cache.1.clone().unwrap_unchecked() });
        }
        match op.await {
            Ok(result) => {
                let value = result.into();
                cache.1.replace(value.clone());
                if self.ttl != Duration::ZERO {
                    cache.0 = Utc::now() + self.ttl;
                }
                Ok(value)
            }
            Err(e) => {
                cache.1.take();
                Err(e)
            }
        }
    }

    pub async fn fetch<F, R>(&self, op: F) -> T
    where
        F: Future<Output = R> + 'static,
        R: Into<T>,
        F::Output: Into<T>,
    {
        let mut cache = self.cache.lock().await;
        if Utc::now() < cache.0 && cache.1.is_some() {
            return cache.1.clone().unwrap();
        }

        let result = op.await.into();
        cache.1.replace(result.clone());
        if self.ttl != Duration::ZERO {
            cache.0 = Utc::now() + self.ttl;
        }

        result
    }

    pub async fn try_fetch_or_default<F, R, E>(&self, op: F) -> Result<T, E>
    where
        R: Into<T>,
        F: Future<Output = Result<R, E>> + 'static,
        T: Default,
    {
        let mut cache = self.cache.lock().await;
        if Utc::now() < cache.0 && cache.1.is_some() {
            return Ok(unsafe { cache.1.clone().unwrap_unchecked() });
        }

        match op.await {
            Ok(result) => {
                let value = result.into();
                cache.1.replace(value.clone());
                if self.ttl != Duration::ZERO {
                    cache.0 = Utc::now() + self.ttl;
                }
                Ok(value)
            }
            Err(_) => {
                cache.1.take();
                if self.ttl != Duration::ZERO {
                    cache.0 = DateTime::<Utc>::MIN_UTC;
                }
                Ok(T::default())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::{sync::Arc, time::Duration};

    use anyhow::{Result, anyhow};
    use mockall::automock;

    use super::Cache;

    #[automock]
    trait Fetch {
        fn fetch(&self) -> &'static str;
        fn try_fetch(&self) -> Result<&'static str>;
    }

    #[tokio::test]
    async fn fetch_once() {
        let cache = Cache::<String>::const_once();
        let mut mock = MockFetch::new();
        mock.expect_fetch().once().return_const("test-mock");

        let mock = Arc::new(mock);
        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache.fetch(async move { mock_clone.fetch() }).await
        );

        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache.fetch(async move { mock_clone.fetch() }).await
        );
    }

    #[tokio::test]
    async fn try_fetch_once() {
        let cache = Cache::<String>::const_once();
        let mut mock: MockFetch = MockFetch::new();
        mock.expect_try_fetch().once().returning(|| Ok("test-mock"));

        let mock = Arc::new(mock);
        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache
                .try_fetch(async move { mock_clone.try_fetch() })
                .await
                .unwrap()
        );

        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache
                .try_fetch(async move { mock_clone.try_fetch() })
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn fetch_always() {
        let cache = Cache::<String>::const_always();
        let mut mock = MockFetch::new();
        mock.expect_fetch().times(2).return_const("test-mock");

        let mock = Arc::new(mock);
        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache.fetch(async move { mock_clone.fetch() }).await
        );

        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache.fetch(async move { mock_clone.fetch() }).await
        );
    }

    #[tokio::test]
    async fn try_fetch_always() {
        let cache = Cache::<String>::const_always();
        let mut mock = MockFetch::new();
        mock.expect_try_fetch()
            .times(2)
            .returning(|| Ok("test-mock"));

        let mock = Arc::new(mock);
        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache
                .try_fetch(async move { mock_clone.try_fetch() })
                .await
                .unwrap()
        );

        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache
                .try_fetch(async move { mock_clone.try_fetch() })
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn fetch_ttl() {
        let cache = Cache::<String>::const_ttl(Duration::from_millis(100));
        let mut mock = MockFetch::new();
        mock.expect_fetch().times(2).return_const("test-mock");

        let mock = Arc::new(mock);
        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache.fetch(async move { mock_clone.fetch() }).await
        );

        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache.fetch(async move { mock_clone.fetch() }).await
        );

        tokio::time::sleep(Duration::from_millis(500)).await;

        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache.fetch(async move { mock_clone.fetch() }).await
        );
    }

    #[tokio::test]
    async fn try_fetch_ttl() {
        let cache = Cache::<String>::const_ttl(Duration::from_millis(100));
        let mut mock = MockFetch::new();
        mock.expect_try_fetch()
            .times(2)
            .returning(|| Ok("test-mock"));

        let mock = Arc::new(mock);
        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache
                .try_fetch(async move { mock_clone.try_fetch() })
                .await
                .unwrap()
        );

        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache
                .try_fetch(async move { mock_clone.try_fetch() })
                .await
                .unwrap()
        );

        tokio::time::sleep(Duration::from_millis(500)).await;

        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache
                .try_fetch(async move { mock_clone.try_fetch() })
                .await
                .unwrap()
        );

        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache
                .try_fetch(async move { mock_clone.try_fetch() })
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    #[should_panic]
    async fn try_fetch_fail() {
        let cache = Cache::<String>::const_once();
        let mut mock: MockFetch = MockFetch::new();
        mock.expect_try_fetch()
            .once()
            .returning(|| Err(anyhow!("err")));

        let mock = Arc::new(mock);
        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache
                .try_fetch(async move { mock_clone.try_fetch() })
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn try_fetch_or_default() {
        let cache = Cache::<String>::const_once();
        let mut mock: MockFetch = MockFetch::new();
        mock.expect_try_fetch()
            .once()
            .returning(|| Err(anyhow!("err")));

        let mock = Arc::new(mock);
        let mock_clone = mock.clone();
        assert_eq!(
            String::default(),
            cache
                .try_fetch_or_default(async move { mock_clone.try_fetch() })
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn fetch_overalpped() {
        let cache = Cache::<i32>::const_ttl(Duration::from_secs(1));

        let fetch_one = cache
            .fetch(async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                7
            })
            .await;

        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(fetch_one, cache.fetch(async { 11 }).await);
    }

    #[tokio::test]
    async fn clear() {
        let cache = Cache::<String>::const_once();
        let mut mock = MockFetch::new();
        mock.expect_fetch().times(2).return_const("test-mock");

        let mock = Arc::new(mock);
        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache.fetch(async move { mock_clone.fetch() }).await
        );

        cache.clear().await;

        let mock_clone = mock.clone();
        assert_eq!(
            "test-mock",
            cache.fetch(async move { mock_clone.fetch() }).await
        );
    }
}
