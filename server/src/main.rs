use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    extract::{Multipart, State, WebSocketUpgrade},
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use cards::CardDatabase;
use tokio::net::TcpListener;
use tower_http::{services::ServeDir, trace::TraceLayer};

mod cards;
mod draft;

#[derive(serde::Serialize)]
struct Resp {
    message: String,
    success: bool,
}

impl Resp {
    fn axum<S: ToString>(message: S, status: StatusCode) -> Response<String> {
        match serde_json::ser::to_string(&Self {
            message: message.to_string(),
            success: status == StatusCode::OK,
        }) {
            Ok(body) => {
                let mut resp = Response::new(body);
                *resp.status_mut() = status;
                resp
            }
            Err(e) => {
                let mut resp = Response::new(format!("Failed to JSON encode response: {e}"));
                *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                resp
            }
        }
    }

    fn ok<S: ToString>(message: S) -> Response<String> {
        Self::axum(message, StatusCode::OK)
    }

    fn e500<S: ToString>(message: S) -> Response<String> {
        Self::axum(message, StatusCode::INTERNAL_SERVER_ERROR)
    }

    fn e422<S: ToString>(message: S) -> Response<String> {
        Self::axum(message, StatusCode::UNPROCESSABLE_ENTITY)
    }
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {}

async fn launch_handler(
    State(carddb): State<Arc<CardDatabase>>,
    data: Multipart,
) -> axum::http::Response<String> {
    draft::handlers::handle_launch_request(carddb, data).await
}

async fn load_card_database(data: &Path) -> Result<CardDatabase, String> {
    let scryfall_cards = cards::scryfall::load_cards(data).await?;
    tracing::debug!("Inserting scryfall data to card database.");
    let mut database = CardDatabase::new();
    for card in scryfall_cards {
        database.add(card);
    }
    tracing::debug!(
        "Succesfully populated card database with {} cards.",
        database.size()
    );
    Ok(database)
}

#[tokio::main]
async fn main() {
    const USAGE: &str = "Usage: server <static path> <data path> <port>";

    let content = std::env::args().nth(1).expect(USAGE);
    let data = std::env::args().nth(2).expect(USAGE);
    let port = std::env::args()
        .nth(3)
        .map(|s| u16::from_str_radix(&s, 10).expect(&format!("Invalid port number: {s}")))
        .expect(USAGE);

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let card_db = match load_card_database(&PathBuf::from(data)).await {
        Ok(db) => db,
        Err(e) => panic!("Failed to load scryfall card list: {e}"),
    };

    let app = Router::new()
        .fallback_service(ServeDir::new(content).append_index_html_on_directories(true))
        .route("/ws", get(ws_handler))
        .route("/api/start", post(launch_handler))
        .with_state(Arc::new(card_db))
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect(&format!("Failed to open port {port}"));

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("Closed due to error: {e}");
    }
}
