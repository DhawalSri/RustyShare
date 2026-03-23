use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use serde_json::json;
use tower_http::services::ServeDir;
use axum::extract::DefaultBodyLimit;//import for removing 2mb limit

struct AppState {
    files: Mutex<HashMap<String, String>>,
}

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState {
        files: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/upload", post(upload))
        .route("/download/:code", get(download))
        .fallback_service(ServeDir::new("../Frontend"))
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::disable())//disabling the axum body limit
        .with_state(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    
    println!("Server running on http://{}", addr);
    axum::serve(listener, app).await.unwrap();
}

async fn upload(
    State(state): State<Arc<AppState>>, 
    mut multipart: Multipart,          
) -> Response {
    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = field.file_name().unwrap_or("file").to_string();
        
        let data = match field.bytes().await {
            Ok(b) => b,
            Err(_) => return (StatusCode::BAD_REQUEST, "Invalid file data").into_response(),
        };

        let code: String = (0..6)
            .map(|_| rand::thread_rng().gen_range(0..10).to_string())
            .collect();

        let _ = tokio::fs::create_dir_all("uploads").await;
        let save_path = format!("uploads/{}", file_name);
        
        if tokio::fs::write(&save_path, data).await.is_ok() {
            state.files.lock().unwrap().insert(code.clone(), file_name);
            return (StatusCode::OK, Json(json!({ "code": code }))).into_response();
        }
    }
    (StatusCode::INTERNAL_SERVER_ERROR, "Upload failed").into_response()
}

async fn download(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
) -> Response {
    let file_name_opt = {
        let files = state.files.lock().unwrap();
        files.get(&code).cloned() 
    }; 

    if let Some(file_name) = file_name_opt {
        let path = format!("uploads/{}", file_name);
        
        match tokio::fs::read(&path).await {
            Ok(content) => {
                Response::builder()
                    .header("Content-Disposition", format!("attachment; filename=\"{}\"", file_name))
                    .body(axum::body::Body::from(content))
                    .unwrap()
                    .into_response()
            }
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "File read error").into_response(),
        }
    } else {
        (StatusCode::NOT_FOUND, "Code not found").into_response()
    }
}