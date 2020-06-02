use std::fmt::{Debug, Error, Formatter};

/// A card instance ID
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Hash, Eq, PartialEq, Default)]
pub struct InstanceID(usize);

impl InstanceID {
    /// Constructs a card instance ID from a raw index
    pub(crate) fn from_raw(id: usize) -> Self {
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
