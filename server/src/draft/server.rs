use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::cards::Card;

#[derive(serde::Serialize)]
pub enum DraftServerMessage {
    Ended,
    Connected {
        draft: Uuid,
        seat: Uuid,
        pool: Vec<Card>,
        pack: Option<Vec<Card>>,
    },
}

pub enum DraftServerRequest {
    Connect(Uuid, UnboundedSender<DraftServerMessage>),
}

struct DraftServer {}
