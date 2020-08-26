use crate::{Player, Zone};

#[cfg(feature = "bindings")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg_attr(
    feature = "bindings",
    derive(typescript_definitions::TypescriptDefinition)
)]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CardLocation {
    pub player: Player,
    pub location: Option<(Zone, Option<usize>)>,
}

#[cfg_attr(
    feature = "bindings",
    derive(typescript_definitions::TypescriptDefinition)
)]
pub struct ExactCardLocation {
    pub player: Player,
    pub location: (Zone, usize),
}
