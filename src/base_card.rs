use crate::CardState;

pub trait BaseCard: Clone + PartialEq {
    type CardState: CardState;

    fn attachment(&self) -> Option<Self>;

    fn new_card_state(&self) -> Self::CardState;
}
