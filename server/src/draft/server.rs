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
#[serde(tag = "type", content = "value")]
pub enum ServerMessage {
    /// Draft already started, cannot join.
    Started,
    /// Draft ended.
    Ended,
    /// Server terminated due to fatal error.
    FatalError(String),

    /// New pack for user to pick from.
    Pack(Pack), // TODO this should include an ID to handle out of order events

    /// Pick was successful, current pack has been passed on.
    Passed,

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
        in_progress: bool,
        pool: Vec<Card>,
        pack: Option<Vec<Card>>,
    },
}

#[derive(Debug, serde::Deserialize)]
pub enum ClientMessage {
    HeartBeat,
    ReadyState(bool),
    Disconnected,
    SetName(String),
    Pick(usize),
}

#[derive(Debug)]
pub enum DraftServerRequest {
    Connect(Uuid, UnboundedSender<ServerMessage>),
    Message(Uuid, ClientMessage),
    Terminate(String),
}

#[derive(Clone)]
pub struct ServerHandle {
    id: Uuid,
    chan: UnboundedSender<DraftServerRequest>,
}

impl ServerHandle {
    pub fn send(&self, req: DraftServerRequest) {
        self.chan.send(req).ok();
    }

    pub fn is_open(&self) -> bool {
        !self.chan.is_closed()
    }
}

pub struct ServerPool {
    servers: HashMap<Uuid, ServerHandle>,
}

impl ServerPool {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    pub fn spawn(&mut self, config: DraftConfig, pool: DraftPool) -> Uuid {
        let handle = DraftServer::spawn(config, pool);
        let id = handle.id;
        self.servers.insert(id, handle);
        id
    }

