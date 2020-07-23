use crate::{InstanceID, Zone};

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct PlayerCards {
    pub(crate) deck: usize,
    pub(crate) hand: Vec<Option<InstanceID>>,
    pub(crate) field: Vec<InstanceID>,
    pub(crate) graveyard: Vec<InstanceID>,
    pub(crate) dust: Vec<InstanceID>,
    pub(crate) limbo: Vec<InstanceID>,
    pub(crate) casting: Vec<InstanceID>,
    pub(crate) card_selection: usize,

    pub(crate) pointers: usize,
}

impl PlayerCards {
    pub fn deck(&self) -> usize {
        self.deck
    }

    pub fn hand(&self) -> &Vec<Option<InstanceID>> {
        &self.hand
    }

    pub fn field(&self) -> &Vec<InstanceID> {
        &self.field
    }

    pub fn graveyard(&self) -> &Vec<InstanceID> {
        &self.graveyard
    }

    pub fn dust(&self) -> &Vec<InstanceID> {
        &self.dust
    }

    pub fn limbo(&self) -> &Vec<InstanceID> {
        &self.limbo
    }

    pub fn casting(&self) -> &Vec<InstanceID> {
        &self.casting
    }

    pub fn card_selection(&self) -> usize {
        self.card_selection
    }

    pub fn zone(&self, id: InstanceID) -> Option<Zone> {
        self.location(id).map(|(zone, ..)| zone)
    }

    /// The location of the ID if it exists in one of the player's public non-attachment zones
    pub fn location(&self, id: InstanceID) -> Option<(Zone, usize)> {
        self.hand
            .iter()
            .position(|hand_id| *hand_id == Some(id))
            .map(|i| (Zone::Hand { public: true }, i))
            .or_else(|| {
                self.field
                    .iter()
                    .position(|field_id| *field_id == id)
                    .map(|i| (Zone::Field, i))
            })
            .or_else(|| {
                self.graveyard
                    .iter()
                    .position(|graveyard_id| *graveyard_id == id)
                    .map(|i| (Zone::Graveyard, i))
            })
            .or_else(|| {
                self.dust
                    .iter()
                    .position(|dust_id| *dust_id == id)
                    .map(|i| (Zone::Dust { public: true }, i))
            })
            .or_else(|| {
                self.limbo
                    .iter()
                    .position(|limbo_id| *limbo_id == id)
                    .map(|i| (Zone::Limbo { public: true }, i))
            })
            .or_else(|| {
                self.casting
                    .iter()
                    .position(|casting_id| *casting_id == id)
                    .map(|i| (Zone::Casting, i))
            })
    }

    pub(crate) fn remove_from(&mut self, zone: Zone, index: usize) {
        match zone {
            Zone::Deck => self.deck -= 1,
            Zone::Hand { .. } => { self.hand.remove(index); }
            Zone::Field => { self.field.remove(index); }
            Zone::Graveyard => { self.graveyard.remove(index); }
            Zone::Dust { public: true } => { self.dust.remove(index); }
            Zone::Attachment { .. } => todo!(),
            Zone::Limbo { public: true } => { self.limbo.remove(index); }
            Zone::Casting => { self.casting.remove(index); }
            Zone::CardSelection => self.card_selection -= 1,
            _ => (),
        }
    }
}
