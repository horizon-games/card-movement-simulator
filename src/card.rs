use {
    crate::{error, InstanceID, OpaquePointer},
    std::fmt::{Debug, Error, Formatter},
};

#[derive(serde::Serialize, serde::Deserialize, Copy, Clone)]
pub enum Card {
    ID(InstanceID),
    Pointer(OpaquePointer),
}

impl Card {
    pub fn eq(&self, other: impl Into<Self>) -> Result<bool, error::CardEqualityError> {
        let other = other.into();

        match self {
            Self::ID(id) => match other {
                Self::ID(other_id) => Ok(other_id == *id),
                Self::Pointer(..) => {
                    Err(error::CardEqualityError::IncomparableCards { a: *self, b: other })
                }
            },
            Self::Pointer(OpaquePointer { player, index }) => match other {
                Self::ID(..) => {
                    Err(error::CardEqualityError::IncomparableCards { a: *self, b: other })
                }
                Self::Pointer(OpaquePointer {
                    player: other_player,
                    index: other_index,
                }) => {
                    if other_player == *player && other_index == *index {
                        Ok(true)
                    } else {
                        Err(error::CardEqualityError::IncomparableCards { a: *self, b: other })
                    }
                }
            },
        }
    }

    pub fn ne(&self, other: impl Into<Self>) -> Result<bool, error::CardEqualityError> {
        Ok(!self.eq(other)?)
    }
}

impl From<&Card> for Card {
    fn from(id: &Card) -> Self {
        *id
    }
}

impl From<InstanceID> for Card {
    fn from(id: InstanceID) -> Self {
        Self::ID(id)
    }
}

impl From<&InstanceID> for Card {
    fn from(id: &InstanceID) -> Self {
        Self::ID(*id)
    }
}

impl From<OpaquePointer> for Card {
    fn from(ptr: OpaquePointer) -> Self {
        Self::Pointer(ptr)
    }
}

impl From<&OpaquePointer> for Card {
    fn from(ptr: &OpaquePointer) -> Self {
        Self::Pointer(*ptr)
    }
}

impl Debug for Card {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self {
            Self::ID(id) => id.fmt(f),
            Self::Pointer(ptr) => ptr.fmt(f),
        }
    }
}