    pub fn handle(&self, id: Uuid) -> Option<ServerHandle> {
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
    chan: UnboundedSender<ServerMessage>,
    heartbeat: Instant,
}

impl Client {
    fn send(&self, message: ServerMessage) {
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
    fn spawn(config: DraftConfig, pool: DraftPool) -> ServerHandle {
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

        ServerHandle { id, chan: send }
    }

    fn terminate(&mut self, error: String) {
        self.phase = Phase::Terminated;
        self.chan.close();
        self.broadcast(ServerMessage::FatalError(error));
    }

    fn broadcast(&self, message: ServerMessage) {
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

    fn handle_client_connection(&mut self, id: Uuid, chan: UnboundedSender<ServerMessage>) {
        if let Some(client) = self.clients.get_mut(&id) {
            client.chan = chan;
            let client = self.clients.get(&id).unwrap(); // de-mut reference.
            match &self.phase {
                Phase::Lobby(..) => client.send(ServerMessage::Connected {
                    draft: self.id,
                    seat: id,
                    players: self.player_list(),
                }),
                Phase::Draft(draft) => client.send(ServerMessage::Reconnected {
                    draft: self.id,
                    seat: id,
                    in_progress: true,
                    pool: draft.drafted_cards(id).cloned().unwrap_or_default(),
                    pack: draft.current_pack(id),
                }),
                Phase::Finished(pools) => client.send(ServerMessage::Reconnected {
                    draft: self.id,
                    seat: id,
                    in_progress: false,
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
                ServerMessage::Connected {
                    draft: self.id,
                    seat: id,
                    players: self.player_list(),
                },
            );
        } else {
            chan.send(ServerMessage::Started).ok();
        }
    }

    fn handle_client_message(&mut self, id: Uuid, msg: ClientMessage) {
        if let Some(client) = self.clients.get_mut(&id) {
            client.heartbeat = Instant::now();
            match msg {
                ClientMessage::HeartBeat => client.heartbeat = Instant::now(),
                ClientMessage::ReadyState(ready) => {
                    if let Phase::Lobby(readys, ..) = &mut self.phase {
                        readys.insert(id, ready);
                        self.start_if_ready();
                    }
                }
                ClientMessage::Disconnected => {
                    if let Phase::Lobby(readys, ..) = &mut self.phase {
                        self.clients.remove(&id);
                        readys.remove(&id);
                    }
                }
                ClientMessage::SetName(name) => {
                    client.name = name;
                }
                ClientMessage::Pick(index) => {
                    if let Phase::Draft(draft) = &mut self.phase {
                        if let Some(packs) = draft.handle_pick(id, index) {
                            client.send(ServerMessage::Passed);
                            self.send_packs(packs);
                            self.finish_if_done();
                        } else if let Some(pack) = draft.current_pack(id) {
                            // Invalid pick command. Maybe client pack is
                            // desynced? Resend current pack.
                            client.send(ServerMessage::Pack(pack));
                        }
                    }
                }
            }
        }
    }

    fn send_to(&self, id: Uuid, message: ServerMessage) {
        if let Some(client) = self.clients.get(&id) {
            client.send(message);
        }
    }

    fn send_packs(&self, packs: Vec<(Uuid, Pack)>) {
        for (id, pack) in packs {
            self.send_to(id, ServerMessage::Pack(pack));
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
                    Err(e) => self.terminate(format!("Failed to create packs for draft: {e}")),
                }
            }
        }
    }

    fn finish_if_done(&mut self) {
        if let Phase::Draft(draft) = &self.phase {
            if draft.draft_complete() {
                let pools = draft.pools().clone();
                for (id, pool) in &pools {
                    self.send_to(*id, ServerMessage::Finished(pool.clone()));
                }
                self.phase = Phase::Finished(pools);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::assert_matches::assert_matches;

    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::draft::packs::DraftPool;

    fn close_server(handle: ServerHandle) {
        handle.send(DraftServerRequest::Terminate(String::new()));
    }

    fn client_send(handle: &ServerHandle, id: Uuid, message: ClientMessage) {
        handle.send(DraftServerRequest::Message(id, message))
    }

    async fn receive(chan: &mut UnboundedReceiver<ServerMessage>) -> ServerMessage {
        match tokio::time::timeout(tokio::time::Duration::from_millis(1), chan.recv()).await {
            Ok(Some(msg)) => msg,
            Ok(None) => panic!("Channel was closed, expected to receive a message."),
            Err(_) => panic!("Didn't receive on channel after 1ms."),
        }
    }

    async fn add_client(handle: &ServerHandle) -> (Uuid, UnboundedReceiver<ServerMessage>) {
        let user = Uuid::new_v4();
        let (send, mut recv) = unbounded_channel();
        handle.send(DraftServerRequest::Connect(user, send));
        if let ServerMessage::Connected {
            draft,
            seat,
            players,
        } = recv.recv().await.unwrap()
        {
            assert_eq!(draft, handle.id);
            assert_eq!(seat, user);
            assert!(players.contains(&(seat, String::new())));
        } else {
            panic!("Expected to receive connected message first.");
        };
        (user, recv)
    }

    #[tokio::test]
    async fn test_joining_closing_server() {
        let handle = DraftServer::spawn(Default::default(), DraftPool::new());
        let (_user, mut recv) = add_client(&handle).await;
        close_server(handle);
        assert_matches!(recv.recv().await.unwrap(), ServerMessage::FatalError(..));
    }

    #[tokio::test]
    async fn test_draft() {
        let pool = DraftPool::sample(1, 1, 1, 1);
        let config = DraftConfig {
            rounds: 1,
            cards_per_pack: 2,
            unique_cards: true,
            use_rarities: false,
            ..Default::default()
        };
        let handle = DraftServer::spawn(config, pool);
        let (p1, mut chan1) = add_client(&handle).await;
        let (p2, mut chan2) = add_client(&handle).await;

        // Once both players are ready
        client_send(&handle, p1, ClientMessage::ReadyState(true));
        client_send(&handle, p2, ClientMessage::ReadyState(true));
        assert_matches!(receive(&mut chan1).await, ServerMessage::Pack(..));
        assert_matches!(receive(&mut chan2).await, ServerMessage::Pack(..));

        // Client one passes, don't expect any packs at this stage as they are
        // backed up behind p2.
        client_send(&handle, p1, ClientMessage::Pick(0));
        assert_matches!(receive(&mut chan1).await, ServerMessage::Passed);

        // After p2s pick, both players should be sent a new pack.
        client_send(&handle, p2, ClientMessage::Pick(0));
        assert_matches!(receive(&mut chan2).await, ServerMessage::Passed);
        assert_matches!(receive(&mut chan2).await, ServerMessage::Pack(..));
        assert_matches!(receive(&mut chan1).await, ServerMessage::Pack(..));

        client_send(&handle, p1, ClientMessage::Pick(0));
        assert_matches!(receive(&mut chan1).await, ServerMessage::Passed);
        client_send(&handle, p2, ClientMessage::Pick(0));
        assert_matches!(receive(&mut chan2).await, ServerMessage::Passed);

        assert_matches!(receive(&mut chan1).await, ServerMessage::Finished(cards) if cards.len() == 2);
        assert_matches!(receive(&mut chan2).await, ServerMessage::Finished(cards) if cards.len() == 2);
    }
}
