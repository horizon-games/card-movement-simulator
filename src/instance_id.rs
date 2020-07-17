use {
    crate::{CardInstance, GameState, PlayerSecret, State},
    std::fmt::{Debug, Error, Formatter},
};

#[derive(serde::Serialize, serde::Deserialize, Copy, Clone, Hash, Eq, PartialEq)]
pub struct InstanceID(usize);

impl InstanceID {
    pub fn instance<S: State>(
        &self,
        state: &GameState<S>,
        secret: Option<&PlayerSecret<S>>,
    ) -> Option<&CardInstance<S>> {
        todo!();
    }
}

impl Debug for InstanceID {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "card #{}", self.0)
    }
}
