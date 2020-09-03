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

#[cfg(feature = "event-eq")]
impl PartialEq for CardLocation {
    fn eq(&self, other: &CardLocation) -> bool {
        self.player == other.player
            && match (self.location, other.location) {
                (Some((zone, index)), Some((other_zone, other_index))) => {
                    zone.eq(other_zone).unwrap_or(false) && index == other_index
                }
                (None, None) => true,
                _ => false,
            }
    }
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
