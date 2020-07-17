pub trait CardState: Clone {
    fn eq(&self, other: &Self) -> bool;
}
