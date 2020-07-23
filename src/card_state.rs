use std::fmt::Debug;

pub trait CardState: serde::Serialize + serde::de::DeserializeOwned + Clone + Debug {
    fn eq(&self, other: &Self) -> bool;
}
