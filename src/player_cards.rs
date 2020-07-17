use crate::{InstanceID, Zone};

#[derive(Clone, Default)]
pub struct PlayerCards {
    deck: usize,
    hand: Vec<Option<InstanceID>>,
    field: Vec<InstanceID>,
    graveyard: Vec<InstanceID>,
    dust: Vec<InstanceID>,
    limbo: Vec<InstanceID>,
    casting: Vec<InstanceID>,
    card_selection: usize,

    pointers: usize,
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
        todo!();
    }

    pub fn location(&self, id: InstanceID) -> Option<(Zone, usize)> {
        todo!();
    }
}
