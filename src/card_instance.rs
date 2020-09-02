use {
    crate::{BaseCard, InstanceID, State},
    std::ops::{Deref, DerefMut},
};

#[cfg(feature = "bindings")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg_attr(
    feature = "bindings",
    derive(typescript_definitions::TypescriptDefinition)
)]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CardInstance<S: State> {
    pub(crate) id: InstanceID,

    #[serde(bound = "S: State")]
    pub(crate) base: S::BaseCard,

    pub(crate) attachment: Option<InstanceID>,

    #[serde(bound = "S: State")]
    pub(crate) state: <S::BaseCard as BaseCard>::CardState,
}

impl<S: State> Deref for CardInstance<S> {
    type Target = <S::BaseCard as BaseCard>::CardState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<S: State> DerefMut for CardInstance<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

#[cfg(feature = "card-event-eq")]
impl<S: State> PartialEq for CardInstance<S> {
    fn eq(&self, other: &Self) -> bool {
        use crate::card_state::CardState;
        self.id == other.id
            && self.base == other.base
            && self.attachment == other.attachment
            && self.state.eq(&other.state)
    }
}

impl<S: State> CardInstance<S> {
    #[doc(hidden)]
    /// Internal-only API.
    pub fn from_raw(
        id: InstanceID,
        base: S::BaseCard,
        attachment: Option<InstanceID>,
        state: <S::BaseCard as BaseCard>::CardState,
    ) -> Self {
        Self {
            id,
            base,
            attachment,
            state,
        }
    }
    pub fn id(&self) -> InstanceID {
        self.id
    }

    pub fn base(&self) -> &S::BaseCard {
        &self.base
    }

    pub fn attachment(&self) -> Option<InstanceID> {
        self.attachment
    }
}
