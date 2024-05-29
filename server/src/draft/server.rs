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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ClientStatus {
    Ok,
    Warning,
    Error,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PlayerDetails {
    seat: Uuid,
    name: String,
    ready: bool,
    status: ClientStatus,
}

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
    PickSuccessful(Card),

    /// Draft finished, here's your final pool.
    Finished(Vec<Card>),

    /// Successfully connected to the lobby.
    Connected { draft: Uuid, seat: Uuid },

    /// Successfully reconnected to in progress or completed draft.
    Reconnected {
        draft: Uuid,
        seat: Uuid,
        in_progress: bool,
        pool: Vec<Card>,
        pack: Option<Vec<Card>>,
    },

    /// Client sent us a message that doesn't make sense, their state must be
    /// messed up. Tell them to refresh.
    Refresh,

    /// Change to the list of connected players.
    PlayerList(Vec<PlayerDetails>),

    /// Client name, ready state or status update.
    PlayerUpdate(PlayerDetails),
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type", content = "value")]
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

    pub(super) fn spawn(&mut self, config: DraftConfig, pool: DraftPool) -> Uuid {
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
    known_status: ClientStatus,
    heartbeat: Instant,
}

impl Client {
    fn send(&self, message: ServerMessage) {
        self.chan.send(message).ok();
    }

    fn status(&self) -> ClientStatus {
        if self.chan.is_closed() {
            ClientStatus::Error
        } else {
            self.known_status
        }
    }
}

struct DraftClients {
    clients: Vec<Client>,
}

impl DraftClients {
    fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    fn add(&mut self, client: Client) {
        self.clients.push(client);
    }

    fn get(&self, id: Uuid) -> Option<&Client> {
        self.clients.iter().find(|c| c.id == id)
    }

    fn get_mut(&mut self, id: Uuid) -> Option<&mut Client> {
        self.clients.iter_mut().find(|c| c.id == id)
    }

    fn remove(&mut self, id: Uuid) {
        self.clients.retain(|c| c.id != id);
    }

    fn iter(&self) -> std::slice::Iter<Client> {
        self.clients.iter()
    }
}

pub struct DraftServer {
    id: Uuid,
    phase: Phase,
    chan: UnboundedReceiver<DraftServerRequest>,
    clients: DraftClients,
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
                clients: DraftClients {
                    clients: Vec::new(),
                },
            };
            server.run().await;
        });

        ServerHandle { id, chan: send }
    }

    fn terminate(&mut self, error: String) {
        self.phase = Phase::Terminated;
        self.chan.close();
        self.broadcast(ServerMessage::FatalError(error), None);
    }

    fn broadcast(&self, message: ServerMessage, exclude: Option<Uuid>) {
        for client in self.clients.iter() {
            if Some(client.id) != exclude {
                client.send(message.clone());
            }
        }
    }

    fn broadcast_player_update(&self, player: Uuid) {
        let ready = if let Phase::Lobby(readys, ..) = &self.phase {
            readys.get(&player).cloned().unwrap_or(false)
        } else {
            true
        };

        if let Some(client) = self.clients.get(player) {
            self.broadcast(
                ServerMessage::PlayerUpdate(PlayerDetails {
                    seat: player,
                    name: client.name.clone(),
                    ready,
                    status: client.status(),
                }),
                Some(player),
            );
        }
    }

    fn set_client_status(&mut self, id: Uuid, status: ClientStatus) {
        if let Some(client) = self.clients.get_mut(id) {
            if client.known_status != status {
                client.known_status = status;
                self.broadcast_player_update(id);
            }
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
        if let Some(client) = self.clients.get_mut(id) {
            client.chan = chan;
            self.set_client_status(id, ClientStatus::Ok);
            let client = self.clients.get(id).unwrap(); // de-mut reference.
            match &self.phase {
                Phase::Lobby(..) => client.send(ServerMessage::Connected {
                    draft: self.id,
                    seat: id,
                }),
                Phase::Draft(draft) => {
                    client.send(ServerMessage::Reconnected {
                        draft: self.id,
                        seat: id,
                        in_progress: true,
                        pool: draft.drafted_cards(id).cloned().unwrap_or_default(),
                        pack: draft.current_pack(id),
                    });
                    client.send(ServerMessage::PlayerList(self.player_list()));
                }
                Phase::Finished(pools) => {
                    client.send(ServerMessage::Reconnected {
                        draft: self.id,
                        seat: id,
                        in_progress: false,
                        pool: pools.get(&id).cloned().unwrap_or_default(),
                        pack: None,
                    });
                    client.send(ServerMessage::PlayerList(self.player_list()));
                }
                Phase::Terminated => {
                    client.send(ServerMessage::FatalError("Draft terminated.".into()))
                }
            }
        } else if let Phase::Lobby(readys, ..) = &mut self.phase {
            readys.insert(id, false);
            let client = Client {
                id,
                name: id.to_string()[0..8].to_string(),
                chan,
                known_status: ClientStatus::Ok,
                heartbeat: Instant::now(),
            };
            self.clients.add(client);
            self.send_to(
                id,
                ServerMessage::Connected {
                    draft: self.id,
                    seat: id,
                },
            );
            self.broadcast(ServerMessage::PlayerList(self.player_list()), None);
        } else {
            chan.send(ServerMessage::Started).ok();
        }
    }

    fn handle_client_message(&mut self, id: Uuid, msg: ClientMessage) {
        if let Some(client) = self.clients.get_mut(id) {
            client.heartbeat = Instant::now();
            match msg {
                ClientMessage::HeartBeat => client.heartbeat = Instant::now(),
                ClientMessage::ReadyState(ready) => {
                    if let Phase::Lobby(readys, ..) = &mut self.phase {
                        readys.insert(id, ready);
                        if !self.start_if_ready() {
                            self.broadcast_player_update(id);
                        }
                    }
                }
                ClientMessage::Disconnected => {
                    if let Phase::Lobby(readys, ..) = &mut self.phase {
                        self.clients.remove(id);
                        readys.remove(&id);
                        self.broadcast(ServerMessage::PlayerList(self.player_list()), None);
                    } else {
                        self.set_client_status(id, ClientStatus::Error);
                    }
                }
                ClientMessage::SetName(name) => {
                    client.name = name;
                    self.broadcast_player_update(id);
                }
                ClientMessage::Pick(index) => {
                    if let Phase::Draft(draft) = &mut self.phase {
                        if let Ok((card, packs)) = draft.handle_pick(id, index) {
                            client.send(ServerMessage::PickSuccessful(card));
                            self.send_packs(packs);
                            self.finish_if_done();
                        } else if let Some(pack) = draft.current_pack(id) {
                            // Invalid pick command. Maybe client pack is
                            // desynced? Resend current pack.
                            client.send(ServerMessage::Pack(pack));
                        }
                    } else {
                        client.send(ServerMessage::Refresh);
                    }
                }
            }
        }
    }

    fn send_to(&self, id: Uuid, message: ServerMessage) {
        if let Some(client) = self.clients.get(id) {
            client.send(message);
        }
    }

    fn send_packs(&self, packs: Vec<(Uuid, Pack)>) {
        for (id, pack) in packs {
            self.send_to(id, ServerMessage::Pack(pack));
        }
    }

    fn ready_state(&self, seat: Uuid) -> bool {
        if let Phase::Lobby(readys, ..) = &self.phase {
            readys.get(&seat).cloned().unwrap_or(false)
        } else {
            self.clients.get(seat).is_some()
        }
    }

    fn details_of(&self, seat: Uuid) -> Option<PlayerDetails> {
        let client = self.clients.get(seat)?;
        Some(PlayerDetails {
            seat,
            name: client.name.clone(),
            ready: self.ready_state(seat),
            status: client.status(),
        })
    }

    fn player_list(&self) -> Vec<PlayerDetails> {
        self.clients
            .iter()
            .filter_map(|c| self.details_of(c.id))
            .collect()
    }

    /// If all players are ready to start, attempt to build packs and start the
    /// draft. Returns true if the draft was started, else false.
    fn start_if_ready(&mut self) -> bool {
        if let Phase::Lobby(readys, config, pool) = &self.phase {
            if !self.clients.is_empty()
                && self
                    .clients
                    .iter()
                    .all(|c| readys.get(&c.id).copied().unwrap_or(false))
            {
                let players: Vec<Uuid> = self.clients.iter().map(|c| c.id).collect();
                match make_packs(players.len(), config, pool.clone()) {
                    Ok(packs) => {
                        let mut draft = Draft::new(players, config.rounds, packs);
                        self.send_packs(draft.begin());
                        self.phase = Phase::Draft(draft);
                        return true;
                    }
                    Err(e) => self.terminate(format!("Failed to create packs for draft: {e}")),
                }
            }
        }
        false
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
        if let ServerMessage::Connected { draft, seat } = receive(&mut recv).await {
            assert_eq!(draft, handle.id);
            assert_eq!(seat, user);
        } else {
            panic!("Expected to receive connected message first.");
        };
        if let ServerMessage::PlayerList(players) = receive(&mut recv).await {
            assert!(players.iter().any(|p| p.seat == user));
        } else {
            panic!("Expected to receive player list second.");
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
        assert_matches!(receive(&mut chan1).await, ServerMessage::PlayerList(..));

        // Once both players are ready
        client_send(&handle, p1, ClientMessage::ReadyState(true));
        assert_matches!(receive(&mut chan2).await, ServerMessage::PlayerUpdate(..));
        client_send(&handle, p2, ClientMessage::ReadyState(true));
        assert_matches!(receive(&mut chan1).await, ServerMessage::Pack(..));
        assert_matches!(receive(&mut chan2).await, ServerMessage::Pack(..));

        // Client one passes, don't expect any packs at this stage as they are
        // backed up behind p2.
        client_send(&handle, p1, ClientMessage::Pick(0));
        assert_matches!(receive(&mut chan1).await, ServerMessage::PickSuccessful(_));

        // After p2s pick, both players should be sent a new pack.
        client_send(&handle, p2, ClientMessage::Pick(0));
        assert_matches!(receive(&mut chan2).await, ServerMessage::PickSuccessful(_));
        assert_matches!(receive(&mut chan2).await, ServerMessage::Pack(..));
        assert_matches!(receive(&mut chan1).await, ServerMessage::Pack(..));

        client_send(&handle, p1, ClientMessage::Pick(0));
        assert_matches!(receive(&mut chan1).await, ServerMessage::PickSuccessful(_));
        client_send(&handle, p2, ClientMessage::Pick(0));
        assert_matches!(receive(&mut chan2).await, ServerMessage::PickSuccessful(_));

        assert_matches!(receive(&mut chan1).await, ServerMessage::Finished(cards) if cards.len() == 2);
        assert_matches!(receive(&mut chan2).await, ServerMessage::Finished(cards) if cards.len() == 2);
    }

    #[tokio::test]
    async fn test_set_name() {
        let handle = &DraftServer::spawn(DraftConfig::default(), DraftPool::new());
        let (p1, mut _chan1) = add_client(handle).await;
        let (_p2, mut chan2) = add_client(handle).await;

        client_send(handle, p1, ClientMessage::SetName("name".into()));
        let ServerMessage::PlayerUpdate(PlayerDetails {
            seat,
            name,
            ready,
            status,
        }) = receive(&mut chan2).await
        else {
            panic!("Should have received a status update.");
        };
        assert_eq!(seat, p1);
        assert_eq!(name, "name");
        assert!(!ready);
        assert_eq!(status, ClientStatus::Ok);

        client_send(handle, p1, ClientMessage::ReadyState(true));
        let ServerMessage::PlayerUpdate(PlayerDetails {
            seat,
            name,
            ready,
            status,
        }) = receive(&mut chan2).await
        else {
            panic!("Should have received a status update.");
        };
        assert_eq!(seat, p1);
        assert_eq!(name, "name");
        assert!(ready);
        assert_eq!(status, ClientStatus::Ok);
    }
}
