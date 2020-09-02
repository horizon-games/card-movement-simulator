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
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ExactCardLocation {
    pub player: Player,
    pub location: (Zone, usize),
}

#[cfg(feature = "event-eq")]
impl PartialEq for ExactCardLocation {
    fn eq(&self, other: &ExactCardLocation) -> bool {
        self.player == other.player
            && self.location.0.eq(other.location.0).unwrap()
            && self.location.1 == other.location.1
    }
}
