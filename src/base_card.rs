use crate::CardState;
use std::fmt::Debug;
use std::hash::Hash;

pub trait BaseCard:
    serde::Serialize + serde::de::DeserializeOwned + Clone + PartialEq + Eq + Hash + Debug
{
    type CardState: CardState;

    fn attachment(&self) -> Option<Self>;

    fn new_card_state(&self, parent: Option<&Self::CardState>) -> Self::CardState;

    fn reset_card(&self, card: &Self::CardState) -> Self::CardState;
}
