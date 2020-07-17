use std::fmt::Debug;

mod base_card;
mod card;
mod card_game;
mod card_instance;
mod card_state;
mod game_state;
mod instance_id;
mod opaque_pointer;
mod player_secret;
mod player_cards;
mod state;
mod zone;

pub use {
    arcadeum::{crypto::Address, store::Event, Nonce, Player, ID},
    base_card::BaseCard,
    card::Card,
    card_game::{CardGame, CardInfo, CardInfoMut},
    card_instance::CardInstance,
    card_state::CardState,
    game_state::GameState,
    instance_id::InstanceID,
    opaque_pointer::OpaquePointer,
    player_secret::PlayerSecret,
    player_cards::PlayerCards,
    state::State,
    zone::Zone,
};

pub trait Action: arcadeum::Action + Debug {}

impl<T: arcadeum::Action + Debug> Action for T {}

pub trait Secret: serde::Serialize + serde::de::DeserializeOwned + Clone {}

pub type Context<S> = arcadeum::store::Context<GameState<S>>;
