use crate::{Card, InstanceID, Zone};

#[derive(thiserror::Error, Debug)]
pub enum MoveCardError {
    #[error("cannot move dusted {card:?}")]
    DustedCard { card: Card },
}

#[derive(thiserror::Error, Debug)]
pub enum ModifyCardError {
    #[error("cannot find {card:?}")]
    MissingPointer { card: Card },

    #[error("cannot find {card:?} {id:?}")]
    MissingInstance { card: Card, id: InstanceID },
}

#[derive(thiserror::Error, Debug)]
pub enum CardEqualityError {
    #[error("cannot determine if {a:?} and {b:?} are equal")]
    IncomparableCards { a: Card, b: Card },
}

#[derive(thiserror::Error, Debug)]
pub enum ZoneEqualityError {
    #[error("cannot determine if {a:?} and {b:?} are equal")]
    IncomparableZones { a: Zone, b: Zone },
}

#[doc(hidden)]
#[derive(thiserror::Error, Eq, PartialEq, Debug)]
pub enum RevealOkError {}
