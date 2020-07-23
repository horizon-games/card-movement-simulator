use {
    crate::{
        error, player_secret, BaseCard, Card, CardInstance, Context, Event, GameState, InstanceID,
        InstanceOrPlayer, OpaquePointer, Player, PlayerSecret, Secret, State, Zone,
    },
    rand::seq::IteratorRandom,
    std::{
        iter::repeat,
        ops::{Deref, DerefMut},
    },
};

pub struct CardGame<S: State> {
    pub(crate) state: GameState<S>,

    pub context: Context<S>,
}

impl<S: State> Deref for CardGame<S> {
    type Target = GameState<S>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<S: State> DerefMut for CardGame<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl<S: State> CardGame<S> {
    pub fn new(state: GameState<S>, context: Context<S>) -> Self {
        Self { state, context }
    }

    pub fn new_card(&mut self, player: Player, base: S::BaseCard) -> InstanceID {
        let attachment = base.attachment().map(|attachment| {
            let id = InstanceID(self.instances.len());
            let state = attachment.new_card_state();
            let instance = CardInstance {
                id,
                base: attachment,
                attachment: None,
                state,
            };

            self.instances.push(InstanceOrPlayer::from(instance));

            id
        });

        let id = InstanceID(self.instances.len());
        let state = base.new_card_state();
        let instance = CardInstance {
            id,
            base,
            attachment,
            state,
        };

        self.instances.push(InstanceOrPlayer::from(instance));

        self.player_cards_mut(player).limbo.push(id);

        id
    }

    pub fn deck_card(&mut self, player: Player, index: usize) -> Card {
        self.context.mutate_secret(player, |secret, _, _| {
            secret.pointers.push(secret.deck()[index]);
        });

        let player_cards = self.player_cards_mut(player);

        player_cards.pointers += 1;

        OpaquePointer {
            player,
            index: player_cards.pointers - 1,
        }
        .into()
    }

    pub fn hand_card(&mut self, player: Player, index: usize) -> Card {
        match self.player_cards(player).hand()[index] {
            Some(id) => id.into(),
            None => {
                self.context.mutate_secret(player, |secret, _, _| {
                    secret.pointers.push(secret.hand()[index].expect(&format!(
                        "player {} hand {} is neither public nor secret",
                        player, index
                    )));
                });

                let player_cards = self.player_cards_mut(player);

                player_cards.pointers += 1;

                OpaquePointer {
                    player,
                    index: player_cards.pointers - 1,
                }
                .into()
            }
        }
    }

    pub fn field_card(&self, player: Player, index: usize) -> InstanceID {
        self.player_cards(player).field()[index]
    }

    pub fn graveyard_card(&self, player: Player, index: usize) -> InstanceID {
        self.player_cards(player).graveyard()[index]
    }

    pub fn public_dust_card(&self, player: Player, index: usize) -> InstanceID {
        self.player_cards(player).dust()[index]
    }

    pub fn secret_dust_card(&mut self, player: Player, index: usize) -> Card {
        self.context.mutate_secret(player, |secret, _, _| {
            secret.pointers.push(secret.dust()[index]);
        });

        let player_cards = self.player_cards_mut(player);

        player_cards.pointers += 1;

        OpaquePointer {
            player,
            index: player_cards.pointers - 1,
        }
        .into()
    }

    pub fn public_limbo_card(&self, player: Player, index: usize) -> InstanceID {
        self.player_cards(player).limbo()[index]
    }

    pub fn secret_limbo_card(&mut self, player: Player, index: usize) -> Card {
        self.context.mutate_secret(player, |secret, _, _| {
            secret.pointers.push(secret.limbo()[index]);
        });

        let player_cards = self.player_cards_mut(player);

        player_cards.pointers += 1;

        OpaquePointer {
            player,
            index: player_cards.pointers - 1,
        }
        .into()
    }

    pub fn casting_card(&self, player: Player, index: usize) -> InstanceID {
        self.player_cards(player).casting()[index]
    }

    pub fn card_selection_card(&mut self, player: Player, index: usize) -> Card {
        self.context.mutate_secret(player, |secret, _, _| {
            secret.pointers.push(secret.card_selection()[index]);
        });

        let player_cards = self.player_cards_mut(player);

        player_cards.pointers += 1;

        OpaquePointer {
            player,
            index: player_cards.pointers - 1,
        }
        .into()
    }

    pub fn deck_cards(&mut self, player: Player) -> Vec<Card> {
        self.context.mutate_secret(player, |secret, _, _| {
            secret.append_deck_to_pointers();
        });

        let player_cards = self.player_cards_mut(player);

        player_cards.pointers += player_cards.deck();

        (player_cards.pointers - player_cards.deck()..player_cards.pointers)
            .map(|index| OpaquePointer { player, index }.into())
            .collect()
    }

    pub fn hand_cards(&mut self, player: Player) -> Vec<Card> {
        self.context.mutate_secret(player, |secret, _, _| {
            secret.append_secret_hand_to_pointers();
        });

        let player_cards = self.player_cards_mut(player);

        let secret_hand = player_cards.hand().iter().filter(|id| id.is_none()).count();

        player_cards.pointers += secret_hand;

        let mut secret_hand = (player_cards.pointers - secret_hand..player_cards.pointers)
            .map(|index| OpaquePointer { player, index });

        let hand = player_cards
            .hand()
            .iter()
            .map(|id| {
                id.map(Card::from).unwrap_or_else(|| {
                    secret_hand
                        .next()
                        .expect("not enough secret hand cards")
                        .into()
                })
            })
            .collect();

        assert!(secret_hand.next().is_none());

        hand
    }

    pub fn field_cards(&self, player: Player) -> &Vec<InstanceID> {
        self.player_cards(player).field()
    }

    pub fn graveyard_cards(&self, player: Player) -> &Vec<InstanceID> {
        self.player_cards(player).graveyard()
    }

    pub fn public_dust_cards(&self, player: Player) -> &Vec<InstanceID> {
        self.player_cards(player).dust()
    }

    /// This reveals the number of cards in a player's secret dust.
    pub async fn secret_dust_cards(&mut self, player: Player) -> Vec<Card> {
        self.new_secret_pointers(player, |mut secret| {
            secret
                .dust()
                .clone()
                .into_iter()
                .for_each(|id| secret.new_pointer(id));
        })
        .await
    }

    pub fn public_limbo_cards(&self, player: Player) -> &Vec<InstanceID> {
        self.player_cards(player).limbo()
    }

    /// This reveals the number of cards in a player's secret limbo.
    pub async fn secret_limbo_cards(&mut self, player: Player) -> Vec<Card> {
        self.new_secret_pointers(player, |mut secret| {
            secret
                .limbo()
                .clone()
                .into_iter()
                .for_each(|id| secret.new_pointer(id));
        })
        .await
    }

    pub fn casting_cards(&self, player: Player) -> &Vec<InstanceID> {
        self.player_cards(player).casting()
    }

    pub fn card_selection_cards(&mut self, player: Player) -> Vec<Card> {
        self.context.mutate_secret(player, |secret, _, _| {
            secret.append_card_selection_to_pointers();
        });

        let player_cards = self.player_cards_mut(player);

        player_cards.pointers += player_cards.card_selection();

        (player_cards.pointers - player_cards.card_selection()..player_cards.pointers)
            .map(|index| OpaquePointer { player, index }.into())
            .collect()
    }

    pub async fn reveal_if_cards_eq(&mut self, a: impl Into<Card>, b: impl Into<Card>) -> bool {
        let a = a.into();
        let b = b.into();

        if let Ok(result) = a.eq(b) {
            return result;
        }

        if let Card::Pointer(OpaquePointer {
            player: a_player,
            index: a_index,
        }) = a
        {
            if let Card::Pointer(OpaquePointer {
                player: b_player,
                index: b_index,
            }) = b
            {
                if a_player == b_player {
                    return self
                        .context
                        .reveal_unique(
                            a_player,
                            move |secret| secret.pointers[a_index] == secret.pointers[b_index],
                            |_| true,
                        )
                        .await;
                }
            }
        }

        let a = match a {
            Card::ID(id) => id,
            Card::Pointer(OpaquePointer { player, index }) => {
                self.context
                    .reveal_unique(player, move |secret| secret.pointers[index], |_| true)
                    .await
            }
        };

        let b = match b {
            Card::ID(id) => id,
            Card::Pointer(OpaquePointer { player, index }) => {
                self.context
                    .reveal_unique(player, move |secret| secret.pointers[index], |_| true)
                    .await
            }
        };

        a == b
    }

    pub async fn reveal_if_cards_ne(&mut self, a: impl Into<Card>, b: impl Into<Card>) -> bool {
        !self.reveal_if_cards_eq(a, b).await
    }

    pub async fn reveal_from_card<T: Secret>(
        &mut self,
        card: impl Into<Card>,
        f: impl Fn(CardInfo<S>) -> T + Clone + 'static,
    ) -> T {
        let card = card.into();

        match card {
            Card::ID(id) => match &self.instances[id.0] {
                InstanceOrPlayer::Instance(instance) => {
                    let (owner, zone) = self.zone(id);
                    let zone = zone.expect(&format!("public {:?} has no zone", id));

                    let attachment = instance.attachment().map(|attachment| {
                        self.instances[attachment.0].instance_ref().expect(&format!(
                            "public {:?} attachment {:?} not public",
                            id, attachment
                        ))
                    });

                    f(CardInfo {
                        instance,
                        owner,
                        zone,
                        attachment,
                    })
                }
                InstanceOrPlayer::Player(owner) => {
                    let owner = {
                        let copy = *owner;
                        drop(owner);
                        copy
                    };

                    self.context
                        .reveal_unique(
                            owner,
                            move |secret| {
                                secret
                                    .reveal_from_card(id, f.clone())
                                    .expect(&format!("{:?} not in player {:?} secret", id, owner))
                            },
                            |_| true,
                        )
                        .await
                }
            },
            Card::Pointer(OpaquePointer { player, index }) => {
                let revealed = self
                    .context
                    .reveal_unique(
                        player,
                        {
                            let f = f.clone();

                            move |secret| {
                                secret
                                    .reveal_from_card(secret.pointers[index], |instance| {
                                        Either::A(f(instance))
                                    })
                                    .unwrap_or_else(|| Either::B(secret.pointers[index]))
                            }
                        },
                        |_| true,
                    )
                    .await;

                match revealed {
                    Either::A(result) => result,
                    Either::B(id) => match &self.instances[id.0] {
                        InstanceOrPlayer::Instance(instance) => {
                            let (owner, zone) = self.zone(id);
                            let zone = zone.expect(&format!("public {:?} has no zone", id));

                            let attachment = instance.attachment().map(|attachment| {
                                self.instances[attachment.0].instance_ref().expect(&format!(
                                    "public {:?} attachment {:?} not public",
                                    id, attachment
                                ))
                            });

                            f(CardInfo {
                                instance,
                                owner,
                                zone,
                                attachment,
                            })
                        }
                        InstanceOrPlayer::Player(owner) => {
                            let owner = {
                                let copy = *owner;
                                drop(owner);
                                copy
                            };

                            self.context
                                .reveal_unique(
                                    owner,
                                    move |secret| {
                                        secret.reveal_from_card(id, f.clone()).expect(&format!(
                                            "{:?} not in player {:?} secret",
                                            id, owner
                                        ))
                                    },
                                    |_| true,
                                )
                                .await
                        }
                    },
                }
            }
        }
    }

    pub async fn reveal_from_cards<T: Secret>(
        &mut self,
        cards: Vec<Card>,
        f: impl Fn(CardInfo<S>) -> T,
    ) -> Vec<T> {
        todo!();
    }

    pub async fn reveal_parent(&mut self, card: impl Into<Card>) -> Option<Card> {
        let card = card.into();

        match card {
            Card::ID(id) => match self.instances[id.0] {
                InstanceOrPlayer::Instance(..) => {
                    let mut parents = self.instances.iter().filter_map(|instance| {
                        instance.instance_ref().and_then(|instance| {
                            if instance.attachment == Some(id) {
                                Some(instance.id())
                            } else {
                                None
                            }
                        })
                    });

                    parents.next().map(|parent| {
                        assert!(parents.next().is_none());

                        parent.into()
                    })
                }
                InstanceOrPlayer::Player(owner) => {
                    let parents = self
                        .new_secret_pointers(owner, |mut secret| {
                            let parents: Vec<_> = secret
                                .instances
                                .values()
                                .filter_map(|instance| {
                                    if instance.attachment == Some(id) {
                                        Some(instance.id())
                                    } else {
                                        None
                                    }
                                })
                                .collect();

                            assert!(parents.len() <= 1);

                            parents
                                .into_iter()
                                .for_each(|parent| secret.new_pointer(parent));
                        })
                        .await;

                    assert!(parents.len() <= 1);

                    parents.into_iter().next()
                }
            },
            Card::Pointer(OpaquePointer { player, index }) => {
                let id = self
                    .context
                    .reveal_unique(
                        player,
                        move |secret| {
                            let id = secret.pointers[index];

                            let parents = secret
                                .instances
                                .values()
                                .filter(|instance| instance.attachment == Some(id))
                                .count();

                            match parents {
                                0 => Some(id),
                                1 => None,
                                parents => unreachable!("{:?} has {} parents", id, parents),
                            }
                        },
                        |_| true,
                    )
                    .await;

                match id {
                    None => {
                        let parents = self
                            .new_secret_pointers(player, |mut secret| {
                                let id = secret.pointers[index];

                                let parents: Vec<_> = secret
                                    .instances
                                    .values()
                                    .filter_map(|instance| {
                                        if instance.attachment == Some(id) {
                                            Some(instance.id())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();

                                assert!(parents.len() <= 1);

                                parents
                                    .into_iter()
                                    .for_each(|parent| secret.new_pointer(parent));
                            })
                            .await;

                        assert!(parents.len() <= 1);

                        parents.into_iter().next()
                    }
                    Some(id) => match self.instances[id.0] {
                        InstanceOrPlayer::Instance(..) => {
                            let mut parents = self.instances.iter().filter_map(|instance| {
                                instance.instance_ref().and_then(|instance| {
                                    if instance.attachment == Some(id) {
                                        Some(instance.id())
                                    } else {
                                        None
                                    }
                                })
                            });

                            parents.next().map(|parent| {
                                assert!(parents.next().is_none());

                                parent.into()
                            })
                        }
                        InstanceOrPlayer::Player(owner) => {
                            let parents = self
                                .new_secret_pointers(owner, |mut secret| {
                                    let parents: Vec<_> = secret
                                        .instances
                                        .values()
                                        .filter_map(|instance| {
                                            if instance.attachment == Some(id) {
                                                Some(instance.id())
                                            } else {
                                                None
                                            }
                                        })
                                        .collect();

                                    assert!(parents.len() <= 1);

                                    parents
                                        .into_iter()
                                        .for_each(|parent| secret.new_pointer(parent));
                                })
                                .await;

                            assert!(parents.len() <= 1);

                            parents.into_iter().next()
                        }
                    },
                }
            }
        }
    }

    pub async fn reveal_parents(&mut self, cards: Vec<Card>) -> Vec<Option<Card>> {
        todo!();
    }

    pub async fn filter_cards(
        &mut self,
        cards: Vec<Card>,
        f: impl Fn(CardInfo<S>) -> bool,
    ) -> Vec<Card> {
        let f = self.reveal_from_cards(cards.clone(), f).await;

        assert_eq!(f.len(), cards.len());

        cards
            .into_iter()
            .zip(f)
            .filter_map(|(card, f)| if f { Some(card) } else { None })
            .collect()
    }

    pub async fn reset_card(&mut self, card: impl Into<Card>) {
        let card = card.into();

        match card {
            Card::ID(id) => match &self.instances[id.0] {
                InstanceOrPlayer::Instance(instance) => {
                    // public ID to public instance

                    let owner = self.owner(id);

                    let attachment = instance.attachment().map(|attachment| {
                        self.instances[attachment.0].instance_ref().expect(&format!(
                            "public {:?} attachment {:?} not public",
                            id, attachment
                        ))
                    });

                    match (
                        attachment.map(|attachment| &attachment.base),
                        instance.base.attachment(),
                    ) {
                        (None, None) => {
                            // do nothing
                        }
                        (Some(..), None) => {
                            // dust current attachment

                            let attachment = attachment
                                .expect("attachment base exists, but no attachment")
                                .id();

                            self.move_card(attachment, owner, Zone::Dust { public: true })
                                .await
                                .expect(&format!(
                                    "unable to move attachment {:?} to public dust",
                                    attachment
                                ));
                        }
                        (None, Some(default)) => {
                            // attach base attachment

                            let attachment = self.new_card(owner, default);

                            self.move_card(
                                attachment,
                                owner,
                                Zone::Attachment { parent: id.into() },
                            )
                            .await
                            .expect(&format!(
                                "unable to attach public limbo {:?} to {:?}",
                                attachment, id
                            ));
                        }
                        (Some(current), Some(default)) if *current == default => {
                            // reset current attachment

                            let attachment = attachment
                                .expect("attachment base exists, but no attachment")
                                .id();

                            let attachment = self.instances[attachment.0].instance_mut().expect("immutable attachment instance exists, but no mutable attachment instance");

                            attachment.state = default.new_card_state();
                        }
                        (Some(..), Some(default)) => {
                            // dust current attachment

                            let attachment = attachment
                                .expect("attachment base exists, but no attachment")
                                .id();

                            self.move_card(attachment, owner, Zone::Dust { public: true })
                                .await
                                .expect(&format!(
                                    "unable to move attachment {:?} to public dust",
                                    attachment
                                ));

                            // attach base attachment

                            let attachment = self.new_card(owner, default);

                            self.move_card(
                                attachment,
                                owner,
                                Zone::Attachment { parent: id.into() },
                            )
                            .await
                            .expect(&format!(
                                "unable to attach public limbo {:?} to {:?}",
                                attachment, id
                            ));
                        }
                    }

                    let instance = self.instances[id.0]
                        .instance_mut()
                        .expect("immutable instance exists, but no mutable instance");

                    instance.state = instance.base.new_card_state();
                }
                InstanceOrPlayer::Player(owner) => {
                    // public ID to secret instance

                    let owner = {
                        let copy = *owner;
                        drop(owner);
                        copy
                    };

                    self.new_secret_cards(owner, |mut secret| {
                        let instance = secret
                            .instance(id)
                            .expect(&format!("player {} secret {:?} not in secret", owner, id));

                        let attachment = instance.attachment().map(|attachment| {
                            secret.instance(attachment).expect(&format!(
                                "player {} secret {:?} attachment {:?} not secret",
                                owner, id, attachment
                            ))
                        });

                        if let Some(player_secret::Mode::NewCards(attachment_id)) = secret.mode {
                            match (
                                attachment,
                                instance.base.attachment(),
                            ) {
                                (None, None) => {
                                    // do nothing
                                }
                                (Some(current), None) => {
                                    // dust current attachment

                                    let current_id = current.id();

                                    secret.dust_card(current_id).expect("current_id is in this secret, and is not already dust.");
                                }
                                (None, Some(default)) => {
                                    // attach base attachment

                                    let state = default.new_card_state();

                                    let attachment = CardInstance {
                                        id: attachment_id,
                                        base: default,
                                        attachment: None,
                                        state,
                                    };

                                    secret.instances.insert(attachment_id, attachment);

                                    secret.attach_card(id, attachment_id).expect("Both id and attachment_id are in this secret.");
                                }
                                (Some(current), Some(default)) if current.base == default => {
                                    // reset current attachment
                                    let attachment_base_state = current.base.new_card_state();
                                    let current_id = current.id();
                                    secret.instance_mut(current_id).expect("").state = attachment_base_state;
                            }
                                (Some(current), Some(default)) => {
                                    // dust current attachment
                                    let current_id = current.id();
                                    secret.dust_card(current_id).expect("current_id is in this secret, and is not already dust.");

                                    // Attach base attachment
                                    let state = default.new_card_state();

                                    let attachment = CardInstance {
                                        id: attachment_id,
                                        base: default,
                                        attachment: None,
                                        state,
                                    };

                                    secret.instances.insert(attachment_id, attachment);

                                    secret.attach_card(id, attachment_id).expect("Both id and attachment_id are in this secret.");
                                }
                            }

                            // unconditionally increment instance ID to avoid leaking attachment information

                            if let Some(player_secret::Mode::NewCards(attachment_id)) = &mut secret.mode {
                                attachment_id.0 += 1;
                            } else {
                                unreachable!("{:?} is not Mode::NewCards(..) inside CardGame::new_secret_cards", secret.mode);
                            }
                        } else {
                            unreachable!("{:?} is not Mode::NewCards(..) inside CardGame::new_secret_cards", secret.mode);
                        }

                        let instance = secret
                            .instance_mut(id)
                            .expect("immutable instance exists, but no mutable instance");

                        instance.state = instance.base.new_card_state();
                    }).await;
                }
            },
            Card::Pointer(OpaquePointer { player, index }) => {
                let id = self
                    .context
                    .reveal_unique(
                        player,
                        move |secret| {
                            let id = secret.pointers[index];

                            if secret.instances.contains_key(&id) {
                                None
                            } else {
                                Some(id)
                            }
                        },
                        |_| true,
                    )
                    .await;

                match id {
                    None => {
                        let owner = player;

                        self.new_secret_cards(owner, |mut secret| {
                        let id = secret.pointers[index];

                        let instance = secret
                            .instance(id)
                            .expect(&format!("player {} secret {:?} not in secret", owner, id));

                        let attachment = instance.attachment().map(|attachment| {
                            secret.instance(attachment).expect(&format!(
                                "player {} secret {:?} attachment {:?} not secret",
                                owner, id, attachment
                            ))
                        });

                        if let Some(player_secret::Mode::NewCards(attachment_id)) = secret.mode {
                            match (
                                attachment,
                                instance.base.attachment(),
                            ) {
                                (None, None) => {
                                    // do nothing
                                }
                                (Some(current), None) => {
                                    // dust current attachment

                                    let current_id = current.id();

                                    secret.dust_card(current_id).expect("current_id is in this secret, and is not already dust.");
                                }
                                (None, Some(default)) => {
                                    // attach base attachment

                                    let state = default.new_card_state();

                                    let attachment = CardInstance {
                                        id: attachment_id,
                                        base: default,
                                        attachment: None,
                                        state,
                                    };

                                    secret.instances.insert(attachment_id, attachment);

                                    secret.attach_card(id, attachment_id).expect("Both id and attachment_id are in this secret.");
                                }
                                (Some(current), Some(default)) if current.base == default => {
                                    // reset current attachment
                                    let attachment_base_state = current.base.new_card_state();
                                    let current_id = current.id();
                                    secret.instance_mut(current_id).expect("").state = attachment_base_state;
                            }
                                (Some(current), Some(default)) => {
                                    // dust current attachment
                                    let current_id = current.id();
                                    secret.dust_card(current_id).expect("current_id is in this secret, and is not already dust.");

                                    // Attach base attachment
                                    let state = default.new_card_state();

                                    let attachment = CardInstance {
                                        id: attachment_id,
                                        base: default,
                                        attachment: None,
                                        state,
                                    };

                                    secret.instances.insert(attachment_id, attachment);

                                    secret.attach_card(id, attachment_id).expect("Both id and attachment_id are in this secret.");
                                }
                            }

                            // unconditionally increment instance ID to avoid leaking attachment information

                            if let Some(player_secret::Mode::NewCards(attachment_id)) = &mut secret.mode {
                                attachment_id.0 += 1;
                            } else {
                                unreachable!("{:?} is not Mode::NewCards(..) inside CardGame::new_secret_cards", secret.mode);
                            }
                        } else {
                            unreachable!("{:?} is not Mode::NewCards(..) inside CardGame::new_secret_cards", secret.mode);
                        }

                        let instance = secret
                            .instance_mut(id)
                            .expect("immutable instance exists, but no mutable instance");

                        instance.state = instance.base.new_card_state();
                    }).await;
                    }
                    Some(id) => match &self.instances[id.0] {
                        InstanceOrPlayer::Instance(instance) => {
                            // public ID to public instance

                            let owner = self.owner(id);

                            let attachment = instance.attachment().map(|attachment| {
                                self.instances[attachment.0].instance_ref().expect(&format!(
                                    "public {:?} attachment {:?} not public",
                                    id, attachment
                                ))
                            });

                            match (
                                attachment.map(|attachment| &attachment.base),
                                instance.base.attachment(),
                            ) {
                                (None, None) => {
                                    // do nothing
                                }
                                (Some(..), None) => {
                                    // dust current attachment

                                    let attachment = attachment
                                        .expect("attachment base exists, but no attachment")
                                        .id();

                                    self.move_card(attachment, owner, Zone::Dust { public: true })
                                        .await
                                        .expect(&format!(
                                            "unable to move attachment {:?} to public dust",
                                            attachment
                                        ));
                                }
                                (None, Some(default)) => {
                                    // attach base attachment

                                    let attachment = self.new_card(owner, default);

                                    self.move_card(
                                        attachment,
                                        owner,
                                        Zone::Attachment { parent: id.into() },
                                    )
                                    .await
                                    .expect(&format!(
                                        "unable to attach public limbo {:?} to {:?}",
                                        attachment, id
                                    ));
                                }
                                (Some(current), Some(default)) if *current == default => {
                                    // reset current attachment

                                    let attachment = attachment
                                        .expect("attachment base exists, but no attachment")
                                        .id();

                                    let attachment = self.instances[attachment.0].instance_mut().expect("immutable attachment instance exists, but no mutable attachment instance");

                                    attachment.state = default.new_card_state();
                                }
                                (Some(..), Some(default)) => {
                                    // dust current attachment

                                    let attachment = attachment
                                        .expect("attachment base exists, but no attachment")
                                        .id();

                                    self.move_card(attachment, owner, Zone::Dust { public: true })
                                        .await
                                        .expect(&format!(
                                            "unable to move attachment {:?} to public dust",
                                            attachment
                                        ));

                                    // attach base attachment

                                    let attachment = self.new_card(owner, default);

                                    self.move_card(
                                        attachment,
                                        owner,
                                        Zone::Attachment { parent: id.into() },
                                    )
                                    .await
                                    .expect(&format!(
                                        "unable to attach public limbo {:?} to {:?}",
                                        attachment, id
                                    ));
                                }
                            }

                            let instance = self.instances[id.0]
                                .instance_mut()
                                .expect("immutable instance exists, but no mutable instance");

                            instance.state = instance.base.new_card_state();
                        }
                        InstanceOrPlayer::Player(owner) => {
                            // public ID to secret instance

                            let owner = {
                                let copy = *owner;
                                drop(owner);
                                copy
                            };

                            self.new_secret_cards(owner, |mut secret| {
                        let instance = secret
                            .instance(id)
                            .expect(&format!("player {} secret {:?} not in secret", owner, id));

                        let attachment = instance.attachment().map(|attachment| {
                            secret.instance(attachment).expect(&format!(
                                "player {} secret {:?} attachment {:?} not secret",
                                owner, id, attachment
                            ))
                        });

                        if let Some(player_secret::Mode::NewCards(attachment_id)) = secret.mode {
                            match (
                                attachment,
                                instance.base.attachment(),
                            ) {
                                (None, None) => {
                                    // do nothing
                                }
                                (Some(current), None) => {
                                    // dust current attachment

                                    let current_id = current.id();

                                    secret.dust_card(current_id).expect("current_id is in this secret, and is not already dust.");
                                }
                                (None, Some(default)) => {
                                    // attach base attachment

                                    let state = default.new_card_state();

                                    let attachment = CardInstance {
                                        id: attachment_id,
                                        base: default,
                                        attachment: None,
                                        state,
                                    };

                                    secret.instances.insert(attachment_id, attachment);

                                    secret.attach_card(id, attachment_id).expect("Both id and attachment_id are in this secret.");
                                }
                                (Some(current), Some(default)) if current.base == default => {
                                    // reset current attachment
                                    let attachment_base_state = current.base.new_card_state();
                                    let current_id = current.id();
                                    secret.instance_mut(current_id).expect("").state = attachment_base_state;
                            }
                                (Some(current), Some(default)) => {
                                    // dust current attachment
                                    let current_id = current.id();
                                    secret.dust_card(current_id).expect("current_id is in this secret, and is not already dust.");

                                    // Attach base attachment
                                    let state = default.new_card_state();

                                    let attachment = CardInstance {
                                        id: attachment_id,
                                        base: default,
                                        attachment: None,
                                        state,
                                    };

                                    secret.instances.insert(attachment_id, attachment);

                                    secret.attach_card(id, attachment_id).expect("Both id and attachment_id are in this secret.");
                                }
                            }

                            // unconditionally increment instance ID to avoid leaking attachment information

                            if let Some(player_secret::Mode::NewCards(attachment_id)) = &mut secret.mode {
                                attachment_id.0 += 1;
                            } else {
                                unreachable!("{:?} is not Mode::NewCards(..) inside CardGame::new_secret_cards", secret.mode);
                            }
                        } else {
                            unreachable!("{:?} is not Mode::NewCards(..) inside CardGame::new_secret_cards", secret.mode);
                        }

                        let instance = secret
                            .instance_mut(id)
                            .expect("immutable instance exists, but no mutable instance");

                        instance.state = instance.base.new_card_state();
                    }).await;
                        }
                    },
                }
            }
        }
    }

    pub async fn reset_cards(&mut self, cards: Vec<Card>) {
        todo!();
    }

    pub async fn modify_card(&mut self, card: impl Into<Card>, f: impl Fn(CardInfoMut<S>)) {
        let card = card.into();

        match card {
            Card::ID(id) => {
                let Self { state, context } = self;

                match &state.instances[id.0] {
                    InstanceOrPlayer::Instance(instance) => {
                        let (owner, zone) = state.zone(id);
                        let zone = zone.expect(&format!("public {:?} has no zone", id));

                        let attachment = instance.attachment.map(|attachment| {
                            state.instances[attachment.0]
                                .instance_ref()
                                .expect(&format!(
                                    "public {:?} attachment {:?} not public",
                                    id, attachment
                                ))
                                .clone()
                        });

                        let instance = state.instances[id.0]
                            .instance_mut()
                            .expect(&format!("{:?} vanished", id));

                        f(CardInfoMut {
                            instance,
                            owner,
                            zone,
                            attachment: attachment.as_ref(),
                            random: &mut context.random().await,
                            log: &mut |event| context.log(event),
                        });
                    }
                    InstanceOrPlayer::Player(owner) => {
                        self.context.mutate_secret(*owner, |secret, random, log| {
                            secret
                                .modify_card(card, random, log, |instance| f(instance))
                                .expect(&format!(
                                    "player {} secret {:?} not in secret",
                                    owner, card
                                ));
                        });
                    }
                }
            }
            Card::Pointer(OpaquePointer { player, index }) => {
                let id = self
                    .context
                    .reveal_unique(
                        player,
                        move |secret| {
                            let id = secret.pointers[index];

                            if secret.instances.contains_key(&id) {
                                None
                            } else {
                                Some(id)
                            }
                        },
                        |_| true,
                    )
                    .await;

                match id {
                    None => {
                        self.context.mutate_secret(player, |secret, random, log| {
                            secret
                                .modify_card(card, random, log, |instance| f(instance))
                                .expect(&format!(
                                    "player {} secret {:?} not in secret",
                                    player, card
                                ));
                        });
                    }
                    Some(id) => {
                        let Self { state, context } = self;

                        match &state.instances[id.0] {
                            InstanceOrPlayer::Instance(instance) => {
                                let (owner, zone) = state.zone(id);
                                let zone = zone.expect(&format!("public {:?} has no zone", id));

                                let attachment = instance.attachment.map(|attachment| {
                                    state.instances[attachment.0]
                                        .instance_ref()
                                        .expect(&format!(
                                            "public {:?} attachment {:?} not public",
                                            id, attachment
                                        ))
                                        .clone()
                                });

                                let instance = state.instances[id.0]
                                    .instance_mut()
                                    .expect(&format!("{:?} vanished", id));

                                f(CardInfoMut {
                                    instance,
                                    owner,
                                    zone,
                                    attachment: attachment.as_ref(),
                                    random: &mut context.random().await,
                                    log: &mut |event| context.log(event),
                                });
                            }
                            InstanceOrPlayer::Player(owner) => {
                                self.context.mutate_secret(*owner, |secret, random, log| {
                                    secret
                                        .modify_card(card, random, log, |instance| f(instance))
                                        .expect(&format!(
                                            "player {} secret {:?} not in secret",
                                            owner, card
                                        ));
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    pub async fn modify_cards(&mut self, cards: Vec<Card>, f: impl Fn(CardInfoMut<S>)) {
        todo!();
    }

    pub async fn move_card(
        &mut self,
        card: impl Into<Card>,
        to_player: Player,
        to_zone: Zone,
    ) -> Result<(Player, Option<Zone>), error::MoveCardError> {
        let card = card.into();
        let to_bucket = match to_zone {
            Zone::Deck => Some(to_player),
            Zone::Hand { public: false } => Some(to_player),
            Zone::Hand { public: true } => None,
            Zone::Field => None,
            Zone::Graveyard => None,
            Zone::Limbo { public: false } => Some(to_player),
            Zone::Limbo { public: true } => None,
            Zone::CardSelection => Some(to_player),
            Zone::Casting => None,
            Zone::Dust { public: false } => Some(to_player),
            Zone::Dust { public: true } => None,
            Zone::Attachment { parent } => {
                return self.attach_card(card, parent).await;
            }
        };

        // We always need to know who owns the card instance itself.

        // Either this card is in Public state (None) or a player's secret (Some(player)).
        // We also need to know who owns the card, regardless of its secrecy, so we can later update the public state for that player.
        let (bucket, owner) = match card {
            Card::Pointer(OpaquePointer { player, index }) => {
                let buckets: Vec<_> = self
                    .instances
                    .iter()
                    .enumerate()
                    .map(|(i, instance)| (instance.player(), self.owner(InstanceID(i))))
                    .collect();

                self.context
                    .reveal_unique(
                        player,
                        move |secret| buckets[secret.pointers[index].0],
                        |_| true,
                    )
                    .await
            }
            Card::ID(id) => (self.instances[id.0].player(), self.owner(id)),
        };

        let id = match card {
            Card::ID(id) => Some(id),
            Card::Pointer(OpaquePointer { player, index }) => {
                if bucket != Some(player) || to_bucket != Some(player) {
                    Some(
                        self.context
                            .reveal_unique(player, move |secret| secret.pointers[index], |_| true)
                            .await,
                    )
                } else {
                    // The pointer, the card, and the destination all exist within one player's secret.
                    // Therefore, the instance ID need not be revealed.

                    None
                }
            }
        };

        // Reveal the zone that a card came from
        let location = match bucket {
            None => {
                let id = id.expect("ID should have been revealed in this case");

                Some(
                    self.location(id)
                        .1
                        .expect("Location for a public card must be public."),
                )
            }
            Some(player) => {
                self.context
                    .reveal_unique(
                        player,
                        move |secret| {
                            let location = secret
                                .location(id.unwrap_or_else(|| {
                                    secret.pointers[card.pointer().unwrap().index]
                                }))
                                .expect("The secret should know the zone.");
                            match location.0 {
                                Zone::Limbo { public: false } => None,
                                Zone::Attachment { .. } => None,
                                zone => Some((zone, location.1)),
                            }
                        },
                        |_| true,
                    )
                    .await
            }
        };

        // Special case, secret -> secret for a single player
        if let Some(bucket_owner) = bucket {
            if to_bucket == bucket {
                self.context.mutate_secret(bucket_owner, |secret, _, _| {
                    let id = id.unwrap_or_else(|| secret.pointers[card.pointer().unwrap().index]);
                    // Remove this card from its old zone in the secret.
                    secret.remove_id(id);

                    // Put the card in its new zone in the secret.
                    match to_zone {
                        Zone::Deck => secret.deck.push(id),
                        Zone::Hand { public: false } => secret.hand.push(Some(id)),
                        Zone::Hand { public: true } => unreachable!(),
                        Zone::Field => unreachable!(),
                        Zone::Graveyard => unreachable!(),
                        Zone::Limbo { public: false } => secret.limbo.push(id),
                        Zone::Limbo { public: true } => unreachable!(),
                        Zone::CardSelection => secret.card_selection.push(id),
                        Zone::Casting => unreachable!(),
                        Zone::Dust { public: false } => secret.dust.push(id),
                        Zone::Dust { public: true } => unreachable!(),
                        Zone::Attachment { .. } => {
                            unreachable!("Can't attach a spell with move_card.")
                        }
                    }
                });

                if let Some((zone, index)) = location {
                    self.player_cards_mut(bucket_owner).remove_from(zone, index);
                }

                // Update the public state about where we put this card
                let player_state = self.player_cards_mut(to_player);
                match to_zone {
                    Zone::Deck => {
                        player_state.deck += 1;
                    }
                    Zone::Hand { public: false } => {
                        player_state.hand.push(None);
                    }
                    Zone::Hand { public: true } => {
                        unreachable!();
                    }
                    Zone::Field => {
                        unreachable!();
                    }
                    Zone::Graveyard => {
                        unreachable!();
                    }
                    Zone::Limbo { public: false } => {
                        // do nothing, this is a secret
                    }
                    Zone::Limbo { public: true } => {
                        unreachable!();
                    }
                    Zone::CardSelection => {
                        player_state.card_selection += 1;
                    }
                    Zone::Casting => {
                        unreachable!();
                    }
                    Zone::Dust { public: false } => {
                        // do nothing, this is a secret
                    }
                    Zone::Dust { public: true } => {
                        unreachable!();
                    }
                    Zone::Attachment { .. } => unreachable!("Cannot move card to attachment zone"),
                }

                return todo!();
            }
        }

        let (instance, attachment_instance) = match bucket {
            None => {
                let id = id.expect("Card is in public state, but we don't know its id.");

                if let Some(to_bucket_player) = to_bucket {
                    let instance = std::mem::replace(
                        &mut self.instances[id.0],
                        InstanceOrPlayer::Player(to_bucket_player),
                    )
                    .instance()
                    .expect(
                        "Card was identified as public, but it's actually MaybeSecretCard::Secret",
                    );

                    let attachment = instance.attachment.map(|attachment_id| {
                        std::mem::replace(&mut self.instances[attachment_id.0], InstanceOrPlayer::Player(to_bucket_player)).instance().expect("Since parent Card is public, attachment was identified as public, but it's actually MaybeSecretCard::Secret")
                    });

                    self.context.mutate_secret(owner, |secret, _, _| {
                        if let Some((Zone::Hand { public: false }, index)) = location {
                            secret.hand.remove(index);
                        }
                    });

                    (Some(instance), attachment)
                } else {
                    // we're moving from public to public
                    (None, None)
                }
            }
            Some(player) => {
                let (instance, attachment_instance) = self
                    .context
                    .reveal_unique(
                        player,
                        move |secret| {
                            let id = id
                                .unwrap_or_else(|| secret.pointers[card.pointer().unwrap().index]);

                            let instance = secret
                                .instance(id)
                                .expect("Secret has the instance for this ID");

                            (
                                Some(instance.clone()),
                                instance.attachment.map(|attachment| {
                                    secret
                                        .instance(attachment)
                                        .expect("Secret has the instance for this ID")
                                        .clone()
                                }),
                            )
                        },
                        |_| true,
                    )
                    .await;

                self.context.mutate_secret(player, move |secret, _, _| {
                    let id = id.unwrap_or_else(|| secret.pointers[card.pointer().unwrap().index]);

                    // We're removing a card with an attachment from the secret
                    if let Some(attachment_id) = secret.instance(id).expect("").attachment {
                        secret.instances.remove(&attachment_id);
                    }

                    secret.instances.remove(&id);

                    // find what collection id is in and remove it
                    secret.deck.retain(|i| *i != id);
                    secret.hand.retain(|i| *i != Some(id));
                    secret.limbo.retain(|i| *i != id);
                    secret.card_selection.retain(|i| *i != id);
                    secret.dust.retain(|i| *i != id);

                    // We're removing the attachment from a card in the secret
                    if let Some(parent_instance) = secret
                        .instances
                        .values_mut()
                        .find(|c| c.attachment == Some(id))
                    {
                        parent_instance.attachment = None;
                    }
                });
                (instance, attachment_instance)
            }
        };

        // At this point in time, either we already knew ID, or we've revealed it by revealing the instance.
        let id = id
            .or_else(|| instance.as_ref().map(|v| v.id))
            .expect("Either we know ID or we've revealed the instance.");

        let player_state = self.player_cards_mut(to_player);
        match to_zone {
            Zone::Deck => {
                self.context.mutate_secret(to_player, |secret, _, _| {
                    secret.deck.push(id);
                });

                player_state.deck += 1;
            }
            Zone::Hand { public: false } => {
                self.context.mutate_secret(to_player, |secret, _, _| {
                    secret.hand.push(Some(id));
                });

                player_state.hand.push(None);
            }
            Zone::Hand { public: true } => {
                player_state.hand.push(Some(id));

                self.context.mutate_secret(to_player, |secret, _, _| {
                    secret.hand.push(None);
                });
            }
            Zone::Field => {
                player_state.field.push(id);
            }
            Zone::Graveyard => {
                player_state.graveyard.push(id);
            }
            Zone::Limbo { public: false } => {
                self.context.mutate_secret(to_player, |secret, _, _| {
                    secret.limbo.push(id);
                });
            }
            Zone::Limbo { public: true } => {
                player_state.limbo.push(id);
            }
            Zone::CardSelection => {
                self.context.mutate_secret(to_player, |secret, _, _| {
                    secret.card_selection.push(id);
                });

                player_state.card_selection += 1;
            }
            Zone::Casting => {
                player_state.casting.push(id);
            }
            Zone::Dust { public: false } => {
                self.context.mutate_secret(to_player, |secret, _, _| {
                    secret.dust.push(id);
                });
            }
            Zone::Dust { public: true } => {
                player_state.dust.push(id);
            }
            Zone::Attachment { .. } => unreachable!("Cannot move card to attachment zone"),
        }

        if let Some(instance) = instance {
            // we have a new instance, need to put it somewhere.
            let id = instance.id;

            match to_bucket {
                None => {
                    self.instances[id.0] = instance.into();
                }
                Some(to_bucket_player) => {
                    self.instances[id.0] = to_bucket_player.into();

                    self.context
                        .mutate_secret(to_bucket_player, move |secret, _, _| {
                            secret.instances.insert(instance.id, instance.clone());
                        });
                }
            }

            // If we have an attachment_instance, we also need to put it somewhere the same way.
            if let Some(attachment_instance) = attachment_instance {
                let attachment_id = attachment_instance.id;

                match to_bucket {
                    None => {
                        self.instances[attachment_id.0] = attachment_instance.into();
                    }
                    Some(to_bucket_player) => {
                        let attachment_id = attachment_instance.id;
                        self.instances[attachment_id.0] = to_bucket_player.into();

                        self.context
                            .mutate_secret(to_bucket_player, move |secret, _, _| {
                                secret
                                    .instances
                                    .insert(attachment_instance.id, attachment_instance.clone());
                            });
                    }
                }
            }
        }

        match location {
            Some((
                Zone::Attachment {
                    parent: Card::ID(id),
                },
                ..,
            )) => {
                self.instances[id.0]
                    .instance_mut()
                    .expect("Card should have been attached to a public parent")
                    .attachment = None;
            }
            Some(location) => {
                self.player_cards_mut(owner).remove_from(location);
            }
            None => (),
        }

        todo!();
    }

    pub async fn move_cards(
        &mut self,
        cards: Vec<Card>,
        to_player: Player,
        to_zone: Zone,
    ) -> Vec<Result<(Player, Option<Zone>), error::MoveCardError>> {
        todo!();
    }

    pub async fn draw_card(&mut self, player: Player) -> Option<Card> {
        let cards = self.draw_cards(player, 1).await;

        assert!(cards.len() <= 1);

        cards.into_iter().next()
    }

    pub async fn draw_cards(&mut self, player: Player, count: usize) -> Vec<Card> {
        let cards = self
            .deck_cards(player)
            .into_iter()
            .choose_multiple(&mut self.context.random().await, count);

        self.move_cards(cards.clone(), player, Zone::Hand { public: false })
            .await;

        cards
    }

    pub async fn new_secret_cards(
        &mut self,
        player: Player,
        f: impl Fn(SecretInfo<S>),
    ) -> Vec<Card> {
        let start = self.instances.len();

        self.context.mutate_secret(player, |secret, random, log| {
            secret.mode = Some(player_secret::Mode::NewCards(InstanceID(start)));

            f(SecretInfo {
                secret,
                random,
                log,
            })
        });

        let (pointers, end) = self
            .context
            .reveal_unique(
                player,
                |secret| {
                    if let Some(player_secret::Mode::NewCards(id)) = secret.mode {
                        (secret.pointers.len(), id.0)
                    } else {
                        unreachable!("{:?} is not Mode::NewCards(..)", secret.mode);
                    }
                },
                |_| true,
            )
            .await;

        assert!(pointers >= self.player_cards(player).pointers);
        assert!(end >= start);

        self.context.mutate_secret(player, |secret, _, _| {
            secret.mode = None;
        });

        self.instances
            .extend(repeat(InstanceOrPlayer::Player(player)).take(end - start));

        let player_cards = self.player_cards_mut(player);

        let cards = (player_cards.pointers..pointers)
            .map(|index| OpaquePointer { player, index }.into())
            .collect();

        player_cards.pointers = pointers;

        cards
    }

    pub async fn new_secret_pointers(
        &mut self,
        player: Player,
        f: impl Fn(SecretInfo<S>),
    ) -> Vec<Card> {
        self.context.mutate_secret(player, |secret, random, log| {
            secret.mode = Some(player_secret::Mode::NewPointers);

            f(SecretInfo {
                secret,
                random,
                log,
            })
        });

        let pointers = self
            .context
            .reveal_unique(player, |secret| secret.pointers.len(), |_| true)
            .await;

        assert!(pointers >= self.player_cards(player).pointers);

        self.context.mutate_secret(player, |secret, _, _| {
            secret.mode = None;
        });

        let player_cards = self.player_cards_mut(player);

        let cards = (player_cards.pointers..pointers)
            .map(|index| OpaquePointer { player, index }.into())
            .collect();

        player_cards.pointers = pointers;

        cards
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub async fn print(&mut self) {
        todo!();
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub async fn reveal_ok(&mut self) -> Result<(), error::RevealOkError> {
        todo!();
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub async fn is_public(&mut self, card: impl Into<Card>) -> bool {
        let card = card.into();

        match card {
            Card::ID(id) => self.instances[id.0].instance_ref().is_some(),
            Card::Pointer(OpaquePointer { player, index }) => {
                let is_public: Vec<_> = self
                    .instances
                    .iter()
                    .map(|instance| instance.instance_ref().is_some())
                    .collect();

                self.context
                    .reveal_unique(
                        player,
                        move |secret| is_public[secret.pointers[index].0],
                        |_| true,
                    )
                    .await
            }
        }
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub async fn is_secret(&mut self, card: impl Into<Card>) -> bool {
        !self.is_public(card).await
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub async fn move_pointer(&mut self, card: impl Into<Card>, player: Player) {
        todo!();
    }

    fn attach_card(
        &mut self,
        card: impl Into<Card>,
        parent: impl Into<Card>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(Player, Option<Zone>), error::MoveCardError>>>,
    > {
        let card = card.into();
        let parent = parent.into();
        Box::pin(async move {
            let buckets: Vec<_> = self
                .instances
                .iter()
                .map(|instance| instance.player())
                .collect();
            let card_bucket = match card {
                Card::ID(_) => None,
                Card::Pointer(OpaquePointer { player, index }) => {
                    self.context
                        .reveal_unique(
                            player,
                            move |secret| buckets[secret.pointers[index].0],
                            |_| true,
                        )
                        .await
                }
            };

            let parent_bucket = match parent {
                Card::ID(_) => None,
                Card::Pointer(OpaquePointer { player, index }) => {
                    self.context
                        .reveal_unique(
                            player,
                            move |secret| buckets[secret.pointers[index].0],
                            |_| true,
                        )
                        .await
                }
            };

            // Dust parent's current attachment, if any
            let parent_id = match parent_bucket {
                None => {
                    let (id, owner, attachment) = self
                        .reveal_from_card(parent, |instance| {
                            (
                                instance.id,
                                instance.owner,
                                instance.attachment.map(|attachment| attachment.id),
                            )
                        })
                        .await;

                    if let Some(attachment) = attachment {
                        self.move_card(attachment, owner, Zone::Dust { public: true })
                            .await
                            .expect(&format!(
                                "unable to move attachment {:?} to public dust",
                                attachment
                            ));
                    }

                    Some(id)
                }
                Some(parent_card_player) => match parent {
                    Card::Pointer(OpaquePointer {
                        player: ptr_player,
                        index,
                    }) if ptr_player == parent_card_player => {
                        self.context
                            .mutate_secret(parent_card_player, |secret, _, log| {
                                let id = secret.pointers[index];

                                if let Some(attachment) = secret.instance(id).expect("").attachment
                                {
                                    secret.dust_card(attachment).expect("");
                                }
                            });

                        None
                    }
                    Card::Pointer(..) => {
                        let id = match parent {
                            Card::ID(id) => id,
                            Card::Pointer(OpaquePointer { player, index }) => {
                                self.context
                                    .reveal_unique(
                                        player,
                                        |secret| secret.pointers[index],
                                        |_| true,
                                    )
                                    .await
                            }
                        };

                        self.context
                            .mutate_secret(parent_card_player, |secret, _, log| {
                                if let Some(attachment) = secret.instance(id).expect("").attachment
                                {
                                    secret.dust_card(attachment).expect("");
                                }
                            });

                        Some(id)
                    }
                    Card::ID(id) => {
                        self.context
                            .mutate_secret(parent_card_player, |secret, _, log| {
                                if let Some(attachment) = secret.instance(id).expect("").attachment
                                {
                                    secret.dust_card(attachment).expect("");
                                }
                            });

                        Some(id)
                    }
                },
            };

            // Remove card from its current zone, secretly and possibly publicly.

            let card_id = match card {
                Card::Pointer(OpaquePointer {
                    player: card_ptr_player,
                    index,
                }) => {
                    if Some(card_ptr_player) == card_bucket {
                        None
                    } else {
                        Some(match card {
                            Card::ID(id) => id,
                            Card::Pointer(OpaquePointer { player, index }) => {
                                self.context
                                    .reveal_unique(
                                        player,
                                        |secret| secret.pointers[index],
                                        |_| true,
                                    )
                                    .await
                            }
                        })
                    }
                }
                Card::ID(card_id) => Some(card_id),
            };

            if let (card_owner, Some(card_location)) = self.reveal_id_location(card).await {
                self.player_cards_mut(card_owner).remove_from(card_location);
            }

            if let Some(card_id) = card_id {
                self.game.remove_id(card_id);
            }

            match card {
                Card::ID(id) => {
                    for player in 0..2 {
                        self.context.mutate_secret(player, |secret, _, _| {
                            secret.remove_id(id);
                        });
                    }
                }
                Card::Pointer(OpaquePointer { player, index }) => {
                    self.context.mutate_secret(player, |secret, _, _| {
                        let id = secret.pointers[index];

                        secret.remove_id(id);
                    });
                }
            }

            // Step 3 and 4 only need to be performed if the source and destination buckets are different.
            // Move card to parent's bucket.

            if card_bucket != parent_bucket {
                // Step 3:
                // Remove card from its current bucket.
                let card_id = match card_id {
                    None => {
                        self.context
                            .reveal_unique(
                                card.pointer()
                                    .expect("Card pointer should be secret")
                                    .player,
                                move |secret| {
                                    secret.pointers[card
                                        .pointer()
                                        .expect("Card pointer should be secret")
                                        .index]
                                },
                                |_| true,
                            )
                            .await
                    }
                    Some(card_id) => card_id,
                };

                let instance = match card_bucket {
                    None => {
                        let parent_bucket_player = parent_bucket
                            .expect("parent bucket isn't public, but also not a player's secret");

                        self.instances[card_id.0] = InstanceOrPlayer::Player(parent_bucket_player);
                    }
                    Some(card_bucket_player) => {
                        let instance = self
                            .context
                            .reveal_unique(
                                card_bucket_player,
                                move |secret| secret.instance(card_id).clone(),
                                |_| true,
                            )
                            .await;

                        self.context
                            .mutate_secret(card_bucket_player, |secret, _, _| {
                                secret.remove_id(card_id);
                            });

                        instance
                    }
                };

                // Step 4:
                // Add card to parent's bucket.
                match parent_bucket {
                    None => {
                        self.instances[card_id.0] = InstanceOrPlayer::Instance(instance);
                    }
                    Some(parent_bucket_player) => {
                        self.instances[card_id.0] = InstanceOrPlayer::Player(parent_bucket_player);

                        self.context
                            .mutate_secret(parent_bucket_player, |secret, _, _| {
                                secret.instances.insert(card_id, instance.clone());
                            });
                    }
                }
            }

            // Step 5:
            // Add card to parent's attachment zone.

            // can't use .await in an Option::or
            let card_id = match card_id {
                None => {
                    // we don't reveal the card id if it's in the same bucket as the parent
                    if let Some(parent_bucket_player) = parent_bucket {
                        if card_bucket != Some(parent_bucket_player) {
                            Some(
                                self
                                    .context
                                    .reveal_unique(
                                        card_bucket.expect("We would have had a card_id if the card was in the public bucket"),
                                        move |secret| {
                                            secret.pointers[card.pointer().expect("").index]
                                        },
                                        |_| true
                                    ).await
                            )
                        } else {
                            // parent card and card we're attaching are both in the same bucket
                            None
                        }
                    } else {
                        // parent pointer & parent card are both in some player's secret
                        None
                    }
                }
                card_id => card_id,
            };

            match parent_id {
                None => {
                    let parent_bucket_player = parent_bucket
                        .expect("Parent pointer and card are both in some player's secret");

                    self.context
                        .mutate_secret(parent_bucket_player, |secret, _, _| {
                            let card_id = card_id.unwrap_or_else(|| {
                                secret.pointers[card.pointer().expect("").index]
                            });

                            secret.instance(parent).expect("").attachment = Some(card_id);
                        });
                }
                Some(parent_id) => match parent_bucket {
                    None => {
                        let card_id = match card_id {
                            None => match card {
                                Card::ID(id) => id,
                                Card::Pointer(OpaquePointer { player, index }) => {
                                    self.context
                                        .reveal_unique(
                                            player,
                                            |secret| secret.pointers[index],
                                            |_| true,
                                        )
                                        .await
                                }
                            },
                            Some(card_id) => card_id,
                        };

                        self.instances[parent_id.0].instance_mut().expect("If the parent bucket is public, the public state must have that card").attachment = Some(card_id);
                    }
                    Some(parent_bucket_player) => {
                        self.context
                            .mutate_secret(parent_bucket_player, |secret, _, _| {
                                let card_id = card_id.unwrap_or_else(|| {
                                    secret.pointers[card.pointer().expect("").index]
                                });

                                secret.instance_mut(parent_id).expect("").attachment =
                                    Some(card_id);
                            })
                    }
                },
            }
        })
    }
}

pub struct CardInfo<'a, S: State> {
    pub instance: &'a CardInstance<S>,
    pub owner: Player,
    pub zone: Zone,
    pub attachment: Option<&'a CardInstance<S>>,
}

pub struct CardInfoMut<'a, S: State> {
    pub instance: &'a mut CardInstance<S>,
    pub owner: Player,
    pub zone: Zone,
    pub attachment: Option<&'a CardInstance<S>>,
    pub random: &'a mut dyn rand::RngCore,
    pub log: &'a mut dyn FnMut(&dyn Event),
}

pub struct SecretInfo<'a, S: State> {
    pub secret: &'a mut PlayerSecret<S>,
    pub random: &'a mut dyn rand::RngCore,
    pub log: &'a mut dyn FnMut(&dyn Event),
}

impl<S: State> Deref for CardInfo<'_, S> {
    type Target = CardInstance<S>;

    fn deref(&self) -> &Self::Target {
        self.instance
    }
}

impl<S: State> Deref for CardInfoMut<'_, S> {
    type Target = CardInstance<S>;

    fn deref(&self) -> &Self::Target {
        self.instance
    }
}

impl<S: State> DerefMut for CardInfoMut<'_, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.instance
    }
}

impl<S: State> Deref for SecretInfo<'_, S> {
    type Target = PlayerSecret<S>;

    fn deref(&self) -> &Self::Target {
        self.secret
    }
}

impl<S: State> DerefMut for SecretInfo<'_, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.secret
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
enum Either<A, B> {
    A(A),
    B(B),
}
