use {
    crate::{
        error, BaseCard, Card, CardInfo, CardInfoMut, CardInstance, Event, InstanceID,
        OpaquePointer, Player, State, Zone,
    },
    std::ops::{Deref, DerefMut},
};

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PlayerSecret<S: State> {
    #[serde(bound = "S: State")]
    pub secret: S::Secret,

    #[serde(bound = "S: State")]
    pub(crate) instances: indexmap::IndexMap<InstanceID, CardInstance<S>>,
    pub(crate) pointers: Vec<InstanceID>,

    pub(crate) mode: Option<Mode>,

    player: Player,

    pub(crate) deck: Vec<InstanceID>,
    pub(crate) hand: Vec<Option<InstanceID>>,
    pub(crate) dust: Vec<InstanceID>,
    pub(crate) limbo: Vec<InstanceID>,
    pub(crate) card_selection: Vec<InstanceID>,
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

    pub fn reveal_from_card<T>(
        &self,
        card: impl Into<Card>,
        f: impl Fn(CardInfo<S>) -> T,
    ) -> Option<T> {
        let card = card.into();

        self.instance(card).map(|instance| {
            let zone = self.zone(card).expect(&format!(
                "player {} secret {:?} has no zone",
                self.player, card
            ));

            let attachment = instance.attachment.map(|attachment| {
                self.instance(attachment).expect(&format!(
                    "player {} secret {:?} attachment {:?} not secret",
                    self.player, card, attachment
                ))
            });

            f(CardInfo {
                instance,
                owner: self.player,
                zone,
                attachment,
            })
        })
    }

    pub fn modify_card(
        &mut self,
        card: impl Into<Card>,
        random: &mut dyn rand::RngCore,
        log: &mut dyn FnMut(&dyn Event),
        f: impl FnOnce(CardInfoMut<S>),
    ) -> Result<(), error::SecretModifyCardError> {
        let card = card.into();

        let instance =
            self.instance(card)
                .ok_or(error::SecretModifyCardError::MissingInstance {
                    card,
                    player: self.player,
                })?;

        let owner = self.player;

        let zone = self.zone(card).expect(&format!(
            "player {} secret {:?} has no zone",
            self.player, card
        ));

        let attachment = instance.attachment().map(|attachment| {
            self.instance(attachment)
                .expect(&format!(
                    "player {} secret {:?} attachment {:?} not secret",
                    self.player, card, attachment
                ))
                .clone()
        });

        let instance = self
            .instance_mut(card)
            .expect(&format!("{:?} vanished", card));

        f(CardInfoMut {
            instance,
            owner,
            zone,
            attachment: attachment.as_ref(),
            random,
            log,
        });

        Ok(())
    }

    pub fn new_card(&mut self, base: S::BaseCard) -> InstanceID {
        if let Some(Mode::NewCards(mut id)) = self.mode {
            let attachment = base.attachment().map(|attachment| {
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

            id.0 += 1;

            let card = id;
            let state = base.new_card_state();
            let instance = CardInstance {
                id,
                base,
                attachment,
                state,
            };

            self.instances.insert(id, instance);

            id.0 += 1;

            self.pointers.push(card);

            card
        } else {
            panic!("called PlayerSecret::new_card outside of CardGame::new_secret_cards");
        }
    }

    pub fn new_pointer(&mut self, id: InstanceID) {
        if let Some(Mode::NewPointers) = self.mode {
            self.pointers.push(id);
        } else {
            panic!("called PlayerSecret::new_pointer outside of CardGame::new_secret_pointers");
        }
    }

    pub(crate) fn instance_mut(&mut self, card: impl Into<Card>) -> Option<&mut CardInstance<S>> {
        self.id(card)
            .and_then(move |id| self.instances.get_mut(&id))
    }

    pub(crate) fn append_deck_to_pointers(&mut self) {
        self.pointers.extend(&self.deck);
    }

    pub(crate) fn append_secret_hand_to_pointers(&mut self) {
        self.pointers.extend(self.hand.iter().flatten());
    }

    pub(crate) fn append_card_selection_to_pointers(&mut self) {
        self.pointers.extend(&self.card_selection);
    }

    pub(crate) fn attach_card(
        &mut self,
        card: impl Into<Card>,
        attachment: impl Into<Card>,
    ) -> Result<(), error::SecretMoveCardError> {
        todo!()
    }

    pub(crate) fn dust_card(
        &mut self,
        card: impl Into<Card>,
    ) -> Result<(), error::SecretMoveCardError> {
        todo!()
    }

    /// Remove an InstanceID from all zones in this secret.
    /// Internal API only.
    pub(crate) fn remove_id(&mut self, id: InstanceID) {
        todo!();
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

            player: Default::default(),

            deck: Default::default(),
            hand: Default::default(),
            dust: Default::default(),
            limbo: Default::default(),
            card_selection: Default::default(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub(crate) enum Mode {
    NewCards(InstanceID),
    NewPointers,
}
