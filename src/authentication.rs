use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use anyhow::Context;
use axum::{
    Form, Router,
    body::Body,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::{CookieJar, cookie::Cookie};
use deadpool_postgres::Pool;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

use crate::services::{User, UserStore};

const TOKEN_COOKIE: &str = "phrt-token";
const REFRESH_COOKIE: &str = "phrt-refresh";
const ENCODING_KEY: &str = "phrt:local:dev:token:do-not-use:";
const TOKEN_DURATION_SECS: u64 = 5;

fn create_cookie<'cookie, K, T>(key: K, token: T, duration: Duration) -> Cookie<'cookie>
where
    K: Into<String>,
    T: Into<String>,
{
    let duration = duration
        .try_into()
        .with_context(|| format!("Failed to convert duration into cookie duration {duration:?}"))
        .unwrap();

    Cookie::build((key.into(), token.into()))
        .path("/")
        .http_only(true)
        .max_age(duration)
        .secure(!cfg!(debug_assertions))
        .build()
}

fn token_secret() -> &'static [u8] {
    static ENV_VAL: OnceLock<Option<String>> = OnceLock::new();

    match ENV_VAL.get_or_init(|| match std::env::var("PHRT_TOKEN_SECRET") {
        Ok(value) => Some(value),
        _ => None,
    }) {
        Some(secret) => secret.as_bytes(),
        _ => ENCODING_KEY.as_bytes(),
    }
}

#[derive(Deserialize, Serialize)]
struct LoginInfo {
    email: Option<String>,
    password: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct UserClaims {
    user: String,
}

impl TryFrom<User> for UserClaims {
    type Error = serde_json::Error;
    fn try_from(value: User) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            user: serde_json::to_string(&value)?,
        })
    }
}

impl TryFrom<UserClaims> for User {
    type Error = serde_json::Error;

    fn try_from(value: UserClaims) -> std::result::Result<Self, Self::Error> {
        Ok(serde_json::from_str(&value.user)?)
    }
}

#[derive(bon::Builder)]
#[builder(finish_fn = build, on(String, into))]
pub struct TeraTemplate {
    #[builder(field)]
    context: tera::Context,
    #[builder(with = |tera: &tera::Tera| tera.clone())]
    tera: tera::Tera,
    template: String,
}

