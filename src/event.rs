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
    #[serde(bound = "S: State")]
    NewCard {
        instance: CardInstance<S>,
        location: ExactCardLocation,
    },
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
        from: CardLocation,
        to: CardLocation,
    },
    SortField {
        player: Player,
        permutation: Vec<usize>,
    },
}
