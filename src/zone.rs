use crate::Card;

#[derive(serde::Serialize, serde::Deserialize, Copy, Clone, Debug)]
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

    pub fn eq(&self, other: Zone) -> Result<bool, String> {
        todo!();
    }

    pub fn ne(&self, other: Zone) -> Result<bool, String> {
        Ok(!self.eq(other)?)
    }
}
