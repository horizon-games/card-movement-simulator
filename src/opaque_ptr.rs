use std::fmt::{Debug, Error, Formatter};

/// An opaque pointer to a card instance
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Hash, Eq, PartialEq, Default)]
pub struct OpaquePointer(usize);

impl OpaquePointer {
    /// Gets the ID of the instance this pointer points to for a given [super::CardGame] and optional [super::CardGameSecret]
    pub fn id<S: super::State>(
        &self,
        state: &super::CardGame<S>,
        secret: Option<&super::CardGameSecret<S::Secret>>,
    ) -> Option<super::InstanceID> {
        state.opaque_ptrs[self.0]
            .id()
            .or_else(|| secret.and_then(|secret| secret.opaque_ptrs.get(self).copied()))
    }

    /// Gets the instance this pointer points to for a given [super::CardGame] and optional [super::CardGameSecret]
    pub fn instance<'a, S: super::State>(
        &self,
        state: &'a super::CardGame<S>,
        secret: Option<&'a super::CardGameSecret<S::Secret>>,
    ) -> Option<&'a super::CardInstance<<S::Secret as super::Secret>::BaseCard>> {
        self.id(state, secret)
            .and_then(|id| id.instance(state, secret))
    }

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
