use {
    crate::{
        error, BaseCard, Card, CardInfoMut, CardInstance, Event, InstanceID, OpaquePointer, Player,
        State, Zone,
    },
    std::ops::{Deref, DerefMut},
};

#[derive(Clone)]
pub struct PlayerSecret<S: State> {
    pub secret: S::Secret,

    pub(crate) instances: indexmap::IndexMap<InstanceID, CardInstance<S>>,
    pub(crate) pointers: Vec<InstanceID>,

    pub(crate) mode: Option<Mode>,
    pub(crate) next_id: Option<InstanceID>,

    player: Player,

    deck: Vec<InstanceID>,
    hand: Vec<Option<InstanceID>>,
    dust: Vec<InstanceID>,
    limbo: Vec<InstanceID>,
    card_selection: Vec<InstanceID>,
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
        self.id(card).and_then(|id| self.instances.get(&id))
    }

    pub fn zone(&self, card: impl Into<Card>) -> Option<Zone> {
        self.location(card).map(|(zone, ..)| zone)
    }

    pub fn location(&self, card: impl Into<Card>) -> Option<(Zone, usize)> {
        self.id(card).and_then(|id| {
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
        })
    }

    pub fn modify_card(
        &mut self,
        card: impl Into<Card>,
        random: &mut dyn rand::RngCore,
        log: &mut dyn FnMut(&dyn Event),
        f: impl FnOnce(CardInfoMut<S>),
    ) -> Result<(), error::ModifyCardError> {
        let card = card.into();

        let id = self
            .id(card)
            .ok_or(error::ModifyCardError::MissingPointer { card })?;

        let instance = self
            .instances
            .get(&id)
            .ok_or(error::ModifyCardError::MissingInstance { card, id })?;

        let zone = self.zone(card).expect(&format!(
            "{:?} in player {} secret has no zone",
            card, self.player
        ));

        let attachment = instance
            .attachment()
            .and_then(|attachment| self.instances.get(&attachment).cloned());

        let instance = self
            .instances
            .get_mut(&id)
            .expect(&format!("{:?} vanished", id));

        f(CardInfoMut {
            instance,
            owner: self.player,
            zone,
            attachment: attachment.as_ref(),
            random,
            log,
        });

        Ok(())
    }

    pub fn new_card(&mut self, base: S::BaseCard) -> InstanceID {
        assert_eq!(self.mode, Some(Mode::NewCards));

        let next_id = self.next_id.expect("missing next ID for new secret cards");

        let attachment = base.attachment().map(|attachment| {
            let id = next_id;
            let state = attachment.new_card_state();
            let instance = CardInstance {
                id,
                base: attachment,
                attachment: None,
                state,
            };

            self.instances.insert(id, instance);

            id
        });

        let next_id = InstanceID(next_id.0 + 1);

        let id = next_id;
        let state = base.new_card_state();
        let instance = CardInstance {
            id,
            base,
            attachment,
            state,
        };

        self.instances.insert(id, instance);

        self.next_id = Some(InstanceID(next_id.0 + 1));

        self.pointers.push(id);

        id
    }

    pub fn new_pointer(&mut self, id: InstanceID) {
        assert_eq!(self.mode, Some(Mode::NewPointers));

        self.pointers.push(id);
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

    fn id(&self, card: impl Into<Card>) -> Option<InstanceID> {
        let card = card.into();

        match card {
            Card::ID(id) => Some(id),
            Card::Pointer(OpaquePointer { player, index }) => {
                if player == self.player {
                    Some(self.pointers[index])
                } else {
                    None
                }
            }
        }
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
            secret: Default::default(),

            instances: Default::default(),
            pointers: Default::default(),

            mode: Default::default(),
            next_id: Default::default(),

            player: Default::default(),

            deck: Default::default(),
            hand: Default::default(),
            dust: Default::default(),
            limbo: Default::default(),
            card_selection: Default::default(),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub(crate) enum Mode {
    NewCards,
    NewPointers,
}
