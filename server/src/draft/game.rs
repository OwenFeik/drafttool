use std::collections::{HashMap, VecDeque};

use uuid::Uuid;

use crate::{cards::Card, err, Res};

use super::packs::Pack;

#[derive(Clone, Copy)]
enum PassDirection {
    Left,
    Right,
}

impl PassDirection {
    fn reverse(self) -> Self {
        match self {
            PassDirection::Left => PassDirection::Right,
            PassDirection::Right => PassDirection::Left,
        }
    }
}

pub type NewPacks = Vec<(Uuid, Pack)>;

pub struct Draft {
    players: Vec<Uuid>,
    pools: HashMap<Uuid, Vec<Card>>,
    direction: PassDirection,
    rounds: usize,
    current_round: usize,
    generated_packs: Vec<Pack>,
    packs_being_drafted: HashMap<Uuid, VecDeque<Pack>>,
}

impl Draft {
    pub fn new(players: Vec<Uuid>, rounds: usize, packs: Vec<Pack>) -> Self {
        debug_assert!(packs.len() == players.len() * rounds);

        Self {
            players,
            pools: HashMap::new(),

            // Reversed to left at beginning of first round.
            direction: PassDirection::Right,
            current_round: 0,
            rounds,
            generated_packs: packs,
            packs_being_drafted: HashMap::new(),
        }
    }

    /// Start the draft, choosing a pack for each player. Returns a vector of
    /// (player, pack) pairs for each player to make their pick from. This may
    /// only be called once to begin the draft. Future rounds will begin when
    /// the previous round finishes.
    pub fn begin(&mut self) -> Vec<(Uuid, Vec<Card>)> {
        debug_assert!(self.current_round == 0);

        self.start_round()
    }

    /// Given a player and an index to pick from that player's current pack,
    /// attempt to choose that card and pass the pack along to the next player.
    /// If the pick succeeds, returns a vector with up to two elements. Each
    /// element is a (player, new_pack) pair. One element will be produced for
    /// the picking player if they have a new pack available to pick from and
    /// one will be produced for the player next in the draft after the picking
    /// player if the pack the picking player is passing is not empty.
    pub fn handle_pick(&mut self, player: Uuid, index: usize) -> Res<(Card, NewPacks)> {
        let (card, pack) = self.pick_card(player, index)?;
        self.pool_for(player).push(card.clone());

        let mut newly_available_packs = Vec::new();
        let next = self.next_player(player);
        if !pack.is_empty()
            && let Some(next_player) = next
        {
            self.stack_for(next_player).push_back(pack);
            if let Some(next_player_stack) = self.packs_being_drafted.get(&next_player)
                && next_player_stack.len() == 1
            {
                newly_available_packs
                    .push((next_player, next_player_stack.front().cloned().unwrap()));
            }
        }
        if next != Some(player)
            && let Some(next_pack) = self
                .packs_being_drafted
                .get(&player)
                .and_then(|stack| stack.front().cloned())
        {
            newly_available_packs.push((player, next_pack));
        }

        // If this was the last pick in the round, begin the next.
        if newly_available_packs.is_empty() && self.round_finished() && !self.draft_complete() {
            Ok((card, self.start_round()))
        } else {
            Ok((card, newly_available_packs))
        }
    }

    /// Get the pack currently being drafted by this player, if any.
    pub fn current_pack(&self, player: Uuid) -> Option<Vec<Card>> {
        self.packs_being_drafted
            .get(&player)
            .and_then(|stack| stack.front())
            .cloned()
    }

    /// Get the pool of cards drafted by this player, if any.
    pub fn drafted_cards(&self, player: Uuid) -> Option<&Vec<Card>> {
        self.pools.get(&player)
    }

    /// Check if this draft is completed. This is true when the final card has
    /// been drafted from the final round.
    pub fn draft_complete(&self) -> bool {
        self.current_round == self.rounds && self.round_finished()
    }

    /// Map from player ID to pool of picked cards.
    pub fn pools(&self) -> &HashMap<Uuid, Vec<Card>> {
        &self.pools
    }

    /// Get the number of queued of packs for this player.
    pub fn queue_size(&self, player: Uuid) -> usize {
        self.packs_being_drafted
            .get(&player)
            .map(|stack| stack.len())
            .unwrap_or(0)
    }

    /// Find the player in the draft that the given player is passing to in the
    /// current round. Takes account of the the current pass direction.
    fn next_player(&self, player: Uuid) -> Option<Uuid> {
        let index = self.players.iter().position(|p| *p == player)?;
        match self.direction {
            PassDirection::Left => {
                if index == 0 {
                    self.players.last().copied()
                } else {
                    self.players.get(index - 1).copied()
                }
            }
            PassDirection::Right => {
                if index == self.players.len() - 1 {
                    self.players.first().copied()
                } else {
                    self.players.get(index + 1).copied()
                }
            }
        }
    }

