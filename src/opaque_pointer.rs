use {
    crate::Player,
    std::fmt::{Debug, Error, Formatter},
};

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
