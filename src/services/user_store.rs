use std::fmt::{Debug, Display};

use anyhow::{Context, Result, anyhow};
use base64::{Engine, prelude::BASE64_STANDARD};
use bon::Builder;
use deadpool_postgres::Pool;
use mockall::automock;
use postgres_from_row::FromRow;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, FromRow, Serialize, Deserialize, Builder)]
#[builder(on(_, required))]
pub struct User {
    id: Option<i64>,
    pub email: Option<String>,
    pub password: Option<Vec<u8>>,
}

#[derive(Builder)]
pub struct UserStore {
    database_pool: Pool,
}

/// Database agnostic storage for news items, also provides a mock for testing
#[automock]
impl UserStore {
    pub async fn get<I>(&self, id: I) -> Result<Option<User>>
    where
        I: Into<Uuid> + 'static,
    {
        let client = self.database_pool.get().await?;
        let id = id.into();
        match client
            .query_opt("SELECT * FROM users WHERE id = $1", &[&id])
            .await
            .with_context(|| format!("failed to with {id}"))?
        {
            Some(row) => Ok(Some(
                User::try_from_row(&row).map_err(|e| anyhow!("failed to deserialize row: {e}"))?,
            )),
            None => Ok(None),
        }
    }

    pub async fn for_email<E>(&self, email: E) -> Result<Option<User>>
    where
        E: Into<String> + 'static,
    {
        let email = email.into();
        let client = self.database_pool.get().await?;
        match client
            .query_opt("SELECT * FROM users WHERE email = $1", &[&email])
            .await
            .with_context(|| format!("failed to with {email}"))?
        {
            Some(row) => Ok(Some(
                User::try_from_row(&row).map_err(|e| anyhow!("failed to deserialize row: {e}"))?,
            )),
            None => Ok(None),
        }
    }

    async fn update(&self, user: &User) -> Result<User> {
        let mut client = self.database_pool.get().await?;
        let transaction = client.transaction().await?;

        transaction
            .execute(
                r#"
            UPDATE user SET
                email = $2,
                password = $3,
            WHERE
                id = $1
            "#,
                &[&user.id, &user.email, &user.password],
            )
            .await
            .with_context(|| format!("failed to insert news item = {user:?}"))?;

        transaction
            .execute("DELETE FROM refresh_tokens WHERE user_id = $1", &[&user.id])
            .await
            .with_context(|| format!("failed to remove user tokens {}", user.id.unwrap_or(0)))?;

        transaction.commit().await.with_context(|| {
            format!(
                "failed to commit transaction for user update {}",
                user.id.unwrap_or(0)
            )
        })?;

        Ok(user.clone())
    }

    async fn create(&self, user: &User) -> Result<User> {
        let client = self.database_pool.get().await?;
        let inserted_row = client
            .query_one(
                r#"
                INSERT INTO users
                    (email, password)
                VALUES
                    ($1, $2)
                RETURNING id;
                "#,
                &[&user.email, &user.password],
            )
            .await
            .with_context(|| format!("failed to insert news item into database item={user:?}"))?;

        let mut item = user.clone();
        item.id = Some(inserted_row.get(0));
        Ok(item)
    }

    pub async fn delete<I>(&self, id: I) -> Result<()>
    where
        I: Into<Uuid> + 'static,
    {
        let id = id.into();
        let client = self.database_pool.get().await?;
        client
            .execute("DELETE FROM user WHERE id = $1", &[&id])
            .await
            .with_context(|| format!("failed to delete news item id={id}"))?;
        Ok(())
    }

    pub async fn save(&self, user: &User) -> Result<User> {
        match user.id {
            Some(_id) => self.update(&user).await,
            None => self.create(&user).await,
        }
    }

    /// Create or retrieve a new refresh token for the user this is a unique identifier which
    /// is used to make sure we can auto refresh credentials whenever the user changes
    /// this value is deleted.
    pub async fn refresh_token(&self, user: &User) -> Result<String> {
        let client = self.database_pool.get().await?;

        let row = client
            .query_opt(
                r#"SELECT token FROM refresh_tokens WHERE user_id = $1"#,
                &[&user.id],
            )
            .await
            .with_context(|| {
                format!("failed to execute token query for {}", user.id.unwrap_or(0))
            })?;

        match row {
            Some(row) => {
                let token: Vec<u8> = row.try_get(0).with_context(|| {
                    format!("failed to extract token from row {}", user.id.unwrap_or(0))
                })?;
                Ok(BASE64_STANDARD.encode(&token))
            }
            None => {
                let mut token = [0u8; 512];
                rand::rng().fill_bytes(&mut token);

                client
                    .execute(
                        r#"INSERT INTO refresh_tokens (user_id, token) VALUES ($1, $2)"#,
                        &[&user.id, &token.to_vec()],
                    )
                    .await
                    .with_context(|| {
                        format!("failed to insert token for user {}", user.id.unwrap_or(0))
                    })?;

                Ok(BASE64_STANDARD.encode(token))
            }
        }
    }

