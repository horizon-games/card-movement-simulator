use crate::{Card, Player, Zone};

#[derive(thiserror::Error, Debug)]
pub enum MoveCardError {
    #[error("cannot move dusted {card:?}")]
    DustedCard { card: Card },
}

#[derive(thiserror::Error, Debug)]
pub enum SecretMoveCardError {
    #[error("cannot find {card:?} in player {player:?}'s secret")]
    MissingPointer { card: Card, player: Player },
    #[error("cannot find {card:?} in player {player:?}'s secret")]
    MissingInstance { card: Card, player: Player },
    #[error("cannot move dusted {card:?}")]
    DustedCard { card: Card },
}

#[derive(thiserror::Error, Debug)]
pub enum SecretModifyCardError {
    #[error("cannot find {card:?} in player {player:?}'s secret")]
    MissingInstance { card: Card, player: Player },
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
pub enum RevealOkError {
    #[error("err:?")]
    Error { err: String },
}
