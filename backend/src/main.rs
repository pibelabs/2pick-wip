mod email_service;

use std::env;

use axum::{
    Json, Router,
    extract::{FromRef, Path, Request, State},
    http::{StatusCode, Uri, header::CONTENT_TYPE},
    middleware,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use email_address::EmailAddress;
use minijinja::context;
use serde::Deserialize;
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

use crate::email_service::{EmailPacket, EmailService};

static DB_ADDRESS: &str = "sqlite://db.sqlite3?mode=rwc";

#[derive(FromRef, Clone)]
struct AppState {
    db: SqlitePool,
    email_svc: EmailService,
}

use axum::middleware::Next;

async fn add_html_ext(mut req: Request, next: Next) -> Response {
    let uri = req.uri().clone();
    let path = uri.path();

    // only rewrite if no extension and no trailing slash
    if !path.contains('.') && !path.ends_with('/') && !path.starts_with("/api") {
        let new_path = format!("{}.html", path);
        let mut parts = uri.into_parts();
        parts.path_and_query = Some(new_path.parse().unwrap());
        *req.uri_mut() = Uri::from_parts(parts).unwrap();
    }

    next.run(req).await
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let now = chrono::Utc::now();
    let file_appender = tracing_appender::rolling::never("log", "latest.log");
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(fmt::layer().with_writer(file_writer).with_ansi(false))
        .with(fmt::layer())
        .init();

    tracing::info!("Initialized logger");

    tracing::info!("Getting Resend API key...");
    let resend_api_key = env::var("RESEND_API_KEY")?;

    tracing::info!("Trying to bind listener to 0.0.0.0:8001...");
    let tcp_listener = TcpListener::bind("0.0.0.0:8001").await?;
    tracing::info!("Bound listener");

    tracing::info!("Attempting to open DB...");
    let db = sqlx::SqlitePool::connect(DB_ADDRESS).await?;
    tracing::info!("Opened DB");

    let email_svc = EmailService::new(
        resend_api_key,
        EmailAddress::new_unchecked("noreply@2pick.de"),
    )
    .await?;

    let api_router = Router::new()
        .route("/waitlist", post(add_to_waitlist))
        .route("/deregister/{uuid}", get(deregister))
        .with_state(AppState { db, email_svc });

    let serve_dir = ServeDir::new("../frontend");
    let router = Router::new()
        .nest("/api", api_router)
        .fallback_service(serve_dir)
        .layer(middleware::from_fn(add_html_ext))
        .layer(middleware::from_fn(fix_font_mime));

    axum::serve(tcp_listener, router)
        .with_graceful_shutdown(shutdown(now))
        .await?;

    Ok(())
}

async fn fix_font_mime(req: Request<axum::body::Body>, next: middleware::Next) -> Response {
    let is_otf = req.uri().path().ends_with(".otf");
    let mut response = next.run(req).await;
    if is_otf {
        response
            .headers_mut()
            .insert(CONTENT_TYPE, "font/otf".parse().unwrap());
    }
    response
}

#[derive(Deserialize)]
struct WaitlistRequest {
    email: email_address::EmailAddress,
}

async fn add_to_waitlist(
    State(pool): State<SqlitePool>,
    State(svc): State<EmailService>,
    Json(body): Json<WaitlistRequest>,
) -> impl IntoResponse {
    let uuid = Uuid::new_v4().hyphenated().to_string();

    match add_to_waitlist_inner(body.email.clone(), uuid.clone(), pool).await {
        Ok(_) => {
            svc.send(EmailPacket::new(
                "signup-email".to_owned(),
                context! { unsubscribe_link => format!("https://2pick.de/api/deregister/{uuid}") },
                body.email,
            ))
            .await
            .unwrap();

            (StatusCode::OK, "Registriert")
        }
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            (StatusCode::CONFLICT, "Bereits registriert")
        }
        Err(e) => {
            tracing::error!(err = ?e, "Database returned error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Bitte versuchen sie es später noch einmal",
            )
        }
    }
}

async fn add_to_waitlist_inner(
    email: email_address::EmailAddress,
    uuid: String,
    pool: SqlitePool,
) -> sqlx::Result<()> {
    let email = email.as_str();

    let mut tx = pool.begin().await?;
    let uid = sqlx::query!(
        "
            insert into waitlist (email) values ($1)
            returning id
        ",
        email
    )
    .fetch_one(&mut *tx)
    .await?
    .id;

    sqlx::query!(
        "
            insert into deregistration_links (user_id, value) values ($1, $2)
        ",
        uid,
        uuid
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await
}

async fn deregister(
    Path(uuid): Path<String>,
    State(pool): State<SqlitePool>,
) -> Result<Redirect, impl IntoResponse> {
    match deregister_inner(uuid.clone(), pool).await {
        Ok(()) => Ok(Redirect::to("/deregister.html")),
        Err(sqlx::Error::RowNotFound) => {
            Err((StatusCode::NOT_FOUND, "Diese E-Mail ist nicht registriert"))
        }
        Err(e) => {
            tracing::error!(uuid = uuid, err = ?e, "Database returned error");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Ein interner Fehler ist aufgetreten",
            ))
        }
    }
}

async fn deregister_inner(uuid: String, pool: SqlitePool) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    let user_id = sqlx::query!(
        "
            delete from deregistration_links where value = $1 returning user_id
        ",
        uuid
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
        "
            delete from waitlist where id = $1
        ",
        user_id.user_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await
}

async fn shutdown(started_at: DateTime<Utc>) {
    let ctrlc = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install ctrl-c handler")
    };

    #[cfg(unix)]
    let sigterm = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let sigterm = std::future::pending::<()>();

    tokio::select! {
        _ = ctrlc => (),
        _ = sigterm => ()
    };

    let fmted = started_at.format("%Y-%m-%d_%H_%M_%S");

    if let Err(e) = tokio::fs::rename("log/latest.log", format!("log/{}.log", fmted)).await {
        tracing::error!(err = %e, "Failed to rename latest.log");
    };
}
