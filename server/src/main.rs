use axum::{
    extract::{Multipart, WebSocketUpgrade},
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use tokio::net::TcpListener;
use tower_http::{services::ServeDir, trace::TraceLayer};

mod cockatrice;
mod draft;
mod scryfall;

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

async fn launch_handler(data: Multipart) -> axum::http::Response<String> {
    draft::handle_launch_request(data).await
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

    let app = Router::new()
        .fallback_service(ServeDir::new(content).append_index_html_on_directories(true))
        .route("/ws", get(ws_handler))
        .route("/api/start", post(launch_handler))
        .layer(TraceLayer::new_for_http());

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect(&format!("Failed to open port {port}"));

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("Closed due to error: {e}");
    }
}