impl IntoResponse for TeraTemplate {
    fn into_response(self) -> Response {
        match self.tera.render(self.template.as_ref(), &self.context) {
            Ok(html) => {
                tracing::debug!(
                    context = ?self.context,
                    "successfully rendered template '{}'",
                    self.template
                );

                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/html")
                    .body(Body::from(html))
                    .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
            }
            Err(e) => {
                tracing::error!(
                    context = ?self.context,
                    error = ?e,
                    "failed to render template '{}'",
                    self.template
                );
                #[cfg(debug_assertions)]
                return axum_extra::response::InternalServerError(e).into_response();
                #[cfg(not(debug_assertions))]
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    }
}

impl<S> TeraTemplateBuilder<S>
where
    S: tera_template_builder::State,
{
    pub fn value<N, V>(mut self, name: N, value: &V) -> Self
    where
        N: AsRef<str>,
        V: Serialize,
    {
        self.context.insert(name.as_ref(), value);
        self
    }
}

struct LoginRouteState {
    user_store: UserStore,
    tera: Arc<tera::Tera>,
}

pub fn login_routes(database_pool: Pool, tera: Arc<tera::Tera>) -> Router {
    let state = Arc::new(LoginRouteState {
        user_store: UserStore::builder().database_pool(database_pool).build(),
        tera,
    });

    Router::new()
        .route("/login", get(login_get))
        .route("/login", post(login_post))
        .with_state(state)
}

async fn login_get(
    State(state): State<Arc<LoginRouteState>>,
    cookies: CookieJar,
) -> impl IntoResponse {
    (
        cookies.remove(TOKEN_COOKIE).remove(REFRESH_COOKIE),
        TeraTemplate::builder()
            .tera(&state.tera)
            .template("content/login.tera")
            .build(),
    )
        .into_response()
}

fn create_jwk_for_user(user: &User) -> anyhow::Result<String> {
    let encoding_key = EncodingKey::from_secret(token_secret());
    let user_claims = UserClaims::try_from(user.clone())?;
    let token = encode(&Header::default(), &user_claims, &encoding_key)
        .with_context(|| format!("failed to encode token for {user}"))?;
    Ok(token)
}

fn decode_jwk_into_user<J>(jwk: J) -> anyhow::Result<User>
where
    J: AsRef<str>,
{
    let decoding_key = DecodingKey::from_secret(token_secret());
    let validation = Validation::new(jsonwebtoken::Algorithm::HS512);

    let token_claims = decode::<UserClaims>(jwk.as_ref(), &decoding_key, &validation)
        .with_context(|| format!("failed to decode jwt :: {}", jwk.as_ref()))?;

    let user = serde_json::from_str(&token_claims.claims.user)
        .with_context(|| "failed to deserialize user from token claims")?;

    Ok(user)
}

#[axum::debug_handler]
async fn login_post(
    State(state): State<Arc<LoginRouteState>>,
    cookies: CookieJar,
    Form(login_form): Form<LoginInfo>,
) -> Response {
    let template = TeraTemplate::builder()
        .tera(&state.tera)
        .template("content/login.tera")
        .value("email", &login_form.email);

    let LoginInfo {
        email: Some(user_email),
        password: Some(user_password),
        to: _,
    } = login_form
    else {
        tracing::debug!("no login information was provided");
        // todo send template?
        return template.value("login_error", &true).build().into_response();
    };

    let user = match state.user_store.for_email(user_email.clone()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::debug!("failed to locate user '{user_email}'");
            return template.value("login_error", &true).build().into_response();
        }
        Err(e) => {
            tracing::error!("failed to retrieve the user from teh database: {e}");
            return template.value("login_error", &true).build().into_response();
        }
    };

    if !user.check_password(user_password) {
        tracing::debug!(
            user = ?user,
            "{user} failed to authenticate"
        );
        return template.value("login_error", &true).build().into_response();
    }

    let refresh_token = match state.user_store.refresh_token(&user).await {
        Ok(refresh_token) => refresh_token,
        Err(e) => {
            tracing::error!(
                error = ?e,
                user = ?user,
                "failed to retrieve {user} refresh token: {e}"
            );

            return template.value("login_error", &true).build().into_response();
        }
    };

    let redirect_to = login_form.to.unwrap_or_else(|| String::from("/"));
    let encoding_key = EncodingKey::from_secret(token_secret());
    let user_claims = UserClaims::try_from(user.clone()).unwrap();
    let token = encode(&Header::default(), &user_claims, &encoding_key).unwrap();

    tracing::info!(
        user = ?user,
        token = token,
        refresh = refresh_token,
        "authenticated user {user}",
    );

    (
        cookies
            .add(create_cookie(
                TOKEN_COOKIE,
                token,
                Duration::from_secs(TOKEN_DURATION_SECS),
            ))
            .add(create_cookie(
                REFRESH_COOKIE,
                refresh_token,
                Duration::from_secs(30),
            )),
        Redirect::to(&redirect_to),
    )
        .into_response()
}

fn get_cookies_from_request(
    request: &Request,
    cookies: &CookieJar,
) -> (Option<String>, Option<String>) {
    (
        cookies
            .get(TOKEN_COOKIE)
            .map(|cookie| cookie.value().to_owned()),
        cookies
            .get(REFRESH_COOKIE)
            .map(|cookie| cookie.value().to_owned()),
    )
}

/// This is a middleware function that will place the user into the request if the cookies exist and are valid
pub async fn authenticate(
    State(user_store): State<Arc<UserStore>>,
    mut request: Request,
    next: Next,
) -> impl IntoResponse {
    let cookies = Arc::new(CookieJar::from_headers(request.headers()));
    let (token_cookie, refresh_cookie) = get_cookies_from_request(&request, &cookies);

    let (user, new_cookies): (Option<User>, CookieJar) = if let Some(token) = token_cookie {
        match decode_jwk_into_user(token) {
            Ok(user) => {
                debug!("{user} was authenticated via jwk");
                (Some(user), CookieJar::clone(&cookies))
            }
            Err(e) => {
                tracing::error!("failed to retrieve user form the claims");
                (
                    None,
                    CookieJar::clone(&cookies)
                        .remove(TOKEN_COOKIE)
                        .remove(REFRESH_COOKIE),
                )
            }
        }
    } else if let Some(refresh) = refresh_cookie {
        match user_store.user_for_refresh_token(refresh).await {
            Ok(Some(user)) => match create_jwk_for_user(&user) {
                Ok(token) => (
                    Some(user),
                    CookieJar::from_headers(request.headers()).add(create_cookie(
                        TOKEN_COOKIE,
                        token,
                        Duration::from_secs(TOKEN_DURATION_SECS),
                    )),
                ),
                Err(e) => {
                    error!("failed to create jwk for {user}");
                    (None, CookieJar::clone(&cookies).remove(REFRESH_COOKIE))
                }
            },
            Ok(None) => {
                tracing::debug!("no user exists for the refresh token");
                (None, CookieJar::clone(&cookies).remove(REFRESH_COOKIE))
            }
            Err(e) => {
                tracing::debug!("query failed  to resolve user for refresh token: {e}");
                (None, CookieJar::clone(&cookies).remove(REFRESH_COOKIE))
            }
        }
    } else {
        (None, CookieJar::clone(&cookies))
    };

    request.extensions_mut().insert(user);
    let response = next.run(request).await;
    (new_cookies, response).into_response()
}
