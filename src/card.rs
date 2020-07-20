use {
    crate::{InstanceID, OpaquePointer},
    std::fmt::{Debug, Error, Formatter},
};

#[derive(serde::Serialize, serde::Deserialize, Copy, Clone)]
pub enum Card {
    ID(InstanceID),
    Pointer(OpaquePointer),
}

impl Card {
    pub fn eq(&self, other: impl Into<Self>) -> Result<bool, String> {
        todo!();
    }

    pub fn ne(&self, other: impl Into<Self>) -> Result<bool, String> {
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
