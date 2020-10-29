use {
    crate::{
        card_state::CardState, error, Card, CardEvent, CardInfo, CardInfoMut, CardInstance,
        CardLocation, ExactCardLocation, GameState, InstanceID, OpaquePointer, Player, State, Zone,
    },
    rand::seq::SliceRandom,
    std::ops::{Deref, DerefMut},
};

#[cfg(feature = "bindings")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg_attr(
    feature = "bindings",
    derive(typescript_definitions::TypescriptDefinition)
)]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
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

    /// Used for deferred ModifyCard events when attachments are detached.
    /// Internal use only.
    #[serde(bound = "S: State")]
    pub(crate) deferred_logs: Vec<CardEvent<S>>,

    pub(crate) deferred_locations: Vec<(Zone, Option<usize>)>,

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

            deferred_logs: Default::default(),
            deferred_locations: Default::default(),

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
        self.location(card).location.map(|(zone, ..)| zone)
    }

    pub fn location(&self, card: impl Into<Card>) -> CardLocation {
        self.id(card)
            .and_then(|id| {
                self.deck
                    .iter()
                    .position(|deck_id| *deck_id == id)
                    .map(|i| CardLocation {
                        player: self.player,
                        location: Some((Zone::Deck, Some(i))),
                    })
                    .or_else(|| {
                        self.hand
                            .iter()
                            .position(|hand_id| *hand_id == Some(id))
                            .map(|i| CardLocation {
                                player: self.player,
                                location: Some((Zone::Hand { public: false }, Some(i))),
                            })
                    })
                    .or_else(|| {
                        self.dust
                            .iter()
                            .position(|dust_id| *dust_id == id)
                            .map(|i| CardLocation {
                                player: self.player,
                                location: Some((Zone::Dust { public: false }, Some(i))),
                            })
                    })
                    .or_else(|| {
                        self.limbo
                            .iter()
                            .position(|limbo_id| *limbo_id == id)
                            .map(|i| CardLocation {
                                player: self.player,
                                location: Some((Zone::Limbo { public: false }, Some(i))),
                            })
                    })
                    .or_else(|| {
                        self.card_selection
                            .iter()
                            .position(|card_selection_id| *card_selection_id == id)
                            .map(|i| CardLocation {
                                player: self.player,
                                location: Some((Zone::CardSelection, Some(i))),
                            })
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

                            CardLocation {
                                player: self.player,
                                location: Some((
                                    Zone::Attachment {
                                        parent: parent.into(),
                                    },
                                    None,
                                )),
                            }
                        })
                    })
            })
            .unwrap_or(CardLocation {
                player: self.player,
                location: None,
            })
    }

    pub fn reveal_from_card<T>(
        &self,
        card: impl Into<Card>,
        f: impl Fn(CardInfo<S>) -> T,
    ) -> Option<T> {
        let card = card.into();

        self.instance(card).map(|instance| {
            let zone = self
                .zone(card)
                .unwrap_or_else(|| panic!("player {} secret {:?} has no zone", self.player, card));

            let attachment = instance.attachment.map(|attachment| {
                self.instance(attachment).unwrap_or_else(|| {
                    panic!(
                        "player {} secret {:?} attachment {:?} not secret",
                        self.player, card, attachment
                    )
                })
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
        log: &mut dyn FnMut(<GameState<S> as arcadeum::store::State>::Event),
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

        let zone = self
            .zone(card)
            .unwrap_or_else(|| panic!("player {} secret {:?} has no zone", self.player, card));

        let attachment = instance.attachment().map(|attachment| {
            self.instance(attachment)
                .unwrap_or_else(|| {
                    panic!(
                        "player {} secret {:?} attachment {:?} not secret",
                        self.player, card, attachment
                    )
                })
                .clone()
        });

        self.modify_card_internal(card, log, move |instance, log| {
            f(CardInfoMut {
                instance,
                owner,
                zone,
                attachment: attachment.as_ref(),
                log,
            })
        });

        Ok(())
    }

    pub fn attach_card(
        &mut self,
        card: impl Into<Card>,
        attachment: impl Into<Card>,
        from_location: Option<(Zone, Option<usize>)>,
        log: &mut dyn FnMut(<GameState<S> as arcadeum::store::State>::Event),
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

        let parent_id = instance.id;

        if let Some(attachment) = instance.attachment {
            self.dust_card(attachment, log)?;
        }

        let attachment = match attachment {
            Card::ID(id) => id,
            Card::Pointer(OpaquePointer { index, .. }) => self.pointers[index],
        };

        let from = self.location(attachment);

        self.remove_id(log, attachment);

        let new_attach = self.instance(attachment).unwrap().clone();
        let player = self.player;

        self.modify_card_internal(card, log, |parent, log| {
            parent.attachment = Some(attachment);
            // Log the card moving to public zone.
            log(CardEvent::MoveCard {
                // we're moving an attach, so it can never have an attach.
                instance: Some((new_attach.clone(), None)),
                from: CardLocation {
                    player: from.player,
                    location: from.location.or(from_location),
                },
                to: ExactCardLocation {
                    player,
                    location: (
                        Zone::Attachment {
                            parent: parent_id.into(),
                        },
                        0,
                    ),
                },
            });
            S::on_attach(parent, &new_attach);
        });

        Ok(())
    }

    pub fn shuffle_deck(
        &mut self,
        random: &mut dyn rand::RngCore,
        log: &mut dyn FnMut(<GameState<S> as arcadeum::store::State>::Event),
    ) {
        self.deck.shuffle(random);

        log(CardEvent::ShuffleDeck {
            player: self.player,
            deck: self.deck.clone(),
        });
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

    pub(crate) fn dust_card(
        &mut self,
        card: impl Into<Card>,
        log: &mut dyn FnMut(<GameState<S> as arcadeum::store::State>::Event),
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

        let from = self.location(id);

        let instance = self.instance(id).unwrap();
        let attach = instance
            .attachment
            .map(|attach_id| self.instance(attach_id).unwrap().clone());

        // Emit dust event.
        log(CardEvent::MoveCard {
            instance: Some((instance.clone(), attach)),
            from,
            to: ExactCardLocation {
                player: self.player,
                location: (Zone::Dust { public: false }, self.dust.len()),
            },
        });

        // Finally, move the card from its current zone to Dust.
        // remove_id might call on_detach, so we have to run it after we emit the Dust event to get correct log orders.
        self.remove_id(log, id);
        self.dust.push(id);

        Ok(())
    }

    // Internal API only.
    // Modifies a card, but using a &mut CardInstance instead of a CardInfoMut
    pub(crate) fn modify_card_internal(
        &mut self,
        card: impl Into<Card>,
        log: &mut dyn FnMut(<GameState<S> as arcadeum::store::State>::Event),
        f: impl FnOnce(
            &mut CardInstance<S>,
            &mut dyn FnMut(<GameState<S> as arcadeum::store::State>::Event),
        ),
    ) {
        let card = card.into();

        let instance = self
            .instance_mut(card)
            .unwrap_or_else(|| panic!("{:?} vanished", card));

        let before = instance.clone();

        f(instance, log);

        let after = self
            .instance(card)
            .unwrap_or_else(|| panic!("{:?} vanished", card));

        if !before.eq(after) {
            log(CardEvent::ModifyCard {
                instance: after.clone(),
            })
        }
    }

    /// Remove an InstanceID from all zones in this secret.
    /// Internal API only.
    pub(crate) fn remove_id(
        &mut self,
        log: &mut dyn FnMut(<GameState<S> as arcadeum::store::State>::Event),
        id: InstanceID,
    ) {
        self.deck.retain(|deck_id| *deck_id != id);
        self.hand.retain(|hand_id| *hand_id != Some(id));
        self.dust.retain(|dust_id| *dust_id != id);
        self.limbo.retain(|limbo_id| *limbo_id != id);
        self.card_selection
            .retain(|card_selection_id| *card_selection_id != id);

        for parent_id in self.instances.keys().copied().collect::<Vec<_>>() {
            if let Some(attach_id) = self.instance(parent_id).unwrap().attachment {
                if attach_id == id {
                    let attach_clone = self.instance(attach_id).unwrap().clone();
                    self.modify_card_internal(parent_id, log, |parent, _| {
                        S::on_detach(parent, &attach_clone);
                        parent.attachment = None;
                    });
                }
            }
        }
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
