use crate::{CardInstance, CardLocation, OpaquePointer, State};

#[cfg(feature = "bindings")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg_attr(
    feature = "bindings",
    derive(typescript_definitions::TypescriptDefinition)
)]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum CardEvent<S: State> {
    #[serde(bound = "S: State")]
    NewCard {
        instance: CardInstance<S>,
        location: CardLocation,
        #[serde(rename = "isAttachment")]
        is_attachment: bool,
    },
    NewPointer {
        pointer: OpaquePointer,
        location: CardLocation,
    },
    #[serde(bound = "S: State")]
    ResetCard {
        instance: CardInstance<S>,
        location: CardLocation,
    },
    #[serde(bound = "S: State")]
    ModifyCard {
        instance: CardInstance<S>,
        location: CardLocation,
    },
    #[serde(bound = "S: State")]
    MoveCard {
        instance: Option<CardInstance<S>>,
        from: CardLocation,
        to: CardLocation,
    },
}
