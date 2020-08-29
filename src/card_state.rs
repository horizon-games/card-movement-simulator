use std::fmt::Debug;

pub trait CardState: serde::Serialize + serde::de::DeserializeOwned + Clone + Debug {
    fn eq(&self, other: &Self) -> bool;

    /// This is called to create a copy of this card.
    /// It should return a valid state for this card as if it was standalone - no attachments.
    fn copy_card(&self) -> Self {
        self.clone()
    }
}
