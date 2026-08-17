use axum::{
    extract::{Multipart, Path, State, ws::{WebSocketUpgrade, WebSocket, Message}},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use serde_json::json;
use tower_http::services::ServeDir;
use axum::extract::DefaultBodyLimit;//import for removing 2mb limit
use tokio::{net::TcpListener, sync::broadcast};
use futures_util::{SinkExt, StreamExt};

struct AppState {
    files: Mutex<HashMap<String, String>>,
    //will track names of all connected devices
    devices: Mutex<HashSet<String>>,
    //channel that will broadcast messages to all connected devices
    tx: broadcast::Sender<String>,
}

#[tokio::main]
async fn main() {
    //creating a broadcast channel
    let (tx, _rx) = broadcast::channel(100);
    let state = Arc::new(AppState {
        files: Mutex::new(HashMap::new()),
        devices: Mutex::new(HashSet::new()),
        tx,
    });

    let app = Router::new()
        .route("/upload", post(upload))
        .route("/download/:code", get(download))
        .route("/ws", get(ws_handler))//WebSocket Endpoint
        .fallback_service(ServeDir::new("../Frontend"))
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::disable())//disabling the axum body limit
        .with_state(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener:TcpListener = tokio::net::TcpListener::bind(addr).await.unwrap();
    
    println!("Server running on http://{}", addr);
    axum::serve(listener, app).await.unwrap();
}

//NEW WEBSOCKET LOGIC FOR "AIRDROP"
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response{
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

//this event loop will manage real time connection for a single user
async fn handle_socket(socket: WebSocket, state: Arc<AppState>){
    //splitting the socket so that we can send and recieve at the exact same time
    let(mut sender, mut receiver) = socket.split();
    let mut rx = state.tx.subscribe();
    //assigning a random id to this device
    let device_id = format!("Device-{}", rand::thread_rng().gen_range(1000..9999));
    state.devices.lock().unwrap().insert(device_id.clone());
    //telling the specific device its own name
    let welcome_msg = json!({"type":"welcome", "id": device_id.clone()}).to_string();
    let _ = sender.send(Message::Text(welcome_msg.into())).await;

    //broadcast the updated active device list to everyone on the network
    let devices_list = state.devices.lock().unwrap().clone();
    let list_msg = json!({
        "type": "device_list",
        "devices": devices_list.into_iter().collect::<Vec<String>>()
    }).to_string();
    let _ = state.tx.send(list_msg);

    // TASK 1: Listen for global broadcasts and push them to this device's browser
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break; // If sending fails, the browser disconnected
            }
        }
    });
    // TASK 2: Listen for messages from THIS device's browser and broadcast them to the network
    let tx = state.tx.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            // Examples: "Device-A offers file to Device-B", "Device-B accepts"
            let _ = tx.send(text.into());
        }
    });

    //If the user closes the tab, shut down both tasks instantly
    tokio::select!{
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    // Cleanup: When the connection drops, remove them from the active list
    state.devices.lock().unwrap().remove(&device_id);

    // Tell everyone else that the device disconnected
    let devices_list = state.devices.lock().unwrap().clone();
    let list_msg = json!({
        "type": "device_list",
        "devices": devices_list.into_iter().collect::<Vec<String>>()
    }).to_string();
    let _ = state.tx.send(list_msg);
}

//OLD LOGIC
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

//use enum for connected devices instead of array.
