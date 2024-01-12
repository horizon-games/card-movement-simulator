use std::fmt::Debug;

mod base_card;
mod bind;
mod card;
mod card_event;
mod card_game;
mod card_instance;
mod card_location;
mod card_state;
mod game_state;
mod instance_id;
mod opaque_pointer;
mod player_cards;
mod player_secret;
mod state;
mod zone;

pub mod error;

pub use {
    arcadeum::{crypto::Address, Nonce, Player, ID},
    base_card::BaseCard,
    card::Card,
    card_event::CardEvent,
    card_game::{CardGame, CardInfo, CardInfoMut, SecretCardsInfo},
    card_instance::CardInstance,
    card_location::{CardLocation, ExactCardLocation},
    card_state::CardState,
    game_state::GameState,
    instance_id::InstanceID,
    opaque_pointer::OpaquePointer,
    player_cards::PlayerCards,
    player_secret::PlayerSecret,
    state::State,
    zone::Zone,
};

pub(crate) use game_state::InstanceOrPlayer;

pub use arcadeum;

pub trait Action: arcadeum::Action + Debug {}

impl<T: arcadeum::Action + Debug> Action for T {}
pub trait AnySecretData: serde::Serialize + serde::de::DeserializeOwned + Clone {}

impl<T: serde::Serialize + serde::de::DeserializeOwned + Clone> AnySecretData for T {}

pub trait Secret<T: BaseCard>: serde::Serialize + serde::de::DeserializeOwned + Clone {
    fn attachment(&self, id: &InstanceID, base: T) -> Option<T>;

    fn reset_card(&self, id: &InstanceID, parent: T) -> T::CardState;
} //TODO: add a fn that takes the secret self, and old card info, returns default card info

pub type Context<S> = arcadeum::store::Context<
    <GameState<S> as arcadeum::store::State>::Secret,
    <GameState<S> as arcadeum::store::State>::Event,
>;