    /// remove all the refresh tokens associated with teh user, there should only be one
    pub async fn clear_refresh_token(&self, user: &User) -> Result<()> {
        let user = user.clone();
        let client = self.database_pool.get().await?;
        client
            .execute("DELETE FROM refresh_tokens WHERE user_ud = $1", &[&user.id])
            .await
            .with_context(|| {
                format!("failed to delete tokens for user {}", user.id.unwrap_or(0))
            })?;
        Ok(())
    }

    pub async fn user_for_refresh_token<T>(&self, token: T) -> Result<Option<User>>
    where
        T: Into<String> + 'static,
    {
        let token: String = token.into();
        let client = self.database_pool.get().await?;
        let result = client
            .query_opt(
                r#"
            SELECT
                 users.* 
            FROM users JOIN 
                refresh_tokens 
            ON
                users.id = refresh_tokens.user_id
            WHERE 
                refresh_tokens.token = $1
            "#,
                &[&token],
            )
            .await
            .with_context(|| "failed to query user for refresh token")?;

        match result {
            Some(row) => Ok(Some(
                User::try_from_row(&row).with_context(|| "failed to create user from row")?,
            )),
            None => Ok(None),
        }
    }

    /*async fn query_filter<T>(&self, filters: &[(&str, &dyn ToSql)]) -> Result<Vec<User>> {
        let params = filters.iter().map(|(_, p)| p).collect();
        let clause =
            filters
                .iter()
                .enumerate()
                .fold(Vec::new(), |mut clauses, (index, (field, _))| {
                    clauses.push(format!("({} = {})", field, index + 1));
                    clauses
                });

        let client = self.database_pool.get().await?;
        let users = client.query(format!("SELECT * FROM users {}", clause.join(" AND ")), &params)
                .await
                .map_err(|e| anyhow!("failed to execute query: {e}"))
                .into_iter()
                .map(User::try_from_row()
                .collect::<std::result::Result<Vec<User>, _>>()
                .with_context(|| "failed to map users")?;

        Ok(users);
    }*/
}

impl User {
    /// Create a new item
    pub fn check_password<P>(&self, password: P) -> bool
    where
        P: AsRef<str>,
    {
        match self.password.as_ref() {
            Some(user_password) => {
                let password = password.as_ref().as_bytes();
                user_password == password
            }
            None => false,
        }
    }

    pub fn set_password<P>(&mut self, password: P) -> Result<()>
    where
        P: AsRef<str>,
    {
        let password = password.as_ref().as_bytes();
        self.password.replace(password.to_vec());
        Ok(())
    }
}

impl Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("User")
            .field("id", &self.id)
            .field("email", &self.email)
            .field("password", &String::from("<redacted>"))
            .finish()
    }
}

impl Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}({})",
            self.email
                .clone()
                .unwrap_or_else(|| String::from("<user_unknown>")),
            self.id.unwrap_or(0)
        )
    }
}

/*
use argon2::{Argon2, PasswordHasher, PasswordVerifier, password_hash::Salt};

let password = "password";

// This is the b64 hash of "bad salt!" for demo only: don't do this! Instead use:
// let salt = SaltString::generate(&mut OsRng);
let salt_str = "YmFkIHNhbHQh";
let salt: Salt = salt_str.try_into().unwrap();

let argon2 = Argon2::default();
let hash = argon2.hash_password(password.as_bytes(), salt).unwrap();

// This is the hash we will store. Notice our salt string is included, as well as parameters:
// version 0x13 (19), memory 19456KiB (19 MiB), 2 iterations (time), parallelism 1
let expected =
    "$argon2id$v=19$m=19456,t=2,p=1$YmFkIHNhbHQh$DqHGwv6NQV0VcaJi7jeF1E8IpfMXmXcpq4r2kKyqpXk";
//   ^ hash ^ parameters            ^ salt       ^ combined hash

assert_eq!(expected, hash.to_string());

// The verifier reads the salt and the parameters from the hash and verifies the result is equal
Argon2::default().verify_password(password.as_bytes(), &hash).expect("invalid password");

*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_check_password() {
        let user = User::builder()
            .id(None)
            .email(Some(String::from("test@example.com")))
            .password(Some(b"password".to_vec()))
            .build();
        assert!(user.check_password("password"));
    }
}
