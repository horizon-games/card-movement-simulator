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

    pub(crate) instances: indexmap::IndexMap<InstanceID, CardInstance<S>>,
    pub(crate) pointers: Vec<InstanceID>,

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
        let card = card.into();

        match card {
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
        let card = card.into();

        match card {
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
        self.location(card).map(|(zone, ..)| zone)
    }

    pub fn location(&self, card: impl Into<Card>) -> Option<(Zone, usize)> {
        let card = card.into();

        match card {
            Card::ID(id) => self
                .deck
                .iter()
                .position(|deck_id| *deck_id == id)
                .map(|i| (Zone::Deck, i))
                .or_else(|| {
                    self.hand
                        .iter()
                        .position(|hand_id| *hand_id == Some(id))
                        .map(|i| (Zone::Hand { public: false }, i))
                })
                .or_else(|| {
                    self.dust
                        .iter()
                        .position(|dust_id| *dust_id == id)
                        .map(|i| (Zone::Dust { public: false }, i))
                })
                .or_else(|| {
                    self.limbo
                        .iter()
                        .position(|limbo_id| *limbo_id == id)
                        .map(|i| (Zone::Limbo { public: false }, i))
                })
                .or_else(|| {
                    self.card_selection
                        .iter()
                        .position(|card_selection_id| *card_selection_id == id)
                        .map(|i| (Zone::CardSelection, i))
                })
                .or_else(|| {
                    let mut parents = self.instances.values().filter_map(|instance| {
                        if instance.attachment == Some(id) {
                            Some(instance.id())
                        } else {
                            None
                        }
                    });

                    parents.next().map(|parent| {
                        assert!(parents.next().is_none());

                        (
                            Zone::Attachment {
                                parent: parent.into(),
                            },
                            0,
                        )
                    })
                }),
            Card::Pointer(OpaquePointer { player, index }) => {
                if player == self.player {
                    let id = self.pointers[index];

                    self.deck
                        .iter()
                        .position(|deck_id| *deck_id == id)
                        .map(|i| (Zone::Deck, i))
                        .or_else(|| {
                            self.hand
                                .iter()
                                .position(|hand_id| *hand_id == Some(id))
                                .map(|i| (Zone::Hand { public: false }, i))
                        })
                        .or_else(|| {
                            self.dust
                                .iter()
                                .position(|dust_id| *dust_id == id)
                                .map(|i| (Zone::Dust { public: false }, i))
                        })
                        .or_else(|| {
                            self.limbo
                                .iter()
                                .position(|limbo_id| *limbo_id == id)
                                .map(|i| (Zone::Limbo { public: false }, i))
                        })
                        .or_else(|| {
                            self.card_selection
                                .iter()
                                .position(|card_selection_id| *card_selection_id == id)
                                .map(|i| (Zone::CardSelection, i))
                        })
                        .or_else(|| {
                            let mut parents = self.instances.values().filter_map(|instance| {
                                if instance.attachment == Some(id) {
                                    Some(instance.id())
                                } else {
                                    None
                                }
                            });

                            parents.next().map(|parent| {
                                assert!(parents.next().is_none());

                                (
                                    Zone::Attachment {
                                        parent: parent.into(),
                                    },
                                    0,
                                )
                            })
                        })
                } else {
                    None
                }
            }
        }
    }

    pub fn new_card(&mut self, base: S::BaseCard) -> InstanceID {
        todo!();
    }

    pub fn new_pointer(&mut self, id: InstanceID) {
        todo!();
    }

    pub(crate) fn append_deck_to_pointers(&mut self) {
        self.pointers.extend(&self.deck);
    }

    pub(crate) fn append_dust_to_pointers(&mut self) {
        self.pointers.extend(&self.dust);
    }

    pub(crate) fn append_limbo_to_pointers(&mut self) {
        self.pointers.extend(&self.limbo);
    }

    pub(crate) fn append_card_selection_to_pointers(&mut self) {
        self.pointers.extend(&self.card_selection);
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
