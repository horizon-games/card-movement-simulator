use crate::{error, Card};

#[cfg(feature = "bindings")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg_attr(
    feature = "bindings",
    derive(typescript_definitions::TypescriptDefinition)
)]
#[derive(serde::Serialize, serde::Deserialize, Copy, Clone, Debug)]
#[serde(tag = "name")]
pub enum Zone {
    Deck,
    Hand { public: bool },
    Field,
    Graveyard,
    Dust { public: bool },
    Attachment { parent: Card },
    Limbo { public: bool },
    Casting,
    CardSelection,
}

impl Zone {
    pub fn is_deck(&self) -> bool {
        matches!(self, Self::Deck)
    }

    pub fn is_hand(&self) -> bool {
        matches!(self, Self::Hand { .. })
    }

    pub fn is_public_hand(&self) -> bool {
        matches!(self, Self::Hand { public: true })
    }

    pub fn is_secret_hand(&self) -> bool {
        matches!(self, Self::Hand { public: false })
    }

    pub fn is_field(&self) -> bool {
        matches!(self, Self::Field)
    }

    pub fn is_graveyard(&self) -> bool {
        matches!(self, Self::Graveyard)
    }

    pub fn is_dust(&self) -> bool {
        matches!(self, Self::Dust { .. })
    }

    pub fn is_public_dust(&self) -> bool {
        matches!(self, Self::Dust { public: true })
    }

    pub fn is_secret_dust(&self) -> bool {
        matches!(self, Self::Dust { public: false })
    }

    pub fn is_attachment(&self) -> bool {
        matches!(self, Self::Attachment { .. })
    }

    pub fn is_limbo(&self) -> bool {
        matches!(self, Self::Limbo { .. })
    }

    pub fn is_public_limbo(&self) -> bool {
        matches!(self, Self::Limbo { public: true })
    }

    pub fn is_secret_limbo(&self) -> bool {
        matches!(self, Self::Limbo { public: false })
    }

    pub fn is_casting(&self) -> bool {
        matches!(self, Self::Casting)
    }

    pub fn is_card_selection(&self) -> bool {
        matches!(self, Self::CardSelection)
    }

    pub fn eq(&self, other: Zone) -> Result<bool, error::ZoneEqualityError> {
        match self {
            Self::Deck => Ok(other.is_deck()),
            Self::Hand { public: true } => Ok(other.is_public_hand()),
            Self::Hand { public: false } => Ok(other.is_secret_hand()),
            Self::Field => Ok(other.is_field()),
            Self::Graveyard => Ok(other.is_graveyard()),
            Self::Dust { public: true } => Ok(other.is_public_dust()),
            Self::Dust { public: false } => Ok(other.is_secret_dust()),
            Self::Attachment { parent } => {
                match other {
                    Self::Attachment {
                        parent: other_parent,
                    } => other_parent.eq(parent).or(Err(
                        error::ZoneEqualityError::IncomparableZones { a: *self, b: other },
                    )),
                    _ => Ok(false),
                }
            }
            Self::Limbo { public: true } => Ok(other.is_public_limbo()),
            Self::Limbo { public: false } => Ok(other.is_secret_limbo()),
            Self::Casting => Ok(other.is_casting()),
            Self::CardSelection => Ok(other.is_card_selection()),
        }
    }

    pub fn ne(&self, other: Zone) -> Result<bool, error::ZoneEqualityError> {
        Ok(!self.eq(other)?)
    }
}
