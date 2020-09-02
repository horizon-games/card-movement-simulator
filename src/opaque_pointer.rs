use {
    crate::Player,
    std::fmt::{Debug, Error, Formatter},
};

#[cfg(feature = "bindings")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg_attr(feature = "card-event-eq", derive(PartialEq))]
#[cfg_attr(
    feature = "bindings",
    derive(typescript_definitions::TypescriptDefinition)
)]
#[derive(serde::Serialize, serde::Deserialize, Copy, Clone)]
pub struct OpaquePointer {
    pub(crate) player: Player,
    pub(crate) index: usize,
}

impl Debug for OpaquePointer {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "player {} pointer #{}", self.player, self.index)
    }
}
