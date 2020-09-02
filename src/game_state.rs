use {
    crate::{
        Address, Card, CardGame, CardInstance, CardLocation, Context, Event, InstanceID,
        OpaquePointer, Player, PlayerCards, PlayerSecret, State, Zone,
    },
    std::{
        convert::TryInto,
        future::Future,
        ops::{Deref, DerefMut},
        pin::Pin,
    },
};

#[cfg(feature = "bindings")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg_attr(
    feature = "bindings",
    derive(typescript_definitions::TypescriptDefinition)
)]
#[derive(serde::Serialize, serde::Deserialize, Clone, Default, Debug)]
pub struct GameState<S: State> {
    #[serde(bound = "S: State")]
    pub(crate) instances: Vec<InstanceOrPlayer<S>>,

    #[serde(rename = "playerCards")]
    player_cards: [PlayerCards; 2],

    #[serde(bound = "S: State")]
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
        self.location(id).player
    }

    pub fn location(&self, id: InstanceID) -> CardLocation {
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
                            .map(|location| CardLocation {
                                player,
                                location: Some((location.0, Some(location.1))),
                            })
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
                        .unwrap_or_else(|| panic!("{:?} has no owner or public parent", id));

                    assert!(parents.next().is_none());

                    CardLocation {
                        player: self.owner(parent),
                        location: Some((
                            Zone::Attachment {
                                parent: parent.into(),
                            },
                            None,
                        )),
                    }
                }
            }
            InstanceOrPlayer::Player(owner) => CardLocation {
                player: *owner,
                location: None,
            },
        }
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub fn instances(&self) -> usize {
        self.instances.len()
    }
}

impl<S: State> arcadeum::store::State for GameState<S> {
    type ID = S::ID;
    type Nonce = S::Nonce;
    type Action = S::Action;
    type Event = Event<S>;
    type Secret = PlayerSecret<S>;

    fn version() -> &'static [u8] {
        S::version()
    }

    fn challenge(address: &Address) -> String {
        S::challenge(address)
    }

    fn deserialize(data: &[u8]) -> Result<Self, String> {
        serde_cbor::from_slice(data).map_err(|error| error.to_string())
    }

    fn is_serializable(&self) -> bool {
        true
    }

    fn serialize(&self) -> Option<Vec<u8>> {
        Some(serde_cbor::to_vec(self).unwrap())
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

#[cfg_attr(
    feature = "bindings",
    derive(typescript_definitions::TypescriptDefinition)
)]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub(crate) enum InstanceOrPlayer<S: State> {
    #[serde(bound = "S: State", rename = "instance")]
    Instance(CardInstance<S>),

    #[serde(rename = "player")]
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