    /// Get a mutable reference to the pool of picked cards for the specified
    /// player, creating it if necessary.
    fn pool_for(&mut self, player: Uuid) -> &mut Vec<Card> {
        debug_assert!(self.players.contains(&player));

        self.pools.entry(player).or_default()
    }

    /// Get a mutable reference to the stack of packs waiting for the specified
    /// player to draft, creating it if necessary.
    fn stack_for(&mut self, player: Uuid) -> &mut VecDeque<Vec<Card>> {
        debug_assert!(self.players.contains(&player));

        self.packs_being_drafted.entry(player).or_default()
    }

    /// Check if this draft round is complete. This is the case when all packs
    /// have been emptied and removed from stacks.
    fn round_finished(&self) -> bool {
        self.packs_being_drafted
            .values()
            .all(|pack_stack| pack_stack.is_empty())
    }

    /// Begin a new round of the draft. Handles reversing the draft direction,
    /// assigning the new pack from the pool, etc. Returns a vector of pairs of
    /// player ID and new pack for that player.
    fn start_round(&mut self) -> Vec<(Uuid, Vec<Card>)> {
        debug_assert!(self.round_finished());
        debug_assert!(!self.draft_complete());
        debug_assert!(self.current_round < self.rounds);
        debug_assert!(self.generated_packs.len() >= self.players.len());

        self.current_round += 1;
        self.direction = self.direction.reverse();

        // Give each player a pack. Clone required so that self can be mutated
        // in the loop.
        for player in self.players.clone().into_iter() {
            let pack = self.generated_packs.pop().unwrap();
            self.stack_for(player).push_back(pack);
        }
        // Return a collection mapping each player to the pack they need to pick
        // from. This unwrap is ok as we just added a pack to each players
        // pack stack
        self.packs_being_drafted
            .iter()
            .map(|(player, stack)| (*player, stack.front().cloned().unwrap()))
            .collect()
    }

    /// Attempt to perform a pick for a player at a given index in the player's
    /// current pack. On success returns the picked card and the pack (now
    /// removed from the players pack stack). On failure (if the player has no
    /// active pack or the index is invalid) returns None.
    fn pick_card(&mut self, player: Uuid, index: usize) -> Res<(Card, Pack)> {
        let Some(pack_stack) = self.packs_being_drafted.get_mut(&player) else {
            return err("Player not in draft.");
        };
        let Some(current_pack) = pack_stack.front_mut() else {
            return err("No current pack.");
        };
        if index < current_pack.len() {
            Ok((current_pack.remove(index), pack_stack.pop_front().unwrap()))
        } else {
            err("Invalid pick index.")
        }
    }
}

#[cfg(test)]
mod test {
    use uuid::Uuid;

    use crate::draft::{
        game::PassDirection,
        packs::{make_packs, DraftPool},
        DraftConfig,
    };

    use super::Draft;

    fn packless_draft(players: Vec<Uuid>) -> Draft {
        Draft::new(players, 0, Vec::new())
    }

    #[test]
    fn test_next_player_empty() {
        let mut draft = packless_draft(Vec::new());
        assert_eq!(draft.next_player(Uuid::new_v4()), None);
        draft.direction = draft.direction.reverse();
        assert_eq!(draft.next_player(Uuid::new_v4()), None);
    }

    #[test]
    fn test_next_player_single() {
        let id = Uuid::new_v4();
        let mut draft = packless_draft(vec![id]);

        assert_eq!(draft.next_player(id), Some(id));
        assert_eq!(draft.next_player(Uuid::new_v4()), None);
        draft.direction = draft.direction.reverse();
        assert_eq!(draft.next_player(id), Some(id));
        assert_eq!(draft.next_player(Uuid::new_v4()), None);
    }

    #[test]
    fn test_next_player() {
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        let p3 = Uuid::new_v4();
        let p4 = Uuid::new_v4();
        let mut draft = packless_draft(vec![p1, p2, p3, p4]);

        draft.direction = PassDirection::Left;
        assert_eq!(draft.next_player(p1), Some(p4));
        assert_eq!(draft.next_player(p2), Some(p1));
        assert_eq!(draft.next_player(p3), Some(p2));
        assert_eq!(draft.next_player(p4), Some(p3));
        assert_eq!(draft.next_player(Uuid::new_v4()), None);
        draft.direction = draft.direction.reverse();
        assert_eq!(draft.next_player(p1), Some(p2));
        assert_eq!(draft.next_player(p2), Some(p3));
        assert_eq!(draft.next_player(p3), Some(p4));
        assert_eq!(draft.next_player(p4), Some(p1));
        assert_eq!(draft.next_player(Uuid::new_v4()), None);
    }

