use {
    crate::{
        Address, Card, CardGame, CardInstance, Context, InstanceID, OpaquePointer, Player,
        PlayerCards, PlayerSecret, State, Zone,
    },
    std::{
        convert::TryInto,
        future::Future,
        ops::{Deref, DerefMut},
        pin::Pin,
    },
};

#[derive(Clone)]
pub struct GameState<S: State> {
    pub(crate) instances: Vec<InstanceOrPlayer<S>>,

    player_cards: [PlayerCards; 2],

    state: S,
}

impl<S: State> Deref for GameState<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<S: State> DerefMut for GameState<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl<S: State + Default> Default for GameState<S> {
    fn default() -> GameState<S> {
        GameState::new(Default::default())
    }
}

impl<S: State> GameState<S> {
    pub fn new(state: S) -> Self {
        Self {
            instances: Default::default(),
            player_cards: Default::default(),
            state,
        }
    }

    pub fn all_player_cards(&self) -> &[PlayerCards] {
        &self.player_cards
    }

    pub fn all_player_cards_mut(&mut self) -> &mut [PlayerCards] {
        &mut self.player_cards
    }

    pub fn player_cards(&self, player: Player) -> &PlayerCards {
        &self.player_cards[usize::from(player)]
    }

    pub fn player_cards_mut(&mut self, player: Player) -> &mut PlayerCards {
        &mut self.player_cards[usize::from(player)]
    }

    pub fn exists(&self, card: impl Into<Card>) -> bool {
        let card = card.into();

        match card {
            Card::ID(id) => id.0 < self.instances.len(),
            Card::Pointer(OpaquePointer { player, index }) => {
                usize::from(player) < self.player_cards.len()
                    && index < self.player_cards(player).pointers
            }
        }
    }

    pub fn owner(&self, id: InstanceID) -> Player {
        self.zone(id).0
    }

    pub fn zone(&self, id: InstanceID) -> (Player, Option<Zone>) {
        let (owner, location) = self.location(id);

        (owner, location.map(|(zone, ..)| zone))
    }

    pub fn location(&self, id: InstanceID) -> (Player, Option<(Zone, usize)>) {
        match &self.instances[id.0] {
            InstanceOrPlayer::Instance(..) => {
                let mut locations = (0u8..self
                    .player_cards
                    .len()
                    .try_into()
                    .expect("more than 255 players"))
                    .filter_map(|player| {
                        self.player_cards(player)
                            .location(id)
                            .map(|location| (player, Some(location)))
                    });

                if let Some(location) = locations.next() {
                    assert!(locations.next().is_none());

                    location
                } else {
                    let mut parents = self.instances.iter().filter_map(|instance| {
                        instance.instance_ref().and_then(|instance| {
                            if instance.attachment == Some(id) {
                                Some(instance.id())
                            } else {
                                None
                            }
                        })
                    });

                    let parent = parents
                        .next()
                        .expect(&format!("{:?} has no owner or public parent", id));

                    assert!(parents.next().is_none());

                    (
                        self.owner(parent),
                        Some((
                            Zone::Attachment {
                                parent: parent.into(),
                            },
                            0,
                        )),
                    )
                }
            }
            InstanceOrPlayer::Player(owner) => (*owner, None),
        }
    }
}

impl<S: State> arcadeum::store::State for GameState<S> {
    type ID = S::ID;
    type Nonce = S::Nonce;
    type Action = S::Action;
    type Secret = PlayerSecret<S>;

    fn version() -> &'static [u8] {
        S::version()
    }

    fn challenge(address: &Address) -> String {
        S::challenge(address)
    }

    fn deserialize(data: &[u8]) -> Result<Self, String> {
        todo!();
    }

    fn is_serializable(&self) -> bool {
        self.serialize().is_some()
    }

    fn serialize(&self) -> Option<Vec<u8>> {
        todo!();
    }

    fn verify(&self, player: Option<Player>, action: &Self::Action) -> Result<(), String> {
        S::verify(self, player, action)
    }

    fn apply(
        self,
        player: Option<crate::Player>,
        action: &Self::Action,
        context: Context<S>,
    ) -> Pin<Box<dyn Future<Output = (Self, Context<S>)>>> {
        let action = action.clone();

        Box::pin(async move {
            let mut game = CardGame {
                state: self,
                context,
            };

            S::apply(&mut game, player, action).await;

            let CardGame { state, context } = game;

            (state, context)
        })
    }
}

#[derive(Clone)]
pub(crate) enum InstanceOrPlayer<S: State> {
    Instance(CardInstance<S>),
    Player(Player),
}

impl<S: State> InstanceOrPlayer<S> {
    pub fn instance(self) -> Option<CardInstance<S>> {
        match self {
            Self::Instance(instance) => Some(instance),
            _ => None,
        }
    }

    pub fn instance_ref(&self) -> Option<&CardInstance<S>> {
        match self {
            Self::Instance(instance) => Some(instance),
            _ => None,
        }
    }

    pub fn instance_mut(&mut self) -> Option<&mut CardInstance<S>> {
        match self {
            Self::Instance(instance) => Some(instance),
            _ => None,
        }
    }

    pub fn player(&self) -> Option<Player> {
        match self {
            Self::Player(player) => Some(*player),
            _ => None,
        }
    }
}

impl<S: State> From<CardInstance<S>> for InstanceOrPlayer<S> {
    fn from(instance: CardInstance<S>) -> Self {
        Self::Instance(instance)
    }
}

impl<S: State> From<Player> for InstanceOrPlayer<S> {
    fn from(player: Player) -> Self {
        Self::Player(player)
    }
}
