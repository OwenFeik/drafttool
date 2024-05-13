use std::collections::HashMap;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

use crate::cards::Card;

#[derive(serde::Serialize)]
pub enum DraftServerMessage {
    Started, // Draft already started, cannot join.
    Ended,   // Draft ended.
    Connected {
        draft: Uuid,
        seat: Uuid,
        pool: Vec<Card>,
        pack: Option<Vec<Card>>,
    },
}

#[derive(serde::Deserialize)]
pub enum DraftClientMessage {}

pub enum DraftServerRequest {
    Connect(Uuid, UnboundedSender<DraftServerMessage>),
    Message(Uuid, DraftClientMessage),
}

#[derive(PartialEq, Eq)]
enum Phase {
    Lobby,
    Draft,
    Finished,
}

struct Client {
    id: Uuid,
    name: String,
    chan: UnboundedSender<DraftServerMessage>,
}

struct DraftServer {
    phase: Phase,
    chan: UnboundedReceiver<DraftServerRequest>,
    clients: HashMap<Uuid, Client>,
}

impl DraftServer {
    async fn run(&mut self) {
        while let Some(req) = self.chan.recv().await {
            match req {
                DraftServerRequest::Connect(id, mut chan) => {
                    if let Some(client) = self.clients.get_mut(&id) {
                        client.chan = chan;
                        if self.phase == Phase::Draft || self.phase == Phase::Finished {
                            // send pool
                        }
                        if self.phase == Phase::Draft {
                            // send pack
                        }
                    } else if self.phase == Phase::Lobby {
                        self.clients.insert(
                            id,
                            Client {
                                id,
                                name: String::new(),
                                chan,
                            },
                        );
                    } else {
                        chan.send(DraftServerMessage::Started).ok();
                    }
                }
                DraftServerRequest::Message(id, msg) => todo!(),
            }
        }
    }
}
