use std::fmt::{Debug, Error, Formatter};

/// A card instance ID
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Hash, Eq, PartialEq, Default)]
pub struct InstanceID(usize);

impl InstanceID {
    /// Gets the instance with this ID for a given [super::CardGame] and optional [super::CardGameSecret]
    pub fn instance<'a, S: super::State>(
        &self,
        state: &'a super::CardGame<S>,
        secret: Option<&'a super::CardGameSecret<S::Secret>>,
    ) -> Option<&'a super::CardInstance<<S::Secret as super::Secret>::BaseCard>> {
        state.cards[self.0]
            .instance_ref()
            .or_else(|| secret.and_then(|secret| secret.cards.get(self)))
    }

    #[doc(hidden)]
    /// Constructs a card instance ID from a raw index
    pub fn from_raw(id: usize) -> Self {
        Self(id)
    }
}

impl From<InstanceID> for usize {
    fn from(id: InstanceID) -> Self {
        id.0
    }
}

impl Debug for InstanceID {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "card #{}", self.0)
    }
}
