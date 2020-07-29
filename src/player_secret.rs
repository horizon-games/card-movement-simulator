use {
    crate::{
        error, BaseCard, Card, CardInfo, CardInfoMut, CardInstance, Event, InstanceID,
        OpaquePointer, Player, State, Zone,
    },
    std::ops::{Deref, DerefMut},
};

#[cfg(feature = "bindings")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg_attr(
    feature = "bindings",
    derive(typescript_definitions::TypescriptDefinition)
)]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PlayerSecret<S: State> {
    #[serde(bound = "S: State")]
    pub secret: S::Secret,

    #[serde(bound = "S: State")]
    pub(crate) instances: indexmap::IndexMap<InstanceID, CardInstance<S>>,
    pub(crate) next_instance: Option<InstanceID>,
    pub(crate) pointers: Vec<InstanceID>,

    pub(crate) deck: Vec<InstanceID>,
    pub(crate) hand: Vec<Option<InstanceID>>,
    pub(crate) dust: Vec<InstanceID>,
    pub(crate) limbo: Vec<InstanceID>,
    pub(crate) card_selection: Vec<InstanceID>,

    player: Player,
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
    pub fn new(player: Player, secret: S::Secret) -> Self {
        Self {
            secret,

            instances: Default::default(),
            next_instance: Default::default(),
            pointers: Default::default(),

            deck: Default::default(),
            hand: Default::default(),
            dust: Default::default(),
            limbo: Default::default(),
            card_selection: Default::default(),

            player,
        }
    }

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
        let card = card.into();
        let attachment = attachment.into();

        if let Card::Pointer(OpaquePointer { player, .. }) = attachment {
            if player != self.player {
                return Err(error::SecretMoveCardError::MissingPointer {
                    card: attachment,
                    player: self.player,
                });
            }
        }

        let instance = self
            .instance(card)
            .ok_or(error::SecretMoveCardError::MissingInstance {
                card,
                player: self.player,
            })?;

        if let Some(attachment) = instance.attachment {
            self.dust_card(attachment)?;
        }

        let attachment = match attachment {
            Card::ID(id) => id,
            Card::Pointer(OpaquePointer { index, .. }) => self.pointers[index],
        };

        self.remove_id(attachment);

        let instance = self
            .instance_mut(card)
            .expect(&format!("{:?} vanished", card));

        instance.attachment = Some(attachment);

        Ok(())
    }

    pub(crate) fn dust_card(
        &mut self,
        card: impl Into<Card>,
    ) -> Result<(), error::SecretMoveCardError> {
        let card = card.into();

        let id = match card {
            Card::ID(id) => id,
            Card::Pointer(OpaquePointer { player, index }) => {
                if player == self.player {
                    self.pointers[index]
                } else {
                    return Err(error::SecretMoveCardError::MissingPointer { card, player });
                }
            }
        };

        self.remove_id(id);

        self.dust.push(id);

        Ok(())
    }

    /// Remove an InstanceID from all zones in this secret.
    /// Internal API only.
    pub(crate) fn remove_id(&mut self, id: InstanceID) {
        self.deck.retain(|deck_id| *deck_id != id);
        self.hand.retain(|hand_id| *hand_id != Some(id));
        self.dust.retain(|dust_id| *dust_id != id);
        self.limbo.retain(|limbo_id| *limbo_id != id);
        self.card_selection
            .retain(|card_selection_id| *card_selection_id != id);

        self.instances.values_mut().for_each(|instance| {
            if instance.attachment == Some(id) {
                instance.attachment = None;
            }
        });
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
