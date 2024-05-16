#![feature(let_chains)]

use std::{path::PathBuf, sync::Arc};

use axum::{
    extract::{FromRef, Multipart, Path, State, WebSocketUpgrade},
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use cards::CardDatabase;
use draft::server::ServerPool;
use tokio::{net::TcpListener, sync::RwLock};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use uuid::Uuid;

mod cards;
mod draft;

type Res<T> = Result<T, String>;

fn err<T, S: ToString>(message: S) -> Res<T> {
    Err(message.to_string())
}

#[derive(serde::Serialize)]
struct Resp {
    message: String,
    success: bool,
}

impl Resp {
    fn basic<S: ToString>(message: S, status: StatusCode) -> Response<String> {
        Self::json(
            Self {
                message: message.to_string(),
                success: status == StatusCode::OK,
            },
            status,
        )
    }

    fn ok<S: ToString>(message: S) -> Response<String> {
        Self::basic(message, StatusCode::OK)
    }

    fn e500<S: ToString>(message: S) -> Response<String> {
        Self::basic(message, StatusCode::INTERNAL_SERVER_ERROR)
    }

    fn e422<S: ToString>(message: S) -> Response<String> {
        Self::basic(message, StatusCode::UNPROCESSABLE_ENTITY)
    }

    fn redirect<S: ToString>(uri: String, message: S) -> Response<String> {
        match Response::builder()
            .header("Location", uri)
            .status(StatusCode::SEE_OTHER)
            .body(message.to_string())
        {
            Ok(resp) => resp,
            Err(e) => Self::e500(format!("Failed to build redirect response: {e}")),
        }
    }

    fn json<S: serde::Serialize>(body: S, status: StatusCode) -> Response<String> {
        match serde_json::ser::to_string(&body) {
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
}

async fn websocket_handler(
    lobby: Uuid,
    seat: Uuid,
    servers: Servers,
    sock: WebSocketUpgrade,
) -> impl IntoResponse {
    if let Some(server) = servers.read().await.handle(lobby) {
        sock.on_upgrade(move |ws| draft::handlers::handle_websocket_connection(ws, server, seat))
    } else {
        // Server already closed. Just tell the client the draft has ended.
        sock.on_upgrade(move |mut ws| async move {
            if let Ok(data) = serde_json::ser::to_vec(&draft::server::ServerMessage::Ended) {
                ws.send(axum::extract::ws::Message::Binary(data)).await.ok();
            }
        })
    }
}

async fn join_table_handler(
    Path(lobby): Path<Uuid>,
    State(state): State<Arc<AppState>>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    websocket_handler(lobby, Uuid::new_v4(), state.servers.clone(), upgrade).await
}
async fn resume_seat_handler(
    Path((lobby, seat)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<AppState>>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    websocket_handler(lobby, seat, state.servers.clone(), upgrade).await
}

async fn launch_handler(
    State(state): State<Arc<AppState>>,
    data: Multipart,
) -> axum::http::Response<String> {
    draft::handlers::handle_launch_request(state.carddb.clone(), state.servers.clone(), data).await
}

async fn load_card_database(data: &std::path::Path) -> Result<CardDatabase, String> {
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

type Servers = Arc<RwLock<ServerPool>>;

struct AppState {
    carddb: Arc<CardDatabase>,
    servers: Servers,
}

#[tokio::main]
async fn main() {
    const USAGE: &str = "Usage: server <static path> <data path> <port>";

    let content = PathBuf::from(std::env::args().nth(1).expect(USAGE));
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
        .fallback_service(ServeDir::new(&content).append_index_html_on_directories(true))
        .route("/ws/:lobby/:seat", get(resume_seat_handler))
        .route("/ws/:lobby", get(join_table_handler))
        .route("/api/start", post(launch_handler))
        .route_service("/lobby/:id", ServeFile::new(content.join("lobby.html")))
        .with_state(Arc::new(AppState {
            carddb: Arc::new(card_db),
            servers: Arc::new(RwLock::new(ServerPool::new())),
        }))
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect(&format!("Failed to open port {port}"));

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("Closed due to error: {e}");
    }
}
