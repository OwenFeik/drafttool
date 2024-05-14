use std::collections::HashMap;

use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    time::Instant,
};
use uuid::Uuid;

use crate::cards::Card;

use super::{game::Draft, packs::{make_packs, DraftPool}, DraftConfig};

#[derive(Clone, serde::Serialize)]
pub enum DraftServerMessage {
    Started, // Draft already started, cannot join.
    Ended,   // Draft ended.
    FatalError(String), // Server terminated due to fatal error.
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
    Disconnected,
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

    pub fn spawn(&mut self, config: DraftConfig, pool: DraftPool) -> Uuid {
        let (id, handle) = DraftServer::spawn(config, pool);
        self.servers.insert(id, handle);
        id
    }

    pub fn handle(&self, id: Uuid) -> Option<UnboundedSender<DraftServerRequest>> {
        self.servers.get(&id).cloned()
    }
}

enum Phase {
    Lobby(HashMap<Uuid, bool>, DraftConfig, DraftPool),
    Draft(Draft),
    Finished(HashMap<Uuid, Vec<Card>>),
    Terminated,
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
}

impl DraftServer {
    fn spawn(config: DraftConfig, pool: DraftPool) -> (Uuid, UnboundedSender<DraftServerRequest>) {
        let id = Uuid::new_v4();
        let (send, recv) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            let mut server = DraftServer {
                id,
                phase: Phase::Lobby(HashMap::new(), config, pool),
                chan: recv,
                clients: HashMap::new(),
            };
            server.run().await;
        });

        (id, send)
    }

    fn terminate(&mut self) {
        self.phase = Phase::Terminated;
        self.chan.close();
    }

    fn broadcast(&self, message: DraftServerMessage) {
        for client in self.clients.values() {
            client.chan.send(message.clone()).ok();
        }
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
                            },
                            Phase::Terminated => {}
                        }
                    } else if let Phase::Lobby(readys, ..) = &mut self.phase {
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
            client.heartbeat = Instant::now();
            match msg {
                DraftClientMessage::HeartBeat => client.heartbeat = Instant::now(),
                DraftClientMessage::ReadyState { ready } => {
                    if let Phase::Lobby(readys, ..) = &mut self.phase {
                        readys.insert(id, ready);
                        self.start_if_ready();
                    }
                },
                DraftClientMessage::Disconnected => {
                    if let Phase::Lobby(readys, ..) = &mut self.phase {
                        self.clients.remove(&id);
                        readys.remove(&id);
                    }
                }
            }
        }
    }

    fn start_if_ready(&mut self) {
        if let Phase::Lobby(readys, config, pool) = &self.phase {
            if !self.clients.is_empty() && self.clients.values().all(|c| readys.get(&c.id).copied().unwrap_or(false)) {
                let players: Vec<Uuid> = self.clients.values().map(|c| c.id).collect();
                match make_packs(players.len(), config, pool.clone()) {
                    Ok(packs) => {
                        let draft = Draft::new(players, config.rounds, packs);
                        self.phase = Phase::Draft(draft);

                        todo!("begin draft, transmit packs to players")
                    },
                    Err(e) => {
                        self.broadcast(DraftServerMessage::FatalError(e));
                        self.terminate();
                    }
                }
            }
        }
    }
}
