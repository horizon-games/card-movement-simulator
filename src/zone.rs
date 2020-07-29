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
        if let Self::Deck = self {
            true
        } else {
            false
        }
    }

    pub fn is_hand(&self) -> bool {
        if let Self::Hand { .. } = self {
            true
        } else {
            false
        }
    }

    pub fn is_public_hand(&self) -> bool {
        if let Self::Hand { public: true } = self {
            true
        } else {
            false
        }
    }

    pub fn is_secret_hand(&self) -> bool {
        if let Self::Hand { public: false } = self {
            true
        } else {
            false
        }
    }

    pub fn is_field(&self) -> bool {
        if let Self::Field = self {
            true
        } else {
            false
        }
    }

    pub fn is_graveyard(&self) -> bool {
        if let Self::Graveyard = self {
            true
        } else {
            false
        }
    }

    pub fn is_dust(&self) -> bool {
        if let Self::Dust { .. } = self {
            true
        } else {
            false
        }
    }

    pub fn is_public_dust(&self) -> bool {
        if let Self::Dust { public: true } = self {
            true
        } else {
            false
        }
    }

    pub fn is_secret_dust(&self) -> bool {
        if let Self::Dust { public: false } = self {
            true
        } else {
            false
        }
    }

    pub fn is_attachment(&self) -> bool {
        if let Self::Attachment { .. } = self {
            true
        } else {
            false
        }
    }

    pub fn is_limbo(&self) -> bool {
        if let Self::Limbo { .. } = self {
            true
        } else {
            false
        }
    }

    pub fn is_public_limbo(&self) -> bool {
        if let Self::Limbo { public: true } = self {
            true
        } else {
            false
        }
    }

    pub fn is_secret_limbo(&self) -> bool {
        if let Self::Limbo { public: false } = self {
            true
        } else {
            false
        }
    }

    pub fn is_casting(&self) -> bool {
        if let Self::Casting = self {
            true
        } else {
            false
        }
    }

    pub fn is_card_selection(&self) -> bool {
        if let Self::CardSelection = self {
            true
        } else {
            false
        }
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
