use actix_web::error;
use actix_web::{web, App, HttpResponse, HttpServer, Responder, Result};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sqlx::migrate::MigrateDatabase;
use sqlx::{Sqlite, SqlitePool};

#[derive(Deserialize)]
struct UrlInput {
    url: String,
}

#[derive(Serialize)]
struct UrlOutput {
    short_url: String,
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

async fn shorten_url(
    data: web::Data<AppState>,
    input: web::Json<UrlInput>,
) -> Result<impl Responder> {
    let code = generate_code(6);

    match sqlx::query("INSERT INTO urls VALUES (?, ?)")
        .bind(&code)
        .bind(&input.url)
        .execute(&data.db)
        .await
    {
        Ok(_) => {
            let short_url = format!("http://localhost:8080/{code}");
            Ok(web::Json(UrlOutput { short_url }))
        }
        Err(e) => Err(error::ErrorInternalServerError(e)),
    }
}

async fn redirect(data: web::Data<AppState>, path: web::Path<String>) -> Result<impl Responder> {
    let code = path.into_inner();

    let a = sqlx::query_as::<_, (String,)>("SELECT url FROM urls WHERE url_hash = ?")
        .bind(code)
        .fetch_optional(&data.db)
        .await;

    match a {
        Ok(Some(url)) => Ok(HttpResponse::Found()
            .append_header(("location", url.0))
            .finish()),
        Ok(None) => Ok(HttpResponse::NotFound().body("URL não encontrada")),
        Err(e) => Err(error::ErrorInternalServerError(e)),
    }
}

const DATABASE_URL: &str = "sqlite:encurtador.db";

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    if !Sqlite::database_exists(DATABASE_URL).await.unwrap_or(false) {
        println!("Criando banco de dados `{DATABASE_URL}`");
        match Sqlite::create_database(DATABASE_URL).await {
            Ok(()) => println!("Banco de dados criado com sucesso!"),
            Err(e) => panic!("Erro ao criar banco de dados: {e}"),
        }
    }

    let db = match SqlitePool::connect(DATABASE_URL).await {
        Ok(pool) => pool,
        Err(e) => {
            panic!("Erro ao criar tabela do banco de dados: {e}");
        }
    };

    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let migrations = std::path::Path::new(&crate_dir).join("./migrations");

    let migration_results = sqlx::migrate::Migrator::new(migrations)
        .await
        .unwrap()
        .run(&db)
        .await;

    if let Err(e) = migration_results {
        panic!("error: {e}");
    }

    let app_state = web::Data::new(AppState { db });

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/encurtar", web::post().to(shorten_url))
            .route("/{code}", web::get().to(redirect))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await?;

    println!("Servidor rodando em http://localhost:8080");

    Ok(())
}
