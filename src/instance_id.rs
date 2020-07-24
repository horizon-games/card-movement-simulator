use {
    crate::{CardInstance, GameState, InstanceOrPlayer, PlayerSecret, State},
    std::fmt::{Debug, Error, Formatter},
};

#[derive(serde::Serialize, serde::Deserialize, Copy, Clone, Hash, Eq, PartialEq)]
pub struct InstanceID(pub(crate) usize);

impl InstanceID {
    #[doc(hidden)]
    /// Internal-only API! Creates an instance ID from a usize.
    /// Never use this in prod-facing code.
    pub fn from_raw(raw: usize) -> InstanceID {
        InstanceID(raw)
    }
    pub fn instance<'a, S: State>(
        &self,
        state: &'a GameState<S>,
        secret: Option<&'a PlayerSecret<S>>,
    ) -> Option<&'a CardInstance<S>> {
        match &state.instances[self.0] {
            InstanceOrPlayer::Instance(instance) => Some(instance),
            InstanceOrPlayer::Player(owner) => secret.and_then(|secret| {
                if secret.player() == *owner {
                    Some(&secret.instances[self])
                } else {
                    assert!(!secret.instances.contains_key(self));

                    None
                }
            }),
        }
    }
}

impl Debug for InstanceID {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "card #{}", self.0)
    }
}
