use crate::{CardInstance, CardLocation, ExactCardLocation, OpaquePointer, Player, State};

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

    /// Emitted when the field is re-ordered.
    SortField {
        player: Player,

        /// The permutation from the old field order to the new one.
        /// Each item in the array is the index of where that card *used to be*.
        /// e.g. For reordering `[a, b, c, d]` -> `[a, c, d, b]`,
        // the permutation is: `[0, 2, 3, 1]`.
        permutation: Vec<usize>,
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
            CardEvent::SortField {
                player,
                permutation,
            } => write!(f, "Player {}'s field sorted: {:?}", player, permutation),
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
                    permutation,
                },
                Self::SortField {
                    player: other_player,
                    permutation: other_permutation,
                },
            ) => player == other_player && permutation == other_permutation,
            _ => false,
        }
    }
}
