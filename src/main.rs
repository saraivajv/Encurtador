use std::env;

use actix_web::{
    error::{self, ErrorBadRequest, ErrorInternalServerError},
    get,
    http::header::ContentType,
    post,
    web::{self},
    App, HttpResponse, HttpServer, Responder, Result,
};
use anyhow::Context;
use dotenv::dotenv;
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use url::Url;

#[derive(Deserialize)]
struct UrlInput {
    url: String,
}

#[derive(Serialize)]
struct UrlOutput {
    short_url: String,
}

struct UrlRecord {
    url: String,
}

struct AppState {
    db: SqlitePool,
}

fn generate_code(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

#[post("/encurtar")]
async fn shorten_url(
    data: web::Data<AppState>,
    input: web::Json<UrlInput>,
) -> Result<impl Responder> {
    let code = generate_code(6);

    let url = Url::parse(&input.url).map_err(ErrorBadRequest)?.to_string();

    let query = sqlx::query!("INSERT INTO urls VALUES (?, ?)", code, url)
        .execute(&data.db)
        .await;

    match query {
        Ok(_) => {
            let short_url = format!("http://localhost:8080/{code}");
            Ok(web::Json(UrlOutput { short_url }))
        }
        Err(e) => Err(ErrorInternalServerError(e)),
    }
}

#[get("/{code}")]
async fn redirect(data: web::Data<AppState>, path: web::Path<String>) -> Result<impl Responder> {
    let code = path.into_inner();

    let query = sqlx::query_as!(UrlRecord, "SELECT url FROM urls WHERE url_hash = ?", code)
        .fetch_optional(&data.db)
        .await;

    match query {
        Ok(Some(v)) => Ok(HttpResponse::TemporaryRedirect()
            .append_header(("location", v.url))
            .finish()),
        Ok(None) => Ok(HttpResponse::NotFound()
            .content_type(ContentType::plaintext())
            .body("URL não encontrada")),
        Err(e) => Err(error::ErrorInternalServerError(e)),
    }
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let db_url = env::var("DATABASE_URL")
        .context("Variável de ambiente `DATABASE_URL` não está definida")?;

    if !Sqlite::database_exists(&db_url).await.unwrap_or(false) {
        println!("Criando banco de dados `{db_url}`");

        Sqlite::create_database(&db_url)
            .await
            .context("Erro ao criar banco de dados")?;
    }

    let db = SqlitePool::connect(&db_url)
        .await
        .with_context(|| format!("Erro ao conectar ao banco de dados `{db_url}`"))?;

    sqlx::migrate!()
        .run(&db)
        .await
        .context("Erro ao criar tabela do banco de dados")?;

    let app_state = web::Data::new(AppState { db });

    println!("Servidor rodando em http://localhost:8080");

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .service(shorten_url)
            .service(redirect)
    })
    .bind(("127.0.0.1", 8080))
    .context("Porta 8080 já está em uso")?
    .run()
    .await?;

    Ok(())
}
