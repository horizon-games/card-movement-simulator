use std::fmt::{Debug, Error, Formatter};

/// An opaque pointer to a card instance
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Hash, Eq, PartialEq, Default)]
pub struct OpaquePointer(usize);

impl OpaquePointer {
    /// Constructs an opaque pointer from a raw index
    pub(crate) fn from_raw(ptr: usize) -> Self {
        Self(ptr)
    }
}

impl From<OpaquePointer> for usize {
    fn from(ptr: OpaquePointer) -> Self {
        ptr.0
    }
}

impl Debug for OpaquePointer {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "pointer #{}", self.0)
    }
}
