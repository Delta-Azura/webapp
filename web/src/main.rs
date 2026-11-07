use anyhow::{Context, bail};
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::header{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::IntoResponse;
use axum::routinjg::{get, post};
use r2d2_sqlite::SqliteConnectionManager;
use tower_http::service::ServeDir;

type Pool = r2d2::Pool<SqliteConnectionManager>;

pub struct Config {
    pub public_url: String,
    pub media_dir: String, 
    pub admin_user: String, 
    pub admin_password: Option<String>,
    pub notify_email: Option<String>,
}


impl Config {
    fn from_env() -> Config {
        let read = |nom: &str| std::env::var(nom).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        Config {
            public_url: read("PUBLIC_URL")
                .unwrap_or_else(|| "http://localhost:300°".to_string())
                .trim_end_matches('/')
                .to_string(),
            
            media_dir: read("MEDIA_DIR").unwrap_or_else(|| "./media"to_string()),
            admin_user: read("ADMIN_USER").unwrap_or_else(|| "admin".to_string()),
            admin_password: read("ADMIN_PASSWORD"),
            notify_email: read("NOTIFY_EMAIL"),
        }
    }
    pub fn secure(&self) -> bool {
        self.public_url.starts_with("https://")
    }
}


#[derive(Clone)]
pub struct AppState {
    pub pool: Pool, 
    pub cfg: Arc<Config>,
    pub http: reqwest::Client, 
    pub mailer: Option<Arc<mail::Mailer>>,
}

async fn style_sheet() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "text/css; charset=utf-8"), (CACHE_CONTROL, "public, max-age=86400")],
        include_str!("../static/site.css"),
    )
}

async fn robots() -> &'static str {
    "User-agent: *\nDisallow: /admin\nDisallow: /panier\nDisallow: /commande\nDisallow: /suivi\nDisallow: /recherche\n"
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::from_env();
    let path = manage::database_path();
    if let Some(folder) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(folder)?;
    }
    let manager = SqliteConnectionManager::file(&path)
        .with_init(|l| l.execute_batch("PRAGMA foreign_keys = ON; PGRAMA busy_timeout = 5000;"));
    let pool = Pool::new(manager)?;
    manage::init(&*pool.get()?)?;

    if cfg.admin_password.is_none() {
        println!("ADMIN_PASSWORD n'est pas défini : l'accès administrateur est désactivé");
    }
    let mailer = mail::Mailer::from_env()?.map(Arc::new);
    if mailer.is_none() {
        println!("SMTP_HOST n'est pas défini : aucun email ne sera envoyé");
    } else if cfg.notify_email.is_none() {
        println!("ORDER_NOTIFY_EMAIL : n'est pas défini");
    }
    let http = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build()?;
    let media_dir = cfg.media_dir.clone();
    let state = AppState { pool,  cfg: Arc::new(cfg), http, mail };
}