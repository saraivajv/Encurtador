use actix_web::{web, App, HttpResponse, HttpServer, Responder, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use rand::{distributions::Alphanumeric, Rng};

// Estrutura para receber a URL original
#[derive(Deserialize)]
struct UrlInput {
    url: String,
}

// Estrutura para retornar a URL encurtada
#[derive(Serialize)]
struct UrlOutput {
    short_url: String,
}

// Tipo de dados compartilhados: HashMap protegido por Mutex
struct AppState {
    url_map: Mutex<HashMap<String, String>>,
}

// Função para gerar um código aleatório
fn generate_code(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

// Handler para encurtar a URL
async fn shorten_url(
    data: web::Data<AppState>,
    input: web::Json<UrlInput>,
) -> Result<impl Responder> {
    let code = generate_code(6);
    let mut map = data.url_map.lock().unwrap();
    map.insert(code.clone(), input.url.clone());

    // Aqui você pode ajustar o domínio conforme necessário
    let short_url = format!("http://localhost:8080/{}", code);
    Ok(web::Json(UrlOutput { short_url }))
}

// Handler para redirecionar para a URL original
async fn redirect(
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<impl Responder> {
    let code = path.into_inner();
    let map = data.url_map.lock().unwrap();
    if let Some(original_url) = map.get(&code) {
        Ok(HttpResponse::Found()
            .append_header(("Location", original_url.clone()))
            .finish())
    } else {
        Ok(HttpResponse::NotFound().body("URL não encontrada"))
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Inicializa o estado compartilhado
    let app_state = web::Data::new(AppState {
        url_map: Mutex::new(HashMap::new()),
    });

    println!("Servidor rodando em http://localhost:8080");

    // Inicia o servidor
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/encurtar", web::post().to(shorten_url))
            .route("/{code}", web::get().to(redirect))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
