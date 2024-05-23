use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use uuid::Uuid;

use crate::{
    cards::CardDatabase,
    draft::{
        server::{ClientMessage, ServerMessage},
        DraftConfig,
    },
    Resp, Servers,
};

use super::{
    packs::DraftPool,
    server::{DraftServerRequest, ServerHandle},
};

pub async fn handle_launch_request(
    carddb: Arc<CardDatabase>,
    servers: Servers,
    mut data: axum::extract::Multipart,
) -> axum::response::Response<String> {
    let mut cards = None;
    let mut list = None;
    let mut config = DraftConfig::default();
    while let Ok(Some(field)) = data.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name == "card_database" {
            match field.bytes().await {
                Ok(bytes) if bytes.is_empty() => {} // Empty card_database field is OK.
                Ok(bytes) => match crate::cards::cockatrice::decode_xml_cards(bytes) {
                    Ok(db) => cards = Some(db),
                    Err(e) => return Resp::e422(format!("Failed to load card database: {e}")),
                },
                Err(e) => return Resp::e500(e),
            }
            continue;
        }

        let s = match field.text().await {
            Ok(s) => s,
            Err(e) => return Resp::e500(e),
        };

        match field_name.as_str() {
            "list" => list = Some(s),
            "packs" => match s.parse::<usize>() {
                Ok(n) => config.rounds = n,
                Err(_) => return Resp::e422(format!("Invalid pack count: {s}")),
            },
            "cards_per_pack" => match s.parse::<usize>() {
                Ok(n) => config.cards_per_pack = n,
                Err(_) => return Resp::e422(format!("Invalid number of cards per pack: {s}")),
            },
            "unique_cards" => match s.as_str() {
                "checked" => config.unique_cards = true,
                "unchecked" => config.unique_cards = false,
                _ => return Resp::e422(format!("Invalid checkbox value for unique_cards: {s}")),
            },
            "use_rarities" => match s.as_str() {
                "checked" => config.use_rarities = true,
                "unchecked" => config.use_rarities = false,
                _ => return Resp::e422(format!("Invalid checkbox value for use_rarities: {s}")),
            },
            "mythic_incidence" => match s.parse::<f32>() {
                Ok(v) if (0.0..=1.0).contains(&v) => config.mythic_rate = v,
                _ => return Resp::e422(format!("Invalid mythic incidence: {s}")),
            },
            "rares" => match s.parse::<usize>() {
                Ok(n) => config.rares = n,
                Err(_) => return Resp::e422(format!("Invalid number of rares per pack: {s}")),
            },
            "uncommons" => match s.parse::<usize>() {
                Ok(n) => config.uncommons = n,
                Err(_) => return Resp::e422(format!("Invalid number of commons per pack: {s}")),
            },
            "commons" => match s.parse::<usize>() {
                Ok(n) => config.commons = n,
                Err(_) => return Resp::e422(format!("Invalid number of commons per pack: {s}")),
            },
            _ => {}
        }
    }

    if config.rares + config.uncommons + config.commons != config.cards_per_pack {
        return Resp::e422(format!(
            "Count of rares ({}) + uncommons ({}) + commons ({}) greater than number of cards in pack ({}).",
            config.rares,
            config.uncommons,
            config.commons,
            config.cards_per_pack
        ));
    }

    let Some(list) = list else {
        return Resp::e422("No card list provided for draft.");
    };

    let mut pool = DraftPool::new();
    for line in list.lines() {
        let key = &line.trim().to_lowercase();
        if key.is_empty() {
            continue;
        }

        let Some(card) = cards
            .as_ref()
            .and_then(|ccs| ccs.get(key).cloned())
            .or_else(|| carddb.get(key).cloned())
        else {
            return Resp::e422(format!("Card not found in custom list or database: {line}"));
        };

        pool.add(card);
    }

    let id = servers.write().await.spawn(config, pool);

    Resp::redirect(format!("/lobby/{id}"), "Draft launched.".to_string())
}

pub async fn handle_websocket_connection(mut ws: WebSocket, server: ServerHandle, seat: Uuid) {
    // Test sending a ping to validate the connection.
    if ws
        .send(Message::Ping("ping".as_bytes().to_owned()))
        .await
        .is_err()
    {
        tracing::debug!("Ping on connection failed.");
        return;
    }

    // If the server is already closed, abort the connection.
    if !server.is_open() {
        tracing::debug!("Attempted to join already closed draft.");
        if let Ok(data) = serde_json::ser::to_vec(&ServerMessage::Ended) {
            ws.send(Message::Binary(data)).await.ok();
        }
        return;
    }

    // Attempt to send channel to server to allow server to message client.
    let (send, mut recv) = tokio::sync::mpsc::unbounded_channel();
    server.send(DraftServerRequest::Connect(seat, send));

    // Split the websocket. The send half will handle encoding messages from the
    // server and forwarding them to the client, while the receive half will
    // handle decoding messages from the client and sending them to the server.
    let (mut ws_send, mut ws_recv) = ws.split();

    let mut send_task = tokio::spawn(async move {
        while let Some(message) = recv.recv().await {
            match serde_json::ser::to_vec(&message) {
                Ok(data) => {
                    if let Err(e) = ws_send.send(Message::Binary(data)).await {
                        tracing::debug!("Failed to send message to client: {e}");
                        break;
                    }
                }
                Err(e) => tracing::debug!("Failed to encode server message: {e}"),
            }
        }
    });

    let handle = server.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(message)) = ws_recv.next().await {
            let msg = match message {
                Message::Text(text) => serde_json::de::from_str(&text),
                Message::Binary(bytes) => serde_json::de::from_slice(&bytes),
                Message::Ping(_) | Message::Pong(_) => continue, // not a message
                Message::Close(_) => break,                      // client disconnected
            };

            match msg {
                Ok(message) => handle.send(DraftServerRequest::Message(seat, message)),
                Err(e) => tracing::debug!("Failed to decode client message: {e}"),
            };
        }
    });

    // When either task completes, abort the other.
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    server.send(DraftServerRequest::Message(
        seat,
        ClientMessage::Disconnected,
    ));
}
