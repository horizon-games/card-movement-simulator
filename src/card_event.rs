use crate::{
    CardInstance, CardLocation, ExactCardLocation, InstanceID, OpaquePointer, Player, State,
};

#[cfg(feature = "bindings")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg_attr(
    feature = "bindings",
    derive(typescript_definitions::TypescriptDefinition)
)]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum CardEvent<S: State> {
    /// Emitted when an OpaquePointer to an exact location is created.
    /// *not* emitted when new cards (and their associated pointers) are created in secret state.
    NewPointer {
        pointer: OpaquePointer,
        location: ExactCardLocation,
    },

    /// Emitted when a card in public state or in the client's secret state changes.
    #[serde(bound = "S: State")]
    ModifyCard { instance: CardInstance<S> },

    /// Emitted when a card moves zones.
    #[serde(bound = "S: State")]
    MoveCard {
        /// Will be Some(..) if the card is in public state or in the client's secret state.
        /// If the card has an attachment, it'll be provided in this tuple.
        instance: Option<(CardInstance<S>, Option<CardInstance<S>>)>,
        from: CardLocation,
        to: ExactCardLocation,
    },

    /// Emitted when a deck is shuffled.
    ShuffleDeck {
        player: Player,
        deck: Vec<InstanceID>,
    },

    /// Emitted when the field is re-ordered.
    SortField {
        player: Player,
        field: Vec<InstanceID>,
        real: bool,
    },

    /// Game-specific event.
    #[serde(deserialize_with = "deserialize_game_event")]
    GameEvent {
        #[cfg_attr(feature = "bindings", ts(ts_type = "GameEvent"))]
        event: S::Event,
    },
}

fn deserialize_game_event<'de, D: serde::Deserializer<'de>, T>(_: D) -> Result<T, D::Error> {
    unreachable!("attempted to deserialize an CardEvent::GameEvent");
}

impl<S: State> std::fmt::Display for CardEvent<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CardEvent::NewPointer { pointer, location } => {
                write!(f, "New Pointer #{:?} to {:?})", pointer, location)
            }
            CardEvent::ModifyCard { instance } => write!(f, "Card #{:?} modified", instance.id),
            CardEvent::MoveCard { instance, from, to } => write!(
                f,
                "Card moved from {} to {} with{} instance",
                from,
                to,
                if instance.is_some() { "" } else { "out" }
            ),
            CardEvent::ShuffleDeck { player, deck } => {
                write!(f, "Player {}'s deck shuffled: {:?}", player, deck)
            }
            CardEvent::SortField {
                player,
                field,
                real,
            } => {
                write!(
                    f,
                    "Player {}'s field sorted {}: {:?}",
                    player,
                    if *real { "really" } else { "fakely" },
                    field
                )
            }
            CardEvent::GameEvent { .. } => write!(f, "Game Event"),
        }
    }
}

#[cfg(feature = "event-eq")]
impl<S: State> PartialEq for CardEvent<S> {
    fn eq(&self, other: &CardEvent<S>) -> bool {
        match (self, other) {
            (
                Self::NewPointer { pointer, location },
                Self::NewPointer {
                    pointer: other_pointer,
                    location: other_location,
                },
            ) => pointer == other_pointer && location == other_location,
            (
                Self::ModifyCard { instance },
                Self::ModifyCard {
                    instance: other_instance,
                },
            ) => instance == other_instance,
            (
                Self::MoveCard { instance, from, to },
                Self::MoveCard {
                    instance: other_instance,
                    from: other_from,
                    to: other_to,
                },
            ) => instance == other_instance && from == other_from && to == other_to,
            (
                Self::SortField {
                    player,
                    field,
                    real,
                },
                Self::SortField {
                    player: other_player,
                    field: other_field,
                    real: other_real,
                },
            ) => player == other_player && field == other_field && real == other_real,
            _ => false,
        }
    }
}
