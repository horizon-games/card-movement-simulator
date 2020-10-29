use {
    crate::{
        error, Address, Card, CardEvent, CardGame, CardInstance, CardLocation, Context, InstanceID,
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
#[serde(rename_all = "camelCase")]
pub struct GameState<S: State> {
    #[serde(bound = "S: State")]
    pub(crate) instances: Vec<InstanceOrPlayer<S>>,

    player_cards: [PlayerCards; 2],

    pub(crate) shuffle_deck_on_insert: bool,

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
    pub fn new(state: S, shuffle_deck_on_insert: bool) -> Self {
        Self {
            instances: Default::default(),
            player_cards: Default::default(),
            shuffle_deck_on_insert,
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
    pub fn ok(&self, secrets: &[Option<&PlayerSecret<S>>]) -> Result<(), error::RevealOkError> {
        let any_pointer_out_of_bounds = secrets.iter().flatten().any(|secret| {
            secret
                .pointers
                .iter()
                .any(|pointer| pointer.0 >= self.instances.len())
        });
        if any_pointer_out_of_bounds {
            return Err(error::RevealOkError::Error {
                err: "pointer out of bounds".into(),
            });
        }

        // Only one bucket may contain the CardInstance for an InstanceID.
        // If a CardInstance has an attachment, the attachment must be in the same Bucket.
        let real_instance_ids = self
            .instances
            .iter()
            .flat_map(|card| card.instance_ref().map(|instance| instance.id))
            .chain(
                secrets
                    .iter()
                    .flatten()
                    .flat_map(|secret| secret.instances.keys().copied()),
            );

        for id in real_instance_ids.clone() {
            match &self.instances[id.0] {
                InstanceOrPlayer::Player(player) => {
                    // The card must be in that player's secret cards

                    if let Some(secret) = secrets[usize::from(*player)] {
                        if !secret.instances.contains_key(&id) {
                            return Err(error::RevealOkError::Error {
                                err: format!("Card should have been in player {}'s secret", player),
                            });
                        }
                    }

                    // The card must not be in the other player's secret cards

                    if let Some(secret) = secrets[usize::from(1 - *player)] {
                        if secret.instances.contains_key(&id) {
                            return Err(error::RevealOkError::Error {
                                err: format!(
                                    "Card should not have been in player {}'s secret",
                                    1 - player
                                ),
                            });
                        }
                    }

                    // The instance's attachment, if any, should also be in this player's secret.

                    if let Some(secret) = secrets[usize::from(*player)] {
                        if let Some(attachment_id) = secret.instance(id).unwrap().attachment {
                            if !secret.instances.contains_key(&attachment_id) {
                                return Err(error::RevealOkError::Error {
                                    err: format!(
                                        "Card's attachment should have been in player {}'s secret",
                                        player
                                    ),
                                });
                            }
                        }
                    }
                }
                InstanceOrPlayer::Instance(instance) => {
                    // The card shouldn't be in either player's secret cards

                    for (player, secret) in secrets
                        .iter()
                        .enumerate()
                        .filter_map(|(player, secret)| secret.map(|secret| (player, secret)))
                    {
                        if secret.instances.contains_key(&id) {
                            return Err(error::RevealOkError::Error {
                                err: format!(
                                    "InstanceID {:?} is both public and in player {:?}'s secret",
                                    id, player
                                ),
                            });
                        }
                    }

                    // The instance's attachment, if any, should also be public.

                    if let Some(attachment) = instance.attachment {
                        if let InstanceOrPlayer::Player(player) = self.instances[attachment.0] {
                            return Err(error::RevealOkError::Error {
                                err: format!("The instance for card {} is public, but its attachment {} is in player {}'s secret", id.0, attachment.0, player)});
                        }
                    }
                }
            }
        }

        // An InstanceID must occur in all zones combined at most once.
        // It can be 0, because some InstanceIDs correspond to non-existent attachments.
        for id in 0..self.instances.len() {
            let id = InstanceID(id);

            // Count the number of times id occurs in public and secret state.

            let mut count = 0;

            for (player_id, player) in self.all_player_cards().iter().enumerate() {
                let player_id: Player =
                    player_id
                        .try_into()
                        .map_err(|error| error::RevealOkError::Error {
                            err: format!("{}", error),
                        })?;

                count += player
                    .hand
                    .iter()
                    .filter(|hand_id| **hand_id == Some(id))
                    .count();
                count += player
                    .field
                    .iter()
                    .filter(|field_id| **field_id == id)
                    .count();
                count += player
                    .graveyard
                    .iter()
                    .filter(|graveyard_id| **graveyard_id == id)
                    .count();
                count += player
                    .limbo
                    .iter()
                    .filter(|limbo_id| **limbo_id == id)
                    .count();
                count += player
                    .casting
                    .iter()
                    .filter(|casting_id| **casting_id == id)
                    .count();
                count += player.dust.iter().filter(|dust_id| **dust_id == id).count();
                count += self
                    .instances
                    .iter()
                    .filter(|card| {
                        if let InstanceOrPlayer::Instance(instance) = card {
                            instance.attachment == Some(id) && self.owner(instance.id) == player_id
                        } else {
                            false
                        }
                    })
                    .count();
            }

            for secret in secrets.iter().flatten() {
                count += secret.deck.iter().filter(|deck_id| **deck_id == id).count();
                count += secret
                    .hand
                    .iter()
                    .filter(|hand_id| **hand_id == Some(id))
                    .count();
                count += secret
                    .limbo
                    .iter()
                    .filter(|limbo_id| **limbo_id == id)
                    .count();
                count += secret.dust.iter().filter(|dust_id| **dust_id == id).count();
                count += secret
                    .card_selection
                    .iter()
                    .filter(|card_selection_id| **card_selection_id == id)
                    .count();
                count += secret
                    .instances
                    .values()
                    .filter(|instance| instance.attachment == Some(id))
                    .count();
            }

            if count > 1 {
                return Err(error::RevealOkError::Error {
                    err: format!(
                        "Instance ID {} occurs {} times in public and secret state",
                        id.0, count
                    ),
                });
            }
        }

        // If an instance is public, it should be in a public zone.
        // If an instance is secret, it should be in a secret zone.

        for id in real_instance_ids {
            match self.instances[id.0] {
                InstanceOrPlayer::Player(player) => {
                    if let Some(secret) = secrets[usize::from(player)] {
                        if secret.location(id).location.is_none() {
                            return Err(error::RevealOkError::Error {
                                err: format!(
                            "{:?} is in player {}'s secret bucket, but not in any of their zzones",
                            id, player
                        ),
                            });
                        }
                    }
                }
                InstanceOrPlayer::Instance(..) => {
                    self.owner(id);
                }
            }
        }

        // Public state deck must match secret state deck length.
        for (player_id, player) in self.all_player_cards().iter().enumerate() {
            if let Some(secret) = secrets[player_id] {
                if secret.deck.len() != player.deck {
                    return Err(error::RevealOkError::Error {
                        err: format!(
                        "Player {}'s public deck size is {}, but their private deck size is {}.",
                        player_id,
                        player.deck,
                        secret.deck.len()
                    ),
                    });
                }
            }
        }

        // Public state card selection must match secret state card selection length.
        for (player_id, player) in self.all_player_cards().iter().enumerate() {
            if let Some(secret) = secrets[player_id] {
                if secret.card_selection.len() != player.card_selection {
                    return Err(error::RevealOkError::Error {
                    err: format!("Player {}'s public card selection size is {}, but their private card selection size is {}.", player_id, player.card_selection, secret.card_selection.len())
                }
                );
                }
            }
        }

        // For each card in Public & Secret hand, if one Bucket has None, the other must have Some(ID).
        for (player, secret) in self
            .all_player_cards()
            .iter()
            .zip(secrets.iter())
            .filter_map(|(player, secret)| secret.map(|secret| (player, secret)))
        {
            for (index, (public_hand, secret_hand)) in
                player.hand.iter().zip(secret.hand.iter()).enumerate()
            {
                match (public_hand, secret_hand) {
                    (Some(_), None) | (None, Some(_)) => {
                        // ok! only one state has it
                    }
                    (Some(public_some), Some(private_some)) => {
                        return Err(error::RevealOkError::Error {
                            err: format!("Both public state & private state({:?}) have Some(_) at hand position {:?} .\nPublic: Some({:?}), Private: Some({:?})", player, index, public_some, private_some)});
                    }
                    (None, None) => {
                        return Err(error::RevealOkError::Error {
                            err: "Both public & private state have None at this hand position."
                                .to_string(),
                        });
                    }
                }
            }
        }

        for id in self
            .instances
            .iter()
            .flat_map(|card| card.instance_ref().map(|instance| instance.id))
        {
            self.owner(id); // should be able to call owner for each public id
        }

        Ok(())
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
    type Event = CardEvent<S>;
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
