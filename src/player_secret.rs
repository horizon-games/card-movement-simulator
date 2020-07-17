use {
    crate::{Card, CardInstance, InstanceID, OpaquePointer, Player, State, Zone},
    std::ops::{Deref, DerefMut},
};

#[derive(Clone)]
pub struct PlayerSecret<S: State> {
    player: Player,

    deck: Vec<InstanceID>,
    hand: Vec<Option<InstanceID>>,
    dust: Vec<InstanceID>,
    limbo: Vec<InstanceID>,
    card_selection: Vec<InstanceID>,

    instances: indexmap::IndexMap<InstanceID, CardInstance<S>>,
    pointers: Vec<InstanceID>,

    secret: S::Secret,
}

impl<S: State> Deref for PlayerSecret<S> {
    type Target = S::Secret;

    fn deref(&self) -> &Self::Target {
        &self.secret
    }
}

impl<S: State> DerefMut for PlayerSecret<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.secret
    }
}

impl<S: State> PlayerSecret<S> {
    pub fn player(&self) -> Player {
        self.player
    }

    pub fn deck(&self) -> &Vec<InstanceID> {
        &self.deck
    }

    pub fn hand(&self) -> &Vec<Option<InstanceID>> {
        &self.hand
    }

    pub fn dust(&self) -> &Vec<InstanceID> {
        &self.dust
    }

    pub fn limbo(&self) -> &Vec<InstanceID> {
        &self.limbo
    }

    pub fn card_selection(&self) -> &Vec<InstanceID> {
        &self.card_selection
    }

    pub fn instance(&self, card: impl Into<Card>) -> Option<&CardInstance<S>> {
        match card.into() {
            Card::ID(id) => self.instances.get(&id),
            Card::Pointer(OpaquePointer { player, index }) => {
                if player == self.player {
                    self.instances.get(&self.pointers[index])
                } else {
                    None
                }
            }
        }
    }

    pub fn instance_mut(&mut self, card: impl Into<Card>) -> Option<&mut CardInstance<S>> {
        match card.into() {
            Card::ID(id) => self.instances.get_mut(&id),
            Card::Pointer(OpaquePointer { player, index }) => {
                if player == self.player {
                    self.instances.get_mut(&self.pointers[index])
                } else {
                    None
                }
            }
        }
    }

    pub fn zone(&self, card: impl Into<Card>) -> Option<Zone> {
        todo!();
    }

    pub fn location(&self, card: impl Into<Card>) -> Option<(Zone, usize)> {
        todo!();
    }

    pub fn new_card(&mut self, base: S::BaseCard) -> InstanceID {
        todo!();
    }

    pub fn new_pointer(&mut self, id: InstanceID) {
        todo!();
    }
}

impl<S: State> arcadeum::store::Secret for PlayerSecret<S> {
    fn deserialize(data: &[u8]) -> Result<Self, String> {
        todo!();
    }

    fn serialize(&self) -> Vec<u8> {
        todo!();
    }
}

impl<S: State> Default for PlayerSecret<S>
where
    S::Secret: Default,
{
    fn default() -> Self {
        Self {
            player: Default::default(),

            deck: Default::default(),
            hand: Default::default(),
            dust: Default::default(),
            limbo: Default::default(),
            card_selection: Default::default(),

            instances: Default::default(),
            pointers: Default::default(),

            secret: Default::default(),
        }
    }
}
