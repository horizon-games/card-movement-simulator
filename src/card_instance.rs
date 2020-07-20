use {
    crate::{BaseCard, InstanceID, State},
    std::ops::{Deref, DerefMut},
};

#[derive(Clone)]
pub struct CardInstance<S: State> {
    pub(crate) id: InstanceID,

    pub(crate) base: S::BaseCard,

    pub(crate) attachment: Option<InstanceID>,

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

impl<S: State> CardInstance<S> {
    pub fn id(&self) -> InstanceID {
        self.id
    }

    pub fn attachment(&self) -> Option<InstanceID> {
        self.attachment
    }
}
