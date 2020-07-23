use {crate::CardState, std::fmt::Debug};

pub trait BaseCard:
    serde::Serialize + serde::de::DeserializeOwned + Clone + PartialEq + Debug
{
    type CardState: CardState;

    fn attachment(&self) -> Option<Self>;

    fn new_card_state(&self) -> Self::CardState;
}
