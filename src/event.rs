use crate::CardLocation;

#[cfg(feature = "bindings")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg_attr(
    feature = "bindings",
    derive(typescript_definitions::TypescriptDefinition)
)]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum CMSEvent {
    MoveCard {
        from: CardLocation,
        to: CardLocation,
    },
}
