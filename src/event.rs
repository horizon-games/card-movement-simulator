use crate::{CardInstance, ExactCardLocation, OpaquePointer, Player, State};

#[cfg(feature = "bindings")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg_attr(
    feature = "bindings",
    derive(typescript_definitions::TypescriptDefinition)
)]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum Event<S: State> {
    /// Emitted when a card is created in public state, or a card moves from secret to public state.
    #[serde(bound = "S: State")]
    NewCard {
        instance: CardInstance<S>,
        location: ExactCardLocation,
    },

    /// Emitted when an OpaquePointer to an exact location is created.
    /// *not* emitted when new cards (and their associated pointers) are created in secret state.
    NewPointer {
        pointer: OpaquePointer,
        location: ExactCardLocation,
    },

    #[serde(bound = "S: State")]
    ResetCard { instance: CardInstance<S> },
    #[serde(bound = "S: State")]
    ModifyCard { instance: CardInstance<S> },
    #[serde(bound = "S: State")]
    MoveCard {
        instance: Option<CardInstance<S>>,
        from: ExactCardLocation,
        to: ExactCardLocation,
    },

    /// Emitted when the field is re-ordered.
    SortField {
        player: Player,
        permutation: Vec<usize>,
    },

    /// Game-specific event
    GameEvent { event: S::Event },
}

#[cfg(feature = "event-eq")]
impl<S: State> PartialEq for Event<S> {
    fn eq(&self, other: &Event<S>) -> bool {
        match (self, other) {
            (
                Self::NewCard { instance, location },
                Self::NewCard {
                    instance: other_instance,
                    location: other_location,
                },
            ) if instance == other_instance && location == other_location => true,
            _ => false,
        }
    }
}
