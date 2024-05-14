use std::collections::HashMap;

use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    time::Instant,
};
use uuid::Uuid;

use crate::cards::Card;

use super::{game::Draft, packs::DraftPool};

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
pub enum DraftClientMessage {
    HeartBeat,
    ReadyState { ready: bool },
}

pub enum DraftServerRequest {
    Connect(Uuid, UnboundedSender<DraftServerMessage>),
    Message(Uuid, DraftClientMessage),
}

pub struct ServerPool {
    servers: HashMap<Uuid, UnboundedSender<DraftServerRequest>>,
}

impl ServerPool {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    pub fn spawn(&mut self, pool: DraftPool) -> Uuid {
        let (id, handle) = DraftServer::spawn(pool);
        self.servers.insert(id, handle);
        id
    }

    pub fn handle(&self, id: Uuid) -> Option<UnboundedSender<DraftServerRequest>> {
        self.servers.get(&id).cloned()
    }
}

enum Phase {
    Lobby(HashMap<Uuid, bool>),
    Draft(Draft),
    Finished(HashMap<Uuid, Vec<Card>>),
}

struct Client {
    id: Uuid,
    name: String,
    chan: UnboundedSender<DraftServerMessage>,
    heartbeat: Instant,
}

pub struct DraftServer {
    id: Uuid,
    phase: Phase,
    chan: UnboundedReceiver<DraftServerRequest>,
    clients: HashMap<Uuid, Client>,
    pool: DraftPool,
}

impl DraftServer {
    fn spawn(pool: DraftPool) -> (Uuid, UnboundedSender<DraftServerRequest>) {
        let id = Uuid::new_v4();
        let (send, recv) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            let mut server = DraftServer {
                id,
                phase: Phase::Lobby(HashMap::new()),
                chan: recv,
                clients: HashMap::new(),
                pool,
            };
            server.run().await;
        });

        (id, send)
    }

    async fn run(&mut self) {
        while let Some(req) = self.chan.recv().await {
            match req {
                DraftServerRequest::Connect(id, chan) => {
                    if let Some(client) = self.clients.get_mut(&id) {
                        client.chan = chan;
                        match &self.phase {
                            Phase::Lobby(..) => {}
                            Phase::Draft(draft) => {
                                // send pack, pool
                            }
                            Phase::Finished(pools) => {
                                // send pool
                            }
                        }
                    } else if let Phase::Lobby(readys) = &mut self.phase {
                        readys.insert(id, false);
                        self.clients.insert(
                            id,
                            Client {
                                id,
                                name: String::new(),
                                chan,
                                heartbeat: Instant::now(),
                            },
                        );
                    } else {
                        chan.send(DraftServerMessage::Started).ok();
                    }
                }
                DraftServerRequest::Message(id, msg) => self.handle_client_message(id, msg),
            }
        }
    }

    fn handle_client_message(&mut self, id: Uuid, msg: DraftClientMessage) {
        if let Some(client) = self.clients.get_mut(&id) {
            match msg {
                DraftClientMessage::HeartBeat => client.heartbeat = Instant::now(),
                DraftClientMessage::ReadyState { ready } => {
                    if let Phase::Lobby(readys) = &mut self.phase {
                        readys.insert(id, ready);
                        self.start_if_ready();
                    }
                }
            }
        }
    }

    fn start_if_ready(&mut self) {
        if let Phase::Lobby(readys) = self.phase {
            if !readys.is_empty() && readys.values().all(|r| *r) {
                // build packs, send to clients
                self.phase = Phase::Draft();
            }
        }
    }
}
