use std::fmt::Debug;

mod base_card;
mod bind;
mod card;
mod card_game;
mod card_instance;
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
    arcadeum::{crypto::Address, store::Event, Nonce, Player, ID},
    base_card::BaseCard,
    card::Card,
    card_game::{CardGame, CardInfo, CardInfoMut, CardLocation, MoveEvent},
    card_instance::CardInstance,
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

pub trait Action: arcadeum::Action + Debug {}

impl<T: arcadeum::Action + Debug> Action for T {}

pub trait Secret: serde::Serialize + serde::de::DeserializeOwned + Clone {}

impl<T: serde::Serialize + serde::de::DeserializeOwned + Clone> Secret for T {}

pub type Context<S> = arcadeum::store::Context<GameState<S>>;
