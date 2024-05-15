use std::collections::HashMap;

use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    time::Instant,
};
use uuid::Uuid;

use crate::cards::Card;

use super::{
    game::Draft,
    packs::{make_packs, DraftPool, Pack},
    DraftConfig,
};

#[derive(Clone, Debug, serde::Serialize)]
pub enum DraftServerMessage {
    /// Draft already started, cannot join.
    Started,
    /// Draft ended.
    Ended,
    /// Server terminated due to fatal error.
    FatalError(String),

    /// New pack for user to pick from.
    Pack(Pack),

    /// Draft finished, here's your final pool.
    Finished(Vec<Card>),

    /// Successfully connected to the lobby.
    Connected {
        draft: Uuid,
        seat: Uuid,
        players: Vec<(Uuid, String)>,
    },

    /// Successfully reconnected to in progress or completed draft.
    Reconnected {
        draft: Uuid,
        seat: Uuid,
        pool: Vec<Card>,
        pack: Option<Vec<Card>>,
    },
}

#[derive(Debug, serde::Deserialize)]
pub enum DraftClientMessage {
    HeartBeat,
    ReadyState(bool),
    Disconnected,
    SetName(String),
    Pick(usize),
}

#[derive(Debug)]
pub enum DraftServerRequest {
    Connect(Uuid, UnboundedSender<DraftServerMessage>),
    Message(Uuid, DraftClientMessage),
    Terminate(String),
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

impl Client {
    fn send(&self, message: DraftServerMessage) {
        self.chan.send(message).ok();
    }
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

    fn terminate(&mut self, error: String) {
        self.phase = Phase::Terminated;
        self.chan.close();
        self.broadcast(DraftServerMessage::FatalError(error));
    }

    fn broadcast(&self, message: DraftServerMessage) {
        for client in self.clients.values() {
            client.send(message.clone());
        }
    }

    async fn run(&mut self) {
        while let Some(req) = self.chan.recv().await {
            match req {
                DraftServerRequest::Connect(id, chan) => self.handle_client_connection(id, chan),
                DraftServerRequest::Message(id, msg) => self.handle_client_message(id, msg),
                DraftServerRequest::Terminate(reason) => self.terminate(reason),
            }
        }
    }

    fn handle_client_connection(&mut self, id: Uuid, chan: UnboundedSender<DraftServerMessage>) {
        if let Some(client) = self.clients.get_mut(&id) {
            client.chan = chan;
            let client = self.clients.get(&id).unwrap(); // de-mut reference.
            match &self.phase {
                Phase::Lobby(..) => client.send(DraftServerMessage::Connected {
                    draft: self.id,
                    seat: id,
                    players: self.player_list(),
                }),
                Phase::Draft(draft) => client.send(DraftServerMessage::Reconnected {
                    draft: self.id,
                    seat: id,
                    pool: draft.drafted_cards(id).cloned().unwrap_or_default(),
                    pack: draft.current_pack(id).cloned(),
                }),
                Phase::Finished(pools) => client.send(DraftServerMessage::Reconnected {
                    draft: self.id,
                    seat: id,
                    pool: pools.get(&id).cloned().unwrap_or_default(),
                    pack: None,
                }),
                Phase::Terminated => {}
            }
        } else if let Phase::Lobby(readys, ..) = &mut self.phase {
            readys.insert(id, false);
            let client = Client {
                id,
                name: String::new(),
                chan,
                heartbeat: Instant::now(),
            };
            self.clients.insert(id, client);
            self.send_to(
                id,
                DraftServerMessage::Connected {
                    draft: self.id,
                    seat: id,
                    players: self.player_list(),
                },
            );
        } else {
            chan.send(DraftServerMessage::Started).ok();
        }
    }

    fn handle_client_message(&mut self, id: Uuid, msg: DraftClientMessage) {
        if let Some(client) = self.clients.get_mut(&id) {
            client.heartbeat = Instant::now();
            match msg {
                DraftClientMessage::HeartBeat => client.heartbeat = Instant::now(),
                DraftClientMessage::ReadyState(ready) => {
                    if let Phase::Lobby(readys, ..) = &mut self.phase {
                        readys.insert(id, ready);
                        self.start_if_ready();
                    }
                }
                DraftClientMessage::Disconnected => {
                    if let Phase::Lobby(readys, ..) = &mut self.phase {
                        self.clients.remove(&id);
                        readys.remove(&id);
                    }
                }
                DraftClientMessage::SetName(name) => {
                    client.name = name;
                }
                DraftClientMessage::Pick(index) => {
                    if let Phase::Draft(draft) = &mut self.phase {
                        if let Some(packs) = draft.handle_pick(id, index) {
                            self.send_packs(packs);
                            self.finish_if_done();
                        }
                    }
                }
            }
        }
    }

    fn send_to(&self, id: Uuid, message: DraftServerMessage) {
        if let Some(client) = self.clients.get(&id) {
            client.send(message);
        }
    }

    fn send_packs(&self, packs: Vec<(Uuid, Pack)>) {
        for (id, pack) in packs {
            self.send_to(id, DraftServerMessage::Pack(pack));
        }
    }

    fn player_list(&self) -> Vec<(Uuid, String)> {
        self.clients
            .values()
            .map(|c| (c.id, c.name.clone()))
            .collect()
    }

    fn start_if_ready(&mut self) {
        if let Phase::Lobby(readys, config, pool) = &self.phase {
            if !self.clients.is_empty()
                && self
                    .clients
                    .values()
                    .all(|c| readys.get(&c.id).copied().unwrap_or(false))
            {
                let players: Vec<Uuid> = self.clients.values().map(|c| c.id).collect();
                match make_packs(players.len(), config, pool.clone()) {
                    Ok(packs) => {
                        let mut draft = Draft::new(players, config.rounds, packs);
                        self.send_packs(draft.begin());
                        self.phase = Phase::Draft(draft);
                    }
                    Err(e) => self.terminate(e),
                }
            }
        }
    }

    fn finish_if_done(&mut self) {
        if let Phase::Draft(draft) = &self.phase {
            if draft.draft_complete() {
                let pools = draft.pools().clone();
                for (id, pool) in &pools {
                    self.send_to(*id, DraftServerMessage::Finished(pool.clone()));
                }
                self.phase = Phase::Finished(pools);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::draft::packs::DraftPool;

    fn close_server(handle: UnboundedSender<DraftServerRequest>) {
        assert!(handle
            .send(DraftServerRequest::Terminate(String::new()))
            .is_ok());
    }

    #[tokio::test]
    async fn test_joining_server() {
        let (server, handle) = DraftServer::spawn(Default::default(), DraftPool::new());
        let user = Uuid::new_v4();
        let (send, mut recv) = tokio::sync::mpsc::unbounded_channel();
        assert!(handle.send(DraftServerRequest::Connect(user, send)).is_ok());
        if let DraftServerMessage::Connected {
            draft,
            seat,
            players,
        } = recv.recv().await.unwrap()
        {
            assert_eq!(draft, server);
            assert_eq!(seat, user);
            assert_eq!(players, vec![(seat, String::new())]);
        } else {
            panic!("Expected to receive connected message first.");
        };

        close_server(handle);
        assert!(matches!(
            recv.recv().await.unwrap(),
            DraftServerMessage::FatalError(..)
        ));
    }
}