    #[test]
    fn test_simple_draft() {
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        let p3 = Uuid::new_v4();
        let p4 = Uuid::new_v4();
        let players = vec![p1, p2, p3, p4];

        let config = DraftConfig {
            unique_cards: false,
            ..Default::default()
        };
        let pool = DraftPool::sample(1, 1, 1, 1);
        let packs = make_packs(players.len(), &config, pool).unwrap();

        let mut draft = Draft::new(players.clone(), config.rounds, packs);

        let player_packs = draft.begin();

        // Check that all players were assigned a pack.
        assert!(players.iter().all(|player| player_packs
            .iter()
            .any(|(pack_player, _)| pack_player == player)));

        // Pick at invalid index should fail.
        assert!(draft.handle_pick(p1, config.cards_per_pack).is_err());
        // Pick from a player who is not in the draft should fail.
        assert!(draft.handle_pick(Uuid::new_v4(), 0).is_err());
        // There are no packs waiting and the next player already has a pack.
        assert!(draft.handle_pick(p1, 0).unwrap().1.is_empty());
        assert!(draft.handle_pick(p3, 5).unwrap().1.is_empty());

        // No pack available, should be rejected.
        assert!(draft.handle_pick(p1, 0).is_err());

        // When player two makes a pick, we should have 2 updates. Player 2
        // should have a pack available as player 3 has already picked and
        // player 1 now has a pack available.
        let updates = draft.handle_pick(p2, 10).unwrap().1;
        assert!(updates.len() == 2);
        assert!(updates.iter().any(|(player, _)| *player == p1));
        assert!(updates.iter().any(|(player, _)| *player == p2));
        assert!(updates
            .iter()
            .all(|(_, pack)| pack.len() == config.cards_per_pack - 1));
        assert!(draft.handle_pick(p4, 14).unwrap().1.len() == 2);

        // Pick all but 1 of the rest of the cards.
        for _ in 0..(config.cards_per_pack - 2) {
            for &player in &players {
                assert!(draft.handle_pick(player, 0).is_ok());
            }
        }

        // All players should have their final pick remaining.
        assert!(players
            .iter()
            .all(|&player| draft.current_pack(player).unwrap().len() == 1
                && draft.drafted_cards(player).unwrap().len() == 14));
        assert!(draft.handle_pick(p1, 1).is_err());
        assert!(draft.handle_pick(p1, 0).is_ok());
        assert!(draft.handle_pick(p2, 0).is_ok());
        assert!(draft.handle_pick(p3, 0).is_ok());

        // Final pick for p4 is the final pick of the round. The new round
        // should begin.
        let updates = draft.handle_pick(p4, 0).unwrap().1;
        assert!(updates.len() == 4);
        assert!(players
            .iter()
            .all(|player| updates.iter().any(|(pack_player, _)| player == pack_player)));

        // Passing should be in the opposite direction now.
        assert!(draft.handle_pick(p1, 0).unwrap().1.is_empty());
        assert!(draft
            .handle_pick(p4, 0)
            .unwrap()
            .1
            .into_iter()
            .any(|(pack_player, _)| pack_player == p1));
        assert!(!draft.handle_pick(p2, 0).unwrap().1.is_empty());
        assert!(!draft.handle_pick(p3, 0).unwrap().1.is_empty());

        // Complete pack 2 and pack 3.
        for _ in 0..(config.cards_per_pack * 2 - 1) {
            for &player in &players {
                assert!(draft.handle_pick(player, 0).is_ok());
            }
        }

        // Validate that the draft is completed with all packs used and all
        // players having drafted the appropriate number of cards.
        assert!(draft.draft_complete());
        assert!(draft.generated_packs.is_empty());
        assert!(draft.handle_pick(p1, 0).is_err());
        assert!(players
            .iter()
            .all(|&player| draft.drafted_cards(player).unwrap().len()
                == config.rounds * config.cards_per_pack));
    }

    #[test]
    fn test_single_player() {
        let p = Uuid::new_v4();

        let config = &DraftConfig {
            rounds: 1,
            cards_per_pack: 4,
            unique_cards: false,
            rares: 1,
            uncommons: 1,
            commons: 2,
            ..Default::default()
        };
        let pool = DraftPool::sample(1, 1, 1, 1);
        let packs = make_packs(1, config, pool).unwrap();
        let mut draft = Draft::new(vec![p], 1, packs);

        assert!(draft.begin().len() == 1);

        let result = draft.handle_pick(p, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().1.len(), 1);
        assert!(!draft.draft_complete());
    }
}
