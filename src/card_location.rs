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

impl std::fmt::Display for CardLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Player {}'s {}",
            self.player,
            if let Some((zone, index)) = self.location {
                if let Some(index) = index {
                    format!("#{} card in {}", index, zone)
                } else {
                    format!("{}", zone)
                }
            } else {
                "secret location".to_string()
            }
        )
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

impl std::fmt::Display for ExactCardLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Card #{} in Player {}'s {}",
            self.location.1, self.player, self.location.0
        )
    }
}
