pub trait CardState: serde::Serialize + serde::de::DeserializeOwned + Clone {
    fn eq(&self, other: &Self) -> bool;
}
