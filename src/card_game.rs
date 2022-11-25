use {
    crate::{
        error, BaseCard, Card, CardEvent, CardInstance, CardLocation, CardState, Context,
        ExactCardLocation, GameState, InstanceID, InstanceOrPlayer, OpaquePointer, Player, Secret,
        State, Zone,
    },
    rand::seq::IteratorRandom,
    std::{
        cmp::Ordering,
        convert::TryInto,
        future::Future,
        iter::repeat,
        ops::{Deref, DerefMut},
        pin::Pin,
    },
};

#[cfg(feature = "bindings")]
use wasm_bindgen::prelude::wasm_bindgen;

type AttachCardResult = Result<(CardLocation, Option<InstanceID>), error::MoveCardError>;

pub struct CardGame<S: State> {
    pub state: GameState<S>,

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

    pub async fn new_card(&mut self, player: Player, base: S::BaseCard) -> InstanceID {
        let id = InstanceID(self.instances.len());
        let state = base.new_card_state(None);
        let instance: CardInstance<S> = CardInstance {
            id,
            base: base.clone(),
            attachment: None,
            state,
        };

        self.instances
            .push(InstanceOrPlayer::from(instance.clone()));

        self.player_cards_mut(player).limbo.push(id);

        if let Some(attach_base) = base.attachment() {
            let attach_id = InstanceID(self.instances.len());
            let state = attach_base.new_card_state(Some(&instance));
            let instance: CardInstance<S> = CardInstance {
                id: attach_id,
                base: attach_base,
                attachment: None,
                state,
            };

            self.instances
                .push(InstanceOrPlayer::from(instance.clone()));
            self.player_cards_mut(player).limbo.push(attach_id);

            self.move_card(attach_id, player, Zone::Attachment { parent: id.into() })
                .await
                .unwrap();
        }
        id
    }

    pub fn deck_card(&mut self, player: Player, index: usize) -> Card {
        self.context.mutate_secret(player, |mut secret| {
            let pointer = secret.deck()[index];
            secret.pointers.push(pointer);
        });

        let player_cards = self.player_cards_mut(player);

        player_cards.pointers += 1;

        let pointer = OpaquePointer {
            player,
            index: player_cards.pointers - 1,
        };

        self.context.log(CardEvent::NewPointer {
            pointer,
            location: ExactCardLocation {
                player,
                location: (Zone::Deck, index),
            },
        });

        pointer.into()
    }

    pub fn hand_card(&mut self, player: Player, index: usize) -> Card {
        match self.player_cards(player).hand()[index] {
            Some(id) => id.into(),
            None => {
                self.context.mutate_secret(player, |mut secret| {
                    let pointer = secret.hand()[index].unwrap_or_else(|| {
                        panic!(
                            "player {} hand {} is neither public nor secret",
                            player, index
                        )
                    });
                    secret.pointers.push(pointer);
                });

                let player_cards = self.player_cards_mut(player);

                player_cards.pointers += 1;

                let pointer = OpaquePointer {
                    player,
                    index: player_cards.pointers - 1,
                };

                self.context.log(CardEvent::NewPointer {
                    pointer,
                    location: ExactCardLocation {
                        player,
                        location: (Zone::Hand { public: false }, index),
                    },
                });

                pointer.into()
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
        self.context.mutate_secret(player, |mut secret| {
            let pointer = secret.dust()[index];
            secret.pointers.push(pointer);
        });

        let player_cards = self.player_cards_mut(player);

        player_cards.pointers += 1;

        let pointer = OpaquePointer {
            player,
            index: player_cards.pointers - 1,
        };

        self.context.log(CardEvent::NewPointer {
            pointer,
            location: ExactCardLocation {
                player,
                location: (Zone::Dust { public: false }, index),
            },
        });

        pointer.into()
    }

    pub fn public_limbo_card(&self, player: Player, index: usize) -> InstanceID {
        self.player_cards(player).limbo()[index]
    }

    pub fn secret_limbo_card(&mut self, player: Player, index: usize) -> Card {
        self.context.mutate_secret(player, |mut secret| {
            let pointer = secret.limbo()[index];
            secret.pointers.push(pointer);
        });

        let player_cards = self.player_cards_mut(player);

        player_cards.pointers += 1;

        let pointer = OpaquePointer {
            player,
            index: player_cards.pointers - 1,
        };

        self.context.log(CardEvent::NewPointer {
            pointer,
            location: ExactCardLocation {
                player,
                location: (Zone::Limbo { public: false }, index),
            },
        });

        pointer.into()
    }

    pub fn casting_card(&self, player: Player, index: usize) -> InstanceID {
        self.player_cards(player).casting()[index]
    }

    pub fn card_selection_card(&mut self, player: Player, index: usize) -> Card {
        self.context.mutate_secret(player, |mut secret| {
            let pointer = secret.card_selection()[index];
            secret.pointers.push(pointer);
        });

        let player_cards = self.player_cards_mut(player);

        player_cards.pointers += 1;

        let pointer = OpaquePointer {
            player,
            index: player_cards.pointers - 1,
        };

        self.context.log(CardEvent::NewPointer {
            pointer,
            location: ExactCardLocation {
                player,
                location: (Zone::CardSelection, index),
            },
        });

        pointer.into()
    }

    pub fn deck_cards(&mut self, player: Player) -> Vec<Card> {
        self.context.mutate_secret(player, |mut secret| {
            secret.append_deck_to_pointers();
        });

        let player_cards = self.player_cards_mut(player);

        player_cards.pointers += player_cards.deck();

        let pointers: Vec<OpaquePointer> = (player_cards.pointers - player_cards.deck()
            ..player_cards.pointers)
            .map(|index| OpaquePointer { player, index })
            .collect();

        for (index, pointer) in pointers.iter().enumerate() {
            self.context.log(CardEvent::NewPointer {
                pointer: *pointer,
                location: ExactCardLocation {
                    player,
                    location: (Zone::Deck, index),
                },
            });
        }

        pointers.into_iter().map(Into::into).collect()
    }

    pub fn hand_cards(&mut self, player: Player) -> Vec<Card> {
        self.context.mutate_secret(player, |mut secret| {
            secret.append_secret_hand_to_pointers();
        });

        let secret_hand_indices: Vec<usize> = self
            .player_cards(player)
            .hand()
            .iter()
            .enumerate()
            .filter(|(_, id)| id.is_none())
            .map(|(hand_index, _)| hand_index)
            .collect();

        let num_secret_cards = secret_hand_indices.len();
        let pointer_offset = self.player_cards(player).pointers;

        self.player_cards_mut(player).pointers += num_secret_cards;

        for (pointer_index, hand_index) in secret_hand_indices.into_iter().enumerate() {
            self.context.log(CardEvent::NewPointer {
                pointer: OpaquePointer {
                    player,
                    index: pointer_offset + pointer_index,
                },
                location: ExactCardLocation {
                    player,
                    location: (Zone::Hand { public: false }, hand_index),
                },
            });
        }

        let mut secret_hand = (self.player_cards(player).pointers - num_secret_cards
            ..self.player_cards(player).pointers)
            .map(|index| OpaquePointer { player, index });

        let hand = self
            .player_cards(player)
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
        self.context.mutate_secret(player, |mut secret| {
            secret.append_card_selection_to_pointers();
        });

        let num_hand_selection = self.player_cards(player).card_selection();

        self.player_cards_mut(player).pointers += num_hand_selection;

        let start_ptr =
            self.player_cards(player).pointers - self.player_cards(player).card_selection();
        let end_ptr = self.player_cards(player).pointers;

        for (zone_index, pointer_index) in (start_ptr..end_ptr).enumerate() {
            self.context.log(CardEvent::NewPointer {
                pointer: OpaquePointer {
                    player,
                    index: pointer_index,
                },
                location: ExactCardLocation {
                    player,
                    location: (Zone::CardSelection, zone_index),
                },
            });
        }
        (start_ptr..end_ptr)
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

    pub async fn reveal_if_any(
        &mut self,
        cards: Vec<Card>,
        f: impl Fn(CardInfo<S>) -> bool + Clone + 'static,
    ) -> bool {
        // todo!(): betterize this implementation

        self.reveal_from_cards_fold(cards, f, false, move |acc, c| acc || *c)
            .await
    }

    pub async fn reveal_if_every(
        &mut self,
        cards: Vec<Card>,
        f: impl Fn(CardInfo<S>) -> bool + Clone + 'static,
    ) -> bool {
        !self.reveal_if_any(cards, move |card| !f(card)).await
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
                    let CardLocation {
                        player: owner,
                        location,
                    } = self.location(id);
                    let location =
                        location.unwrap_or_else(|| panic!("public {:?} has no zone", id));

                    let attachment = instance.attachment().map(|attachment| {
                        self.instances[attachment.0]
                            .instance_ref()
                            .unwrap_or_else(|| {
                                panic!("public {:?} attachment {:?} not public", id, attachment)
                            })
                    });

                    f(CardInfo {
                        instance,
                        owner,
                        zone: location.0,
                        attachment,
                    })
                }
                InstanceOrPlayer::Player(owner) => {
                    let owner = *owner;

                    self.context
                        .reveal_unique(
                            owner,
                            move |secret| {
                                secret.reveal_from_card(id, f.clone()).unwrap_or_else(|| {
                                    panic!("{:?} not in player {:?} secret", id, owner)
                                })
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
                            let CardLocation {
                                player: owner,
                                location,
                            } = self.location(id);
                            let location =
                                location.unwrap_or_else(|| panic!("public {:?} has no zone", id));

                            let attachment = instance.attachment().map(|attachment| {
                                self.instances[attachment.0]
                                    .instance_ref()
                                    .unwrap_or_else(|| {
                                        panic!(
                                            "public {:?} attachment {:?} not public",
                                            id, attachment
                                        )
                                    })
                            });

                            f(CardInfo {
                                instance,
                                owner,
                                zone: location.0,
                                attachment,
                            })
                        }
                        InstanceOrPlayer::Player(owner) => {
                            let owner = *owner;

                            self.context
                                .reveal_unique(
                                    owner,
                                    move |secret| {
                                        secret.reveal_from_card(id, f.clone()).unwrap_or_else(
                                            || panic!("{:?} not in player {:?} secret", id, owner),
                                        )
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
        f: impl Fn(CardInfo<S>) -> T + Clone + 'static,
    ) -> Vec<T> {
        // todo!(): betterize this implementation

        let mut revealed = Vec::with_capacity(cards.len());

        for card in cards {
            revealed.push(self.reveal_from_card(card, f.clone()).await);
        }

        revealed
    }

    fn card_info(&self, pub_id: InstanceID) -> CardInfo<S> {
        let CardLocation {
            player: owner,
            location,
        } = self.location(pub_id);
        let location = location.unwrap_or_else(|| panic!("public {:?} has no zone", pub_id));

        let instance = &self.instances[pub_id.0].instance_ref().unwrap();
        let attachment = instance.attachment().map(|attachment| {
            self.instances[attachment.0]
                .instance_ref()
                .unwrap_or_else(|| {
                    panic!("public {:?} attachment {:?} not public", pub_id, attachment)
                })
        });

        CardInfo {
            instance,
            owner,
            zone: location.0,
            attachment,
        }
    }

    pub async fn reveal_from_cards_fold<T, B, F, G>(
        &mut self,
        cards: Vec<Card>,
        map: G,
        init: B,
        fold: F,
    ) -> B
    where
        T: Clone + 'static,
        B: Secret + 'static,
        F: Fn(B, &T) -> B + Clone + 'static,
        G: Fn(CardInfo<S>) -> T + Clone + 'static,
    {
        let (public_cards, secret_cards) = {
            let mut public_cards = vec![];
            let mut secret_cards: [Vec<Card>; 2] = [vec![], vec![]];
            for card in cards {
                match card {
                    Card::ID(id) => match &self.instances[id.0] {
                        InstanceOrPlayer::Instance(_) => public_cards.push(id),
                        InstanceOrPlayer::Player(owner) => secret_cards[*owner as usize].push(card),
                    },
                    Card::Pointer(OpaquePointer { player, .. }) => {
                        secret_cards[player as usize].push(card)
                    }
                }
            }
            (public_cards, secret_cards)
        };

        let [p0_cards, p1_cards] = secret_cards;

        let every_single_public_card: indexmap::IndexMap<InstanceID, T> = self
            .instances
            .iter()
            .filter_map(|instance| {
                instance.instance_ref().map(|i| {
                    let id = i.id();
                    let info = self.card_info(id);
                    (id, map(info))
                })
            })
            .collect();

        let mut accumulated = public_cards.into_iter().fold(init, |prev, pub_id| {
            fold(
                prev,
                every_single_public_card
                    .get(&pub_id)
                    .expect("public cards are public"),
            )
        });

        for (player, cards) in vec![p0_cards, p1_cards].into_iter().enumerate() {
            if cards.is_empty() {
                continue;
            }
            let map = map.clone();
            let fold = fold.clone();
            let every_single_public_card = every_single_public_card.clone();
            accumulated = self
                .context
                .reveal_unique(
                    player as u8,
                    move |secret| {
                        let map = map.clone();
                        let fold = fold.clone();
                        let cards = cards.clone();
                        let every_single_public_card = every_single_public_card.clone();
                        cards
                            .into_iter()
                            .fold(accumulated.clone(), move |prev, card| {
                                let id = secret.id(card).unwrap();
                                if secret.instances.get(&id).is_some() {
                                    let map = map.clone();
                                    fold(
                                        prev,
                                        &secret
                                            .reveal_from_card(card, move |c| map(c))
                                            .unwrap_or_else(|| {
                                                panic!(
                                                    "{:?} not in player {:?} secret",
                                                    card, player
                                                )
                                            }),
                                    )
                                } else {
                                    // card is in public state
                                    fold(
                                        prev,
                                        every_single_public_card.get(&id).expect(
                                            "Card must be in public, since it's not in secret.",
                                        ),
                                    )
                                }
                            })
                    },
                    |_| true,
                )
                .await;
        }

        accumulated
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
        // todo!(): betterize this implementation

        let mut parents = Vec::with_capacity(cards.len());

        for card in cards {
            parents.push(self.reveal_parent(card).await);
        }

        parents
    }

    pub async fn filter_cards(
        &mut self,
        cards: Vec<Card>,
        f: impl Fn(CardInfo<S>) -> bool + Clone + 'static,
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

        let card = if let Card::Pointer(OpaquePointer { player, index }) = card {
            self.context
                .reveal_unique(
                    player,
                    move |secret| {
                        let id = secret.pointers[index];

                        if secret.instances.contains_key(&id) {
                            card
                        } else {
                            id.into()
                        }
                    },
                    |_| true,
                )
                .await
        } else {
            card
        };

        match card {
            Card::ID(id) => match &self.instances[id.0] {
                InstanceOrPlayer::Instance(instance) => {
                    // public ID to public instance

                    let owner = self.location(id).player;

                    let attachment = instance.attachment().map(|attachment| {
                        self.instances[attachment.0]
                            .instance_ref()
                            .unwrap_or_else(|| {
                                panic!("public {:?} attachment {:?} not public", id, attachment)
                            })
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
                                .unwrap_or_else(|_| {
                                    panic!(
                                        "unable to move attachment {:?} to public dust",
                                        attachment
                                    )
                                });
                        }
                        (None, Some(default)) => {
                            // attach base attachment

                            // create attach state using new_card_state from implementor
                            let new_state = default.new_card_state(Some(&instance.state));
                            let attachment = self.new_card(owner, default).await;

                            self.modify_card(attachment, move |mut c| {
                                c.state = new_state.clone();
                            })
                            .await;

                            self.move_card(
                                attachment,
                                owner,
                                Zone::Attachment { parent: id.into() },
                            )
                            .await
                            .unwrap_or_else(|_| {
                                panic!("unable to attach public limbo {:?} to {:?}", attachment, id)
                            });
                        }
                        (Some(current), Some(default)) if *current == default => {
                            // reset current attachment

                            let attachment = attachment
                                .expect("attachment base exists, but no attachment")
                                .id();

                            let new_state = default.new_card_state(Some(&instance.state));

                            self.modify_card(attachment, move |mut c| {
                                c.state = new_state.clone();
                            })
                            .await;
                        }
                        (Some(..), Some(default)) => {
                            // attach base attachment, will implicitly dust current attachment.
                            let new_state = default.new_card_state(Some(&instance.state));

                            let attachment = self.new_card(owner, default).await;
                            // create attach state using new_card_state from implementor

                            self.modify_card(attachment, move |mut c| {
                                c.state = new_state.clone();
                            })
                            .await;

                            self.move_card(
                                attachment,
                                owner,
                                Zone::Attachment { parent: id.into() },
                            )
                            .await
                            .unwrap_or_else(|_| {
                                panic!("unable to attach public limbo {:?} to {:?}", attachment, id)
                            });
                        }
                    }
                    self.modify_card(id, |mut c| {
                        c.state = c.base.reset_card(&c.state);
                        if let Some(attach) = c.attachment {
                            S::on_attach(&mut *c, &attach);
                        }
                    })
                    .await;
                }
                InstanceOrPlayer::Player(owner) => {
                    // public ID to secret instance

                    let owner = *owner;

                    self.new_secret_cards(owner, |mut secret| {
                        let instance = secret
                            .instance(id)
                            .unwrap_or_else(|| panic!("player {} secret {:?} not in secret", owner, id));

                        let attachment = instance.attachment().map(|attachment| {
                            secret.instance(attachment).unwrap_or_else(|| panic!("player {} secret {:?} attachment {:?} not secret", owner, id, attachment))
                        });


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
                                    let new_attach = secret.new_card(default);
                                    secret.attach_card(id, new_attach).expect("Unable to secretly attach a secret card to another card in the same secret.");
                                }
                                (Some(current), Some(default)) if current.base == default => {
                                    // reset current attachment
                                    let attachment_base_state = current.base.new_card_state(Some(&instance.state));
                                    let current_id = current.id();
                                    secret.instance_mut(current_id).unwrap().state = attachment_base_state;
                            }
                                (Some(_), Some(default)) => {
                                     // attach base attachment, current attachment is implicitly dusted.
                                     let new_attach = secret.new_card(default);
                                     secret.attach_card(id, new_attach).expect("Unable to secretly attach a secret card to another card in the same secret.");
                                }
                            }



                        secret.modify_card(id, |mut c| {
                            c.state = c.base.reset_card(&c.state);
                            if let Some(attach) = c.attachment {
                                S::on_attach(&mut *c, &attach);
                            }
                        }).expect("Failed to reset card in this secret.");
                    }).await;
                }
            },
            Card::Pointer(OpaquePointer { player, index }) => {
                self.new_secret_cards(player, |mut secret| {
                    let id = secret.pointers[index];

                    let instance = secret
                        .instance(id)
                        .unwrap_or_else(|| panic!("player {} secret {:?} not in secret", player, id));

                    let attachment = instance.attachment().map(|attachment| {
                        secret.instance(attachment).unwrap_or_else(|| panic!("player {} secret {:?} attachment {:?} not secret", player, id, attachment))
                    });

                    let next_instance = secret.next_instance.expect(
                        "`PlayerSecret::next_instance` missing during `CardGame::new_secret_cards` call",
                    );

                    match (attachment, instance.base.attachment()) {
                        (None, None) => {
                            // do nothing
                        }
                        (Some(current), None) => {
                            // dust current attachment

                            let current_id = current.id();

                            secret
                                .dust_card(current_id)
                                .expect("current_id is in this secret, and is not already dust.");
                        }
                        (None, Some(default)) => {
                            // attach base attachment

                            let state = default.new_card_state(Some(&instance.state));

                            let attachment = CardInstance {
                                id: next_instance,
                                base: default,
                                attachment: None,
                                state,
                            };

                            secret.instances.insert(next_instance, attachment);

                            secret
                                .attach_card(id, next_instance)
                                .expect("Both id and next_instance are in this secret.");
                        }
                        (Some(current), Some(default)) if current.base == default => {
                            // reset current attachment
                            let attachment_base_state = current.base.new_card_state(Some(&instance.state));
                            let current_id = current.id();
                            secret.instance_mut(current_id).unwrap().state = attachment_base_state;
                        }
                        (Some(current), Some(default)) => {
                            // dust current attachment
                            let state = default.new_card_state(Some(&instance.state));
                            let current_id = current.id();
                            secret
                                .dust_card(current_id)
                                .expect("current_id is in this secret, and is not already dust.");

                            // Attach base attachment

                            let attachment = CardInstance {
                                id: next_instance,
                                base: default,
                                attachment: None,
                                state,
                            };

                            secret.instances.insert(next_instance, attachment);

                            secret
                                .attach_card(id, next_instance)
                                .expect("Both id and next_instance are in this secret.");
                        }
                    }

                    // unconditionally increment instance ID to avoid leaking attachment information

                    secret
                        .next_instance
                        .as_mut()
                        .expect(
                            "`PlayerSecret::next_instance` missing during `CardGame::new_secret_cards` call",
                        )
                        .0 += 1;

                    let attachment = secret
                        .instance(id).unwrap().attachment().map(|attachment| {
                            secret.instance(attachment).unwrap_or_else(|| panic!("player {} secret {:?} attachment {:?} not secret", player, id, attachment)).clone()
                        });

                    let instance = secret
                        .instance_mut(id)
                        .expect("immutable instance exists, but no mutable instance");

                    instance.state = instance.base.reset_card(&instance.state);
                    if let Some(attach) = attachment {
                        S::on_attach(instance, &attach);
                    }
                })
                .await;
            }
        }
    }

    pub async fn reset_cards(&mut self, cards: Vec<Card>) {
        // todo!(): betterize this implementation

        for card in cards {
            self.reset_card(card).await;
        }
    }

    /// Copies a card.
    ///
    /// If the card is a public ID to player X's public instance, the card is copied to player X's public limbo.
    /// If the card is a public ID to player X's secret instance, the card is copied to player X's secret limbo.
    /// If the card is player X's secret pointer to a public instance, the card is copied to player X's secret limbo.
    /// If the card is player X's secret pointer to player X's secret instance, the card is copied to player X's secret limbo.
    /// If the card is player X's secret pointer to player Y's secret instance, the card is copied to player Y's secret limbo.
    pub fn copy_card<'a>(
        &'a mut self,
        card: impl Into<Card>,
        deep: bool,
    ) -> Pin<Box<dyn Future<Output = Card> + 'a>> {
        return inner(self, card.into(), deep);

        fn inner<'a, S: State>(
            this: &'a mut CardGame<S>,
            card: Card,
            deep: bool,
        ) -> Pin<Box<dyn Future<Output = Card> + 'a>> {
            Box::pin(async move {
                match card {
                    Card::ID(id) => match &this.instances[id.0] {
                        InstanceOrPlayer::Instance(instance) => {
                            let owner = this.owner(id);
                            let base = instance.base.clone();
                            let state = instance.state.copy_card();
                            let attachment = if deep {
                                if let Some(attachment) = instance.attachment {
                                    Some(
                                        this.copy_card(attachment, deep)
                                            .await
                                            .id()
                                            .expect("public card attachment copy must be public"),
                                    )
                                } else {
                                    None
                                }
                            } else if let Some(attachment) = base.attachment() {
                                Some(this.new_card(owner, attachment).await)
                            } else {
                                None
                            };

                            let copy_id = InstanceID(this.instances.len());
                            let copy = CardInstance {
                                id: copy_id,
                                base,
                                state,
                                attachment: None,
                            };
                            this.instances.push(InstanceOrPlayer::Instance(copy));

                            this.player_cards_mut(owner).limbo.push(copy_id);

                            if let Some(attachment) = attachment {
                                this.move_card(
                                    attachment,
                                    owner,
                                    Zone::Attachment {
                                        parent: copy_id.into(),
                                    },
                                )
                                .await
                                .unwrap();
                            }
                            copy_id.into()
                        }
                        InstanceOrPlayer::Player(owner) => {
                            let owner = *owner;
                            this.new_secret_cards(owner, |mut secret| {
                            let (copy_id, attach_id) = {
                                let mut next_instance = secret.next_instance.expect(
                                    "`PlayerSecret::next_instance` missing during `CardGame::new_secret_cards` call",
                                );
                                let attach_id = next_instance;
                                next_instance.0 += 1;
                                let copy_id = next_instance;
                                next_instance.0 += 1;
                                secret.next_instance = Some(next_instance);
                                (copy_id, attach_id)
                            };
                            let instance = &secret.instances[&id];

                            let base = instance.base.clone();
                            let state = instance.state.copy_card();

                            let attachment = if deep {
                                if let Some(attachment) = instance.attachment {
                                    let old_attach = &secret.instances[&attachment];
                                    assert!(old_attach.attachment.is_none(), "Attachments can't have attachments.");
                                    let attachment = CardInstance {
                                        id: attach_id,
                                        base: old_attach.base().clone(),
                                        attachment: None,
                                        state: old_attach.state.copy_card(),
                                    };

                                    secret.instances.insert(attach_id, attachment);
                                    secret.limbo.push(attach_id);
                                    Some(attach_id)
                                } else {
                                    None
                                }
                            } else if let Some(attach_base) = base.attachment() {
                                let attachment = CardInstance {
                                    id: attach_id,
                                    base: attach_base.clone(),
                                    attachment: None,
                                    state: attach_base.new_card_state(Some(&state)),
                                };

                                secret.instances.insert(attach_id, attachment);
                                secret.limbo.push(attach_id);
                                Some(attach_id)
                            } else {
                                None
                            };
                            let copy = CardInstance {
                                id: copy_id,
                                base,
                                state,
                                attachment: None
                            };
                            secret.instances.insert(copy_id, copy);
                            secret.limbo.push(copy_id);
                            secret.pointers.push(copy_id);
                            if let Some(attachment) = attachment {
                                secret.attach_card(copy_id, attachment).unwrap();
                            }
                        })
                        .await[0]
                        }
                    },
                    Card::Pointer(OpaquePointer { player, index }) => {
                        let buckets: Vec<_> = this
                            .instances
                            .iter()
                            .map(InstanceOrPlayer::player)
                            .collect();

                        let id = this
                            .context
                            .reveal_unique(
                                player,
                                move |secret| {
                                    let id = secret.pointers[index];

                                    buckets[id.0].and_then(|bucket| {
                                        if bucket == player {
                                            None
                                        } else {
                                            Some(id)
                                        }
                                    })
                                },
                                |_| true,
                            )
                            .await;

                        match id {
                            None => {
                                let instances = this.instances.clone();

                                this.new_secret_cards(player, |mut secret| {
                                let (copy_id, attach_id) = {
                                    let mut next_instance = secret
                                        .next_instance
                                        .expect("`PlayerSecret::next_instance` missing during `CardGame::new_secret_cards` call");

                                    let copy_id = next_instance;
                                    next_instance.0 += 1;
                                    let attach_id = next_instance;
                                    next_instance.0 += 1;
                                    secret.next_instance = Some(next_instance);
                                    (copy_id, attach_id)
                                };

                                let id = secret.pointers[index];

                                let instance = instances[id.0].instance_ref().or_else(|| secret.instances.get(&id)).expect("instance is neither public nor in this secret");

                                let base = instance.base.clone();
                                let state = instance.state.copy_card();

                                let attachment = if deep {
                                    if let Some(attachment) = instance.attachment {
                                let old_attach = instances[attachment.0].instance_ref().or_else(|| secret.instance(attachment)).unwrap();
                                        assert!(old_attach.attachment.is_none());
                                        let attachment = CardInstance {
                                            id: attach_id,
                                            base: old_attach.base().clone(),
                                            attachment: None,
                                            state: old_attach.state.copy_card(),
                                        };

                                        secret.instances.insert(attach_id, attachment);
                                        secret.limbo.push(attach_id);
                                        Some(attach_id)
                                    } else {
                                        None
                                    }
                                } else if let Some(attach_base) = base.attachment() {
                                    let attachment = CardInstance {
                                        id: attach_id,
                                        base: attach_base.clone(),
                                        attachment: None,
                                        state: attach_base.new_card_state(Some(&state)),
                                    };

                                    secret.instances.insert(attach_id, attachment);
                                    secret.limbo.push(attach_id);
                                    Some(attach_id)
                                } else {
                                    None
                                };
                                let copy = CardInstance {
                                    id: copy_id,
                                    base,
                                    state,
                                    attachment: None
                                };
                                secret.instances.insert(copy_id, copy);
                                secret.limbo.push(copy_id);
                                secret.pointers.push(copy_id);
                                if let Some(attachment) = attachment {
                                    secret.attach_card(copy_id, attachment).unwrap();
                                }
                            }).await[0]
                            }
                            Some(id) => {
                                let owner = this.instances[id.0]
                                    .player()
                                    .expect("instance is not in another player's secret");

                                this.new_secret_cards(owner, |mut secret| {
                                let (copy_id, attach_id) = {
                                        let mut next_instance = secret
                                            .next_instance
                                        .expect("`PlayerSecret::next_instance` missing during `CardGame::new_secret_cards` call");

                                        let copy_id = next_instance;
                                        next_instance.0 += 1;
                                        let attach_id = next_instance;
                                        next_instance.0 += 1;
                                        secret.next_instance = Some(next_instance);
                                        (copy_id, attach_id)
                                    };

                                    let instance = &secret.instances[&id];
                                    let base = instance.base.clone();
                                    let state = instance.state.copy_card();

                                    let attachment = if deep {
                                        if let Some(attachment) = instance.attachment {
                                            let old_attach = &secret.instances[&attachment];
                                            assert!(old_attach.attachment.is_none());
                                            let attachment = CardInstance {
                                                id: attach_id,
                                                base: old_attach.base().clone(),
                                                attachment: None,
                                                state: old_attach.state.copy_card(),
                                            };

                                            secret.instances.insert(attach_id, attachment);
                                            secret.limbo.push(attach_id);
                                            Some(attach_id)
                                        } else {
                                            None
                                        }
                                    } else if let Some(attach_base) = base.attachment() {
                                        let attachment = CardInstance {
                                            id: attach_id,
                                            base: attach_base.clone(),
                                            attachment: None,
                                            state: attach_base.new_card_state(Some(&state)),
                                        };

                                        secret.instances.insert(attach_id, attachment);
                                        secret.limbo.push(attach_id);
                                        Some(attach_id)
                                    } else {
                                        None
                                    };

                                    let copy = CardInstance {
                                        id: copy_id,
                                        base,
                                        state,
                                        attachment: None
                                    };
                                    secret.instances.insert(copy_id, copy);
                                    secret.limbo.push(copy_id);
                                    secret.pointers.push(copy_id);
                                    if let Some(attachment) = attachment {
                                        secret.attach_card(copy_id, attachment).unwrap();
                                        assert!(secret.instance(copy_id).unwrap().attachment.is_some());
                                    }
                                }).await[0]
                            }
                        }
                    }
                }
            })
        }
    }

    pub async fn copy_cards(&mut self, cards: Vec<Card>, deep: bool) -> Vec<Card> {
        // todo!(): betterize this implementation

        let mut copies = Vec::new();

        for card in cards {
            copies.push(self.copy_card(card, deep).await);
        }

        copies
    }

    /// Always returns a Card::ID if the card is in public state.
    pub async fn modify_card(&mut self, card: impl Into<Card>, f: impl Fn(CardInfoMut<S>)) -> Card {
        let card = card.into();

        let card = if let Card::Pointer(OpaquePointer { player, index }) = card {
            self.context
                .reveal_unique(
                    player,
                    move |secret| {
                        let id = secret.pointers[index];

                        if secret.instances.contains_key(&id) {
                            card
                        } else {
                            id.into()
                        }
                    },
                    |_| true,
                )
                .await
        } else {
            card
        };

        match card {
            Card::ID(id) => {
                let Self { state, context } = self;

                match &state.instances[id.0] {
                    InstanceOrPlayer::Instance(instance) => {
                        let CardLocation {
                            player: owner,
                            location,
                        } = state.location(id);
                        let location =
                            location.unwrap_or_else(|| panic!("public {:?} has no zone", id));

                        let attachment = instance.attachment.map(|attachment| {
                            state.instances[attachment.0]
                                .instance_ref()
                                .unwrap_or_else(|| {
                                    panic!("public {:?} attachment {:?} not public", id, attachment)
                                })
                                .clone()
                        });

                        let instance = state.instances[id.0]
                            .instance_mut()
                            .unwrap_or_else(|| panic!("{:?} vanished", id));

                        let before = instance.clone();

                        f(CardInfoMut {
                            instance,
                            owner,
                            zone: location.0,
                            attachment: attachment.as_ref(),
                            log: &mut |event| context.log(event),
                        });

                        let after = state.instances[id.0]
                            .instance_ref()
                            .unwrap_or_else(|| panic!("{:?} vanished", id));

                        if !before.eq(after) {
                            context.log(CardEvent::ModifyCard {
                                instance: after.clone(),
                            })
                        }

                        let mut logs = vec![];

                        match location.0 {
                            Zone::Field => self.sort_field(
                                owner,
                                self.player_cards(owner).field.clone(),
                                true,
                                &mut |event| logs.push(event),
                            ),
                            Zone::Attachment {
                                parent: Card::ID(parent_id),
                            } => {
                                if let Some((Zone::Field, ..)) = self.location(parent_id).location {
                                    self.sort_field(
                                        owner,
                                        self.player_cards(owner).field.clone(),
                                        true,
                                        &mut |event| logs.push(event),
                                    );
                                }
                            }
                            _ => (),
                        }

                        for event in logs.into_iter() {
                            self.context.log(event);
                        }
                    }
                    InstanceOrPlayer::Player(owner) => {
                        self.context.mutate_secret(*owner, |secret| {
                            secret
                                .secret
                                .modify_card(card, secret.log, |instance| f(instance))
                                .unwrap_or_else(|_| {
                                    panic!("player {} secret {:?} not in secret", owner, card)
                                });
                        });
                    }
                }
            }
            Card::Pointer(OpaquePointer { player, .. }) => {
                self.context.mutate_secret(player, |secret| {
                    secret
                        .secret
                        .modify_card(card, secret.log, |instance| f(instance))
                        .unwrap_or_else(|_| {
                            panic!("player {} secret {:?} not in secret", player, card)
                        });
                });
            }
        }

        card
    }

    /// Internal API only.
    /// Modifies a card and logs any changes.
    /// TODO it would be nice to eliminiate all the duplication between here and `CardGame::modify_card`
    pub(crate) async fn modify_card_internal(
        &mut self,
        card: Card,
        f: impl Fn(
            &mut CardInstance<S>,
            &mut dyn FnMut(<GameState<S> as arcadeum::store::State>::Event),
        ),
        logger: &mut dyn FnMut(<GameState<S> as arcadeum::store::State>::Event),
    ) {
        let card = if let Card::Pointer(OpaquePointer { player, index }) = card {
            self.context
                .reveal_unique(
                    player,
                    move |secret| {
                        let id = secret.pointers[index];

                        if secret.instances.contains_key(&id) {
                            card
                        } else {
                            id.into()
                        }
                    },
                    |_| true,
                )
                .await
        } else {
            card
        };

        match card {
            Card::ID(id) => {
                let Self { state, .. } = self;

                match &state.instances[id.0] {
                    InstanceOrPlayer::Instance(_) => {
                        let CardLocation {
                            player: owner,
                            location,
                        } = state.location(id);
                        let location =
                            location.unwrap_or_else(|| panic!("public {:?} has no zone", id));

                        let instance = state.instances[id.0]
                            .instance_mut()
                            .unwrap_or_else(|| panic!("{:?} vanished", id));

                        let before = instance.clone();

                        f(instance, &mut |event| logger(event));

                        let instance = &*instance; // lose mutable ref
                        if !before.eq(instance) {
                            logger(CardEvent::ModifyCard {
                                instance: instance.clone(),
                            })
                        }
                        match location.0 {
                            Zone::Field => self.sort_field(
                                owner,
                                self.player_cards(owner).field.clone(),
                                true,
                                logger,
                            ),
                            Zone::Attachment {
                                parent: Card::ID(parent_id),
                            } => {
                                if let Some((Zone::Field, ..)) = self.location(parent_id).location {
                                    self.sort_field(
                                        owner,
                                        self.player_cards(owner).field.clone(),
                                        true,
                                        logger,
                                    );
                                }
                            }
                            _ => (),
                        }
                    }
                    InstanceOrPlayer::Player(owner) => {
                        self.context.mutate_secret(*owner, |secret| {
                            secret.secret.modify_card_internal(
                                card,
                                secret.log,
                                |instance, log| f(instance, log),
                            );
                        });
                    }
                }
            }
            Card::Pointer(OpaquePointer { player, .. }) => {
                self.context.mutate_secret(player, |secret| {
                    secret
                        .secret
                        .modify_card_internal(card, secret.log, |instance, log| f(instance, log));
                });
            }
        }
    }

    pub async fn modify_cards(&mut self, cards: Vec<Card>, f: impl Fn(CardInfoMut<S>)) {
        // todo!(): betterize this implementation

        for card in cards {
            self.modify_card(card, &f).await;
        }
    }
    pub async fn change_base_card(&mut self, id: InstanceID, new_base: S::BaseCard) {
        self.modify_card(id, |mut c| {
            c.base = new_base.clone();
        })
        .await;
    }
    pub async fn move_card(
        &mut self,
        card: impl Into<Card>,
        to_player: Player,
        to_zone: Zone,
    ) -> Result<(CardLocation, Option<InstanceID>), error::MoveCardError> {
        return inner(self, card.into(), to_player, to_zone).await;

        async fn inner<S: State>(
            this: &mut CardGame<S>,
            card: Card,
            to_player: Player,
            to_zone: Zone,
        ) -> Result<(CardLocation, Option<InstanceID>), error::MoveCardError> {
            let old_field = if to_zone.is_field() {
                Some(this.player_cards(to_player).field.clone())
            } else {
                None
            };
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
                    return this.attach_card(card, parent).await;
                }
            };
            // We always need to know who owns the card instance itself.

            // Either this card is in Public state (None) or a player's secret (Some(player)).
            // We also need to know who owns the card, regardless of its secrecy, so we can later update the public state for that player.
            let (bucket, owner) = match card {
                Card::Pointer(OpaquePointer { player, index }) => {
                    let buckets: Vec<_> = this
                        .instances
                        .iter()
                        .enumerate()
                        .map(|(i, instance)| (instance.player(), this.owner(InstanceID(i))))
                        .collect();

                    this.context
                        .reveal_unique(
                            player,
                            move |secret| buckets[secret.pointers[index].0],
                            |_| true,
                        )
                        .await
                }
                Card::ID(id) => (this.instances[id.0].player(), this.owner(id)),
            };

            let id = match card {
                Card::ID(id) => Some(id),
                Card::Pointer(OpaquePointer { player, index }) => {
                    if bucket != Some(player) || to_bucket != Some(player) {
                        Some(
                            this.context
                                .reveal_unique(
                                    player,
                                    move |secret| secret.pointers[index],
                                    |_| true,
                                )
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
                        this.location(id)
                            .location
                            .expect("CardLocation for a public card must be public."),
                    )
                }
                Some(player) => {
                    this.context.mutate_secret(player, move |mut secret| {
                        let location =
                            secret
                                .location(id.unwrap_or_else(|| {
                                    secret.pointers[card.pointer().unwrap().index]
                                }))
                                .location
                                .expect("The secret should know the zone.");

                        match location.0 {
                            Zone::Limbo { public: false } | Zone::Attachment { .. } => {
                                secret.deferred_locations.push(location);
                            }
                            _ => {
                                // location will get revealed publicly
                            }
                        }
                    });

                    this.context
                        .reveal_unique(
                            player,
                            move |secret| {
                                let location = secret
                                    .location(id.unwrap_or_else(|| {
                                        secret.pointers[card.pointer().unwrap().index]
                                    }))
                                    .location
                                    .expect("The secret should know the zone.");

                                match location.0 {
                                    Zone::Limbo { public: false } | Zone::Attachment { .. } => {
                                        None // secret.deferred_locations has this key.
                                    }
                                    _ => Some(location),
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
                    if let Some((zone, index)) = location {
                        this.player_cards_mut(bucket_owner).remove_from(zone, index);
                    }

                    // Update the public state about where we put this card
                    let player_state = this.player_cards_mut(to_player);
                    match to_zone {
                        Zone::Deck => {
                            player_state.deck += 1;
                        }
                        Zone::Hand { public: false } => {
                            player_state.hand.push(None);
                        }
                        Zone::Hand { public: true } => {
                            unreachable!("{}:{}:{}", file!(), line!(), column!());
                        }
                        Zone::Field => {
                            unreachable!("{}:{}:{}", file!(), line!(), column!());
                        }
                        Zone::Graveyard => {
                            unreachable!("{}:{}:{}", file!(), line!(), column!());
                        }
                        Zone::Limbo { public: false } => {
                            // do nothing, this is a secret
                        }
                        Zone::Limbo { public: true } => {
                            unreachable!("{}:{}:{}", file!(), line!(), column!());
                        }
                        Zone::CardSelection => {
                            player_state.card_selection += 1;
                        }
                        Zone::Casting => {
                            unreachable!("{}:{}:{}", file!(), line!(), column!());
                        }
                        Zone::Dust { public: false } => {
                            // do nothing, this is a secret
                        }
                        Zone::Dust { public: true } => {
                            unreachable!("{}:{}:{}", file!(), line!(), column!());
                        }
                        Zone::Attachment { .. } => {
                            unreachable!("Cannot move card to attachment zone")
                        }
                    }

                    let to_location = (
                        to_zone,
                        match to_zone {
                            Zone::Deck => this.player_cards(to_player).deck() - 1,
                            Zone::Hand { public: false } => {
                                this.player_cards(to_player).hand().len() - 1
                            }
                            Zone::Hand { public: true } => {
                                unreachable!("{}:{}:{}", file!(), line!(), column!())
                            }
                            Zone::Field => unreachable!("{}:{}:{}", file!(), line!(), column!()),
                            Zone::Graveyard => {
                                unreachable!("{}:{}:{}", file!(), line!(), column!())
                            }
                            Zone::Limbo { public: false } => 0,
                            Zone::Limbo { public: true } => {
                                unreachable!("{}:{}:{}", file!(), line!(), column!())
                            }
                            Zone::CardSelection => 0,
                            Zone::Casting => unreachable!("{}:{}:{}", file!(), line!(), column!()),
                            Zone::Dust { public: false } => 0,
                            Zone::Dust { public: true } => {
                                unreachable!("{}:{}:{}", file!(), line!(), column!())
                            }
                            Zone::Attachment { .. } => 0,
                        },
                    );
                    this.context.mutate_secret_or_log(
                        bucket_owner,
                        |mut secret| {
                            let id = id
                                .unwrap_or_else(|| secret.pointers[card.pointer().unwrap().index]);
                            let old_location = secret.location(id);

                            let instance = secret.instance(id).unwrap().clone();
                            let attachment = instance
                                .attachment
                                .map(|a_id| secret.instance(a_id).unwrap().clone());

                            // Remove this card from its old zone in the secret.
                            secret.secret.remove_id(secret.log, id);

                            secret.log(CardEvent::MoveCard {
                                instance: Some((instance, attachment)),
                                from: old_location,
                                to: ExactCardLocation {
                                    player: bucket_owner,
                                    location: (
                                        to_zone,
                                        match to_zone {
                                            Zone::Deck => secret.deck.len(),
                                            Zone::Hand { public: false } => secret.hand.len(),
                                            Zone::Hand { public: true } => {
                                                unreachable!(
                                                    "{}:{}:{}",
                                                    file!(),
                                                    line!(),
                                                    column!()
                                                )
                                            }
                                            Zone::Field => {
                                                unreachable!(
                                                    "{}:{}:{}",
                                                    file!(),
                                                    line!(),
                                                    column!()
                                                )
                                            }
                                            Zone::Graveyard => {
                                                unreachable!(
                                                    "{}:{}:{}",
                                                    file!(),
                                                    line!(),
                                                    column!()
                                                )
                                            }
                                            Zone::Limbo { public: false } => secret.limbo.len(),
                                            Zone::Limbo { public: true } => {
                                                unreachable!(
                                                    "{}:{}:{}",
                                                    file!(),
                                                    line!(),
                                                    column!()
                                                )
                                            }
                                            Zone::CardSelection => secret.card_selection.len(),
                                            Zone::Casting => {
                                                unreachable!(
                                                    "{}:{}:{}",
                                                    file!(),
                                                    line!(),
                                                    column!()
                                                )
                                            }
                                            Zone::Dust { public: false } => secret.dust.len(),
                                            Zone::Dust { public: true } => {
                                                unreachable!(
                                                    "{}:{}:{}",
                                                    file!(),
                                                    line!(),
                                                    column!()
                                                )
                                            }
                                            Zone::Attachment { .. } => 0,
                                        },
                                    ),
                                },
                            });

                            // Put the card in its new zone in the secret.
                            match to_zone {
                                Zone::Deck => secret.deck.push(id),
                                Zone::Hand { public: false } => secret.hand.push(Some(id)),
                                Zone::Hand { public: true } => {
                                    unreachable!("{}:{}:{}", file!(), line!(), column!())
                                }
                                Zone::Field => {
                                    unreachable!("{}:{}:{}", file!(), line!(), column!())
                                }
                                Zone::Graveyard => {
                                    unreachable!("{}:{}:{}", file!(), line!(), column!())
                                }
                                Zone::Limbo { public: false } => secret.limbo.push(id),
                                Zone::Limbo { public: true } => {
                                    unreachable!("{}:{}:{}", file!(), line!(), column!())
                                }
                                Zone::CardSelection => secret.card_selection.push(id),
                                Zone::Casting => {
                                    unreachable!("{}:{}:{}", file!(), line!(), column!())
                                }
                                Zone::Dust { public: false } => secret.dust.push(id),
                                Zone::Dust { public: true } => {
                                    unreachable!("{}:{}:{}", file!(), line!(), column!())
                                }
                                Zone::Attachment { .. } => {
                                    unreachable!("Can't attach a spell with move_card.")
                                }
                            }
                        },
                        CardEvent::MoveCard {
                            instance: None,
                            from: CardLocation {
                                player: bucket_owner,
                                location,
                            },
                            to: ExactCardLocation {
                                player: to_player,
                                location: to_location,
                            },
                        },
                    );

                    return Ok((
                        CardLocation {
                            player: bucket_owner,
                            location,
                        },
                        id,
                    ));
                }
            }

            let mut deferred_logs = vec![];

            let (instance, attachment_instance) = match bucket {
                None => {
                    let id = id.expect("Card is in public state, but we don't know its id.");
                    if let Some((
                        Zone::Attachment {
                            parent: Card::ID(old_parent),
                        },
                        ..,
                    )) = location
                    {
                        let attach_clone = id
                            .instance(this, None)
                            .expect("Match is in None, so this id must be in public state.")
                            .clone();
                        this.modify_card_internal(
                            old_parent.into(),
                            |parent, _| {
                                parent.attachment = None;
                                S::on_detach(parent, &attach_clone);
                            },
                            &mut |event| deferred_logs.push(event),
                        )
                        .await;
                    }

                    if let Some(to_bucket_player) = to_bucket {
                        let instance = std::mem::replace(
                        &mut this.instances[id.0],
                        InstanceOrPlayer::Player(to_bucket_player),
                    )
                    .instance()
                    .expect(
                        "Card was identified as public, but it's actually InstanceOrPlayer::Player",
                    );

                        let attachment = instance.attachment.map(|attachment_id| {
                        std::mem::replace(&mut this.instances[attachment_id.0], InstanceOrPlayer::Player(to_bucket_player)).instance().expect("Since parent Card is public, attachment was identified as public, but it's actually InstanceOrPlayer::Player")
                    });

                        this.context.mutate_secret(owner, |mut secret| {
                            if let Some((Zone::Hand { public: false }, index)) = location {
                                secret
                                    .hand
                                    .remove(index.expect("no index for secret hand card"));
                            }
                        });

                        (Some(instance), attachment)
                    } else {
                        // we're moving from public to public
                        (None, None)
                    }
                }
                Some(player) => {
                    let (instance, attachment_instance) = this
                        .context
                        .reveal_unique(
                            player,
                            move |secret| {
                                let id = id.unwrap_or_else(|| {
                                    secret.pointers[card.pointer().unwrap().index]
                                });

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

                    this.context.mutate_secret(player, move |mut secret| {
                        let id =
                            id.unwrap_or_else(|| secret.pointers[card.pointer().unwrap().index]);
                        // find what collection id is in and remove it
                        secret.deck.retain(|i| *i != id);
                        secret.hand.retain(|i| *i != Some(id));
                        secret.limbo.retain(|i| *i != id);
                        secret.card_selection.retain(|i| *i != id);
                        secret.dust.retain(|i| *i != id);

                        let parent_id = secret
                            .instances
                            .values()
                            .find(|c| c.attachment == Some(id))
                            .map(|c| c.id);
                        // We're removing the attachment from a card in the secret
                        if let Some(parent_id) = parent_id {
                            let attach_clone = secret.instance(id).unwrap().clone();
                            let mut deferred_logs = vec![];
                            secret.modify_card_internal(
                                parent_id,
                                &mut |event| deferred_logs.push(event),
                                |parent, _| {
                                    parent.attachment = None;
                                    S::on_detach(parent, &attach_clone);
                                },
                            );
                            secret.deferred_logs.append(&mut deferred_logs);
                        }
                        // We're removing a card with an attachment from the secret
                        if let Some(attachment_id) = secret.instance(id).unwrap().attachment {
                            secret.instances.remove(&attachment_id);
                        }

                        // Finally, remove the card from the secret's instances.
                        secret.instances.remove(&id);
                    });
                    (instance, attachment_instance)
                }
            };

            // At this point in time, either we already knew ID, or we've revealed it by revealing the instance.
            let id = id
                .or_else(|| instance.as_ref().map(|v| v.id))
                .expect("Either we know ID or we've revealed the instance.");

            // If this card came from a secret, we know it's leaving that secret. SX -> SX case handled above.
            if let Some(bucket_owner) = bucket {
                this.context.mutate_secret(bucket_owner, |secret| {
                    // Take its ID out of any zones in that secret.
                    secret.secret.remove_id(secret.log, id);
                });
            } else if let Some((Zone::Hand { public: true }, index)) = location {
                this.context.mutate_secret(owner, |mut secret| {
                    secret
                        .hand
                        .remove(index.expect("no index for public hand card"));
                });
            }

            match location {
                Some((
                    Zone::Attachment {
                        parent: Card::ID(old_parent),
                    },
                    ..,
                )) => {
                    this.instances[old_parent.0]
                        .instance_mut()
                        .expect("Card should have been attached to a public parent")
                        .attachment = None;
                }
                Some((zone, index)) => {
                    this.player_cards_mut(owner).remove_from(zone, index);
                }
                None => (),
            }

            let field_index = if let Zone::Field = to_zone {
                let my_instance = instance.as_ref().unwrap_or_else(|| {
                id.instance(this, None)
                    .expect("Card going to field isn't being removed from a secret, so should be in public state.")
            });
                let my_attachment = attachment_instance.as_ref().or_else(|| {
                my_instance
                    .attachment
                    .map(|attach| attach.instance(this, None)
                    .expect("Attachment of card going to field isn't being removed from a secret, so should be in public state."))
            });

                let field_index = this
                    .player_cards(to_player)
                    .field
                    .iter()
                    .map(|id| {
                        let instance = id
                            .instance(this, None)
                            .expect("Instances on the field are in public state");
                        CardInfo {
                            instance,
                            owner: to_player,
                            zone: Zone::Field,
                            attachment: instance.attachment.map(|attach| {
                                attach.instance(this, None).expect(
                                    "Attachments on instances on the field are in public state",
                                )
                            }),
                        }
                    })
                    .position(move |card| {
                        S::field_order(
                            card,
                            CardInfo {
                                instance: &my_instance,
                                owner: to_player,
                                zone: Zone::Field,
                                attachment: my_attachment,
                            },
                        ) == Ordering::Greater
                    })
                    .unwrap_or_else(|| this.player_cards(to_player).field.len());
                Some(field_index)
            } else {
                None
            };

            match to_zone {
                Zone::Deck => {
                    this.context.mutate_secret(to_player, |mut secret| {
                        secret.deck.push(id);
                    });

                    this.player_cards_mut(to_player).deck += 1;
                }
                Zone::Hand { public: false } => {
                    this.context.mutate_secret(to_player, |mut secret| {
                        secret.hand.push(Some(id));
                    });

                    this.player_cards_mut(to_player).hand.push(None);
                }
                Zone::Hand { public: true } => {
                    let index = match location {
                        Some((Zone::Hand { public: false }, Some(index))) if to_player == owner => {
                            index
                        }
                        _ => this.player_cards(to_player).hand.len(),
                    };

                    this.player_cards_mut(to_player)
                        .hand
                        .insert(index, Some(id));

                    this.context.mutate_secret(to_player, |mut secret| {
                        secret.hand.insert(index, None);
                    });
                }
                Zone::Field => {
                    this.player_cards_mut(to_player).field.insert(
                        field_index.expect("field_index should be Some when to_zone is Field"),
                        id,
                    );
                }
                Zone::Graveyard => {
                    this.player_cards_mut(to_player).graveyard.push(id);
                }
                Zone::Limbo { public: false } => {
                    this.context.mutate_secret(to_player, |mut secret| {
                        secret.limbo.push(id);
                    });
                }
                Zone::Limbo { public: true } => {
                    this.player_cards_mut(to_player).limbo.push(id);
                }
                Zone::CardSelection => {
                    this.context.mutate_secret(to_player, |mut secret| {
                        secret.card_selection.push(id);
                    });

                    this.player_cards_mut(to_player).card_selection += 1;
                }
                Zone::Casting => {
                    this.player_cards_mut(to_player).casting.push(id);
                }
                Zone::Dust { public: false } => {
                    this.context.mutate_secret(to_player, |mut secret| {
                        secret.dust.push(id);
                    });
                }
                Zone::Dust { public: true } => {
                    this.player_cards_mut(to_player).dust.push(id);
                }
                Zone::Attachment { .. } => unreachable!("Cannot move card to attachment zone"),
            }

            if let Some(instance) = instance.clone() {
                // we have a new instance, need to put it somewhere.
                let id = instance.id;

                match to_bucket {
                    None => {
                        this.instances[id.0] = instance.into();
                    }
                    Some(to_bucket_player) => {
                        this.instances[id.0] = to_bucket_player.into();

                        this.context
                            .mutate_secret(to_bucket_player, move |mut secret| {
                                secret.instances.insert(instance.id, instance.clone());
                            });
                    }
                }

                // If we have an attachment_instance, we also need to put it somewhere the same way.
                if let Some(attachment_instance) = attachment_instance.clone() {
                    let attachment_id = attachment_instance.id;

                    match to_bucket {
                        None => {
                            this.instances[attachment_id.0] = attachment_instance.into();
                        }
                        Some(to_bucket_player) => {
                            let attachment_id = attachment_instance.id;
                            this.instances[attachment_id.0] = to_bucket_player.into();

                            this.context
                                .mutate_secret(to_bucket_player, move |mut secret| {
                                    secret.instances.insert(
                                        attachment_instance.id,
                                        attachment_instance.clone(),
                                    );
                                });
                        }
                    }
                }
            }
            // we have to emit a sort field before we emit the card move event, otherwise things with same ID will sort wrong.
            if to_zone.is_field() {
                let mut logs = vec![];
                this.sort_field(
                    to_player,
                    old_field
                        .clone()
                        .expect("If moving to field, we should have cached the old field."),
                    false,
                    &mut |event| logs.push(event),
                );
                for event in logs.into_iter() {
                    this.context.log(event);
                }
            }

            let move_card_event = (
                instance.map(|i| (i, attachment_instance)).or_else(|| {
                    id.instance(this, None).map(|instance| {
                        (
                            instance.clone(),
                            instance
                                .attachment
                                .map(|a_id| a_id.instance(this, None).unwrap().clone()),
                        )
                    })
                }),
                CardLocation {
                    player: owner,
                    location,
                },
                ExactCardLocation {
                    player: to_player,
                    location: (
                        to_zone,
                        match to_zone {
                            Zone::Deck => this.player_cards(to_player).deck() - 1,
                            Zone::Hand { .. } => match location {
                                Some((Zone::Hand { public: false }, Some(index)))
                                    if to_player == owner =>
                                {
                                    index
                                }
                                _ => this.player_cards(to_player).hand.len() - 1,
                            },
                            Zone::Field => field_index
                                .expect("field_index should be Some when to_zone is Field"),
                            Zone::Graveyard => this.player_cards(to_player).graveyard().len() - 1,
                            Zone::Limbo { public: false } => 0,
                            Zone::Limbo { public: true } => {
                                this.player_cards(to_player).limbo().len() - 1
                            }
                            Zone::CardSelection => {
                                this.player_cards(to_player).card_selection() - 1
                            }
                            Zone::Casting => this.player_cards(to_player).casting().len() - 1,
                            Zone::Dust { public: false } => 0,
                            Zone::Dust { public: true } => {
                                this.player_cards(to_player).dust().len() - 1
                            }
                            Zone::Attachment { .. } => 0,
                        },
                    ),
                },
            );

            this.context.mutate_secret_or_log(owner, |mut secret| {
                let (instance, mut from, to) = move_card_event.clone();

                if from.location.is_none() {
                    let missing_location = secret.deferred_locations.pop().expect("If from location is none, publically, and we're the player, we should have the deferred location.");
                    from.location = Some(missing_location);
                }

                secret.log(CardEvent::MoveCard { instance, from, to });
            },CardEvent::MoveCard { instance: move_card_event.0.clone(), from: move_card_event.1.clone(), to: move_card_event.2.clone() });

            for deferred_log in deferred_logs {
                this.context.log(deferred_log);
            }

            for log_player in 0..2 {
                this.context.mutate_secret(log_player, |secret| {
                    for deferred_log in secret.secret.deferred_logs.drain(..) {
                        (secret.log)(deferred_log);
                    }
                });
            }

            match to_zone {
                Zone::Deck => {
                    if this.shuffle_deck_on_insert {
                        this.context.mutate_secret(to_player, |secret| {
                            secret.secret.shuffle_deck(secret.random, secret.log);
                        });
                    }
                }
                Zone::Field => {
                    let mut logs = vec![];
                    this.sort_field(
                        to_player,
                        old_field
                            .expect("If moving to field, we should have cached the old field."),
                        true,
                        &mut |event| logs.push(event),
                    );
                    for event in logs.into_iter() {
                        this.context.log(event);
                    }
                }
                _ => (),
            }

            Ok((
                CardLocation {
                    player: owner,
                    location,
                },
                Some(id),
            ))
        }
    }

    pub async fn move_cards(
        &mut self,
        cards: Vec<Card>,
        to_player: Player,
        to_zone: Zone,
    ) -> Vec<Result<(CardLocation, Option<InstanceID>), error::MoveCardError>> {
        // todo!(): betterize this implementation

        let mut results = Vec::with_capacity(cards.len());

        for card in cards {
            results.push(self.move_card(card, to_player, to_zone).await);
        }

        results
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
        f: impl Fn(SecretCardsInfo<S>),
    ) -> Vec<Card> {
        let start = self.instances.len();

        self.context.mutate_secret(player, |mut secret| {
            secret.next_instance = Some(InstanceID(start));

            f(secret.into())
        });

        let (pointers, end) = self
            .context
            .reveal_unique(
                player,
                |secret| {
                    (secret.pointers.len(), secret.next_instance.expect("`PlayerSecret::next_instance` missing during `CardGame::new_secret_cards` call").0)
                },
                |_| true,
            )
            .await;

        assert!(pointers >= self.player_cards(player).pointers);
        assert!(end >= start);

        self.context.mutate_secret(player, |mut secret| {
            secret.next_instance = None;
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

    pub async fn new_secret_cards_with_fakes(
        &mut self,
        player: Player,
        f: impl Fn(SecretCardsWithFakesInfo<S>),
    ) {
        let start = self.instances.len();

        self.context.mutate_secret(player, |mut secret| {
            secret.next_instance = Some(InstanceID(start));

            f(secret.into())
        });

        let (pointers, end) = self
            .context
            .reveal_unique(
                player,
                |secret| {
                    (secret.pointers.len(), secret.next_instance.expect("`PlayerSecret::next_instance` missing during `CardGame::new_secret_cards` call").0)
                },
                |_| true,
            )
            .await;

        assert!(pointers >= self.player_cards(player).pointers);
        assert!(end >= start);

        self.context.mutate_secret(player, |mut secret| {
            secret.next_instance = None;
        });

        self.instances
            .extend(repeat(InstanceOrPlayer::Player(player)).take(end - start));

        let player_cards = self.player_cards_mut(player);

        player_cards.pointers = pointers;
    }

    pub async fn new_secret_pointers(
        &mut self,
        player: Player,
        f: impl Fn(SecretPointersInfo<S>),
    ) -> Vec<Card> {
        self.context
            .mutate_secret(player, |secret| f(secret.into()));

        let pointers = self
            .context
            .reveal_unique(player, |secret| secret.pointers.len(), |_| true)
            .await;

        assert!(pointers >= self.player_cards(player).pointers);

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
        let secrets = {
            let mut secrets = Vec::with_capacity(self.all_player_cards().len());

            for player in 0u8..secrets
                .capacity()
                .try_into()
                .expect("more than 255 players")
            {
                secrets.push(
                    self.context
                        .reveal_unique(player, |secret| secret.clone(), |_| true)
                        .await,
                );
            }

            secrets
        };

        println!("{:#?}", self.state);
        println!("{:#?}", secrets);
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub async fn reveal_ok(&mut self) -> Result<(), error::RevealOkError> {
        // todo!();

        let secrets = {
            let mut secrets = Vec::with_capacity(self.all_player_cards().len());

            for player in 0u8..secrets
                .capacity()
                .try_into()
                .expect("more than 255 players")
            {
                secrets.push(
                    self.context
                        .reveal_unique(player, |secret| secret.clone(), |_| true)
                        .await,
                );
            }

            secrets
        };

        self.ok(&secrets.iter().map(Some).collect::<Vec<_>>())
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
    pub async fn move_pointer(&mut self, card: impl Into<Card>, player: Option<Player>) -> Card {
        let card = card.into();

        let id = match card {
            Card::ID(id) => id,
            Card::Pointer(OpaquePointer { player, index }) => {
                self.context
                    .reveal_unique(player, move |secret| secret.pointers[index], |_| true)
                    .await
            }
        };

        match player {
            None => id.into(),
            Some(player) => {
                self.new_secret_pointers(player, |mut secret| secret.new_pointer(id))
                    .await[0]
            }
        }
    }

    fn attach_card<'a>(
        &'a mut self,
        card: impl Into<Card>,
        parent: impl Into<Card>,
    ) -> Pin<Box<dyn Future<Output = AttachCardResult> + 'a>> {
        let card = card.into();
        let parent = parent.into();

        Box::pin(async move {
            let buckets: Vec<_> = self
                .instances
                .iter()
                .map(InstanceOrPlayer::player)
                .collect();

            let card_bucket = match card {
                Card::ID(id) => buckets[id.0],
                Card::Pointer(OpaquePointer { player, index }) => {
                    let buckets = buckets.clone();

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
                Card::ID(id) => buckets[id.0],
                Card::Pointer(OpaquePointer { player, index }) => {
                    let buckets = buckets.clone();

                    self.context
                        .reveal_unique(
                            player,
                            move |secret| buckets[secret.pointers[index].0],
                            |_| true,
                        )
                        .await
                }
            };

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
                            .unwrap_or_else(|_| {
                                panic!("unable to move attachment {:?} to public dust", attachment)
                            });
                    }

                    Some(id)
                }
                Some(parent_card_player) => match parent {
                    Card::Pointer(OpaquePointer {
                        player: ptr_player, ..
                    }) if ptr_player == parent_card_player => None,
                    Card::Pointer(..) => {
                        let id = match parent {
                            Card::ID(id) => id,
                            Card::Pointer(OpaquePointer { player, index }) => {
                                self.context
                                    .reveal_unique(
                                        player,
                                        move |secret| secret.pointers[index],
                                        |_| true,
                                    )
                                    .await
                            }
                        };

                        Some(id)
                    }
                    Card::ID(id) => Some(id),
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
                        Some(
                            self.context
                                .reveal_unique(
                                    card_ptr_player,
                                    move |secret| secret.pointers[index],
                                    |_| true,
                                )
                                .await,
                        )
                    }
                }
                Card::ID(card_id) => Some(card_id),
            };

            let owner = card_bucket.unwrap_or_else(|| self.owner(card_id.unwrap()));

            // Reveal the zone that a card came from
            let location = match card_bucket {
                None => {
                    let id = card_id.expect("ID should have been revealed in this case");

                    Some(
                        self.location(id)
                            .location
                            .expect("CardLocation for a public card must be public."),
                    )
                }
                Some(player) => {
                    self.context.mutate_secret(player, move |mut secret| {
                        let location =
                            secret
                                .location(card_id.unwrap_or_else(|| {
                                    secret.pointers[card.pointer().unwrap().index]
                                }))
                                .location
                                .expect("The secret should know the zone.");

                        match location.0 {
                            Zone::Limbo { public: false } | Zone::Attachment { .. } => {
                                secret.deferred_locations.push(location);
                            }
                            _ => {
                                // location will be revealed publicly
                            }
                        }
                    });
                    self.context
                        .reveal_unique(
                            player,
                            move |secret| {
                                let location = secret
                                    .location(card_id.unwrap_or_else(|| {
                                        secret.pointers[card.pointer().unwrap().index]
                                    }))
                                    .location
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

            let mut deferred_logs = vec![];

            if let Some((zone, index)) = location {
                match zone {
                    Zone::Attachment { parent } => {
                        match parent {
                            Card::ID(parent) => {
                                let attach_clone = card_id.expect("Parent is public, so this attach's id must be in public state.")
                                .instance(self, None)
                                .unwrap()
                                .clone();
                                self.modify_card_internal(
                                    parent.into(),
                                    |parent, _| {
                                        parent.attachment = None;
                                        S::on_detach(parent, &attach_clone);
                                    },
                                    &mut |event| deferred_logs.push(event),
                                )
                                .await;
                            },
                            _ => unreachable!("If the location is public, the parent in question will never be secret.")
                        };
                    }
                    _ => self.player_cards_mut(owner).remove_from(zone, index),
                }
            }

            self.context.mutate_secret(owner, |mut secret| {
                // Either we know the ID, or it's in this secret!
                let id = card_id.unwrap_or_else(|| secret.pointers[card.pointer().unwrap().index]);
                let mut deferred_logs = vec![];
                secret.remove_id(&mut |event| deferred_logs.push(event), id);
                secret.deferred_logs.extend(deferred_logs);
            });

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

                        std::mem::replace(
                            &mut self.instances[card_id.0],
                            InstanceOrPlayer::Player(parent_bucket_player),
                        )
                        .instance()
                        .unwrap()
                    }
                    Some(card_bucket_player) => {
                        let instance = self
                            .context
                            .reveal_unique(
                                card_bucket_player,
                                move |secret| secret.instance(card_id).unwrap().clone(),
                                |_| true,
                            )
                            .await;

                        self.context
                            .mutate_secret(card_bucket_player, |mut secret| {
                                secret.instances.remove(&card_id);
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
                            .mutate_secret(parent_bucket_player, |mut secret| {
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
                                            secret.pointers[card.pointer().unwrap().index]
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
                        .mutate_secret_or_log(parent_bucket_player, |mut secret| {
                            let card_id = card_id
                                .unwrap_or_else(|| secret.pointers[card.pointer().unwrap().index]);
                            let parent_id = secret.pointers[parent.pointer().unwrap().index];
                            let location = location.or_else(|| {
                                if card_bucket == Some(parent_bucket_player) {
                                    Some(secret.deferred_locations.pop().expect("Has deferred location, because we're attaching from -> to the same secret, so this secret has the from."))
                                } else {
                                    None
                                }
                            });
                            secret.secret
                                .attach_card(parent_id, card_id, location, secret.log)
                                .unwrap();
                        },CardEvent::MoveCard {
                            instance: None,
                            from: CardLocation {
                                player: owner,
                                location,
                            },
                            to: ExactCardLocation {
                                player: parent_bucket_player,
                                location: (Zone::Attachment { parent }, 0),
                            },
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
                                            move |secret| secret.pointers[index],
                                            |_| true,
                                        )
                                        .await
                                }
                            },
                            Some(card_id) => card_id,
                        };

                        let new_attach = self.instances[card_id.0]
                            .instance_ref()
                            .expect("New instance exists!")
                            .clone();
                        let parent_owner = self.owner(parent_id);
                        let mut logs = vec![];
                        self.modify_card_internal(
                            parent_id.into(),
                            move |parent, log| {
                                parent.attachment = Some(card_id);

                                // Log the card moving to public zone.
                                log(CardEvent::MoveCard {
                                    // we're moving an attach, so it can never have an attach.
                                    instance: Some((new_attach.clone(), None)),
                                    from: CardLocation {
                                        player: owner,
                                        location,
                                    },
                                    to: ExactCardLocation {
                                        player: parent_owner,
                                        location: (
                                            Zone::Attachment {
                                                parent: parent_id.into(),
                                            },
                                            0,
                                        ),
                                    },
                                });

                                S::on_attach(parent, &new_attach);
                            },
                            &mut |event| logs.push(event),
                        )
                        .await;
                        for msg in logs.into_iter() {
                            if let CardEvent::MoveCard { instance, from, to } = msg {
                                let from_player = from.player;
                                self.context
                                .mutate_secret_or_log(from_player, |mut secret| {
                                    let location = from.location.or_else(||
                                        {
                                                Some(secret.deferred_locations.pop().expect("Has deferred location, because we're attaching from -> to the same secret, so this secret has the from."))
                                        });
                                    secret.log(CardEvent::MoveCard {
                                        instance: instance.clone(),
                                        from: CardLocation {
                                            player: from_player,
                                            location,
                                        },
                                        to: to.clone()
                                    })
                                },CardEvent::MoveCard { instance: instance.clone(), from: from.clone(), to: to.clone() });
                            } else {
                                deferred_logs.push(msg);
                            }
                        }
                    }
                    Some(parent_bucket_player) => {
                        self.context
                            .mutate_secret_or_log(parent_bucket_player, |mut secret| {
                                let card_id = card_id.unwrap_or_else(|| {
                                    secret.pointers[card.pointer().unwrap().index]
                                });
                                let location = location.or_else(||
                                    {   if card_bucket == Some(parent_bucket_player) {
                                            Some(secret.deferred_locations.pop().expect("Has deferred location, because we're attaching from -> to the same secret, so this secret has the from."))
                                        } else {
                                            None
                                        }
                                    });
                                secret.secret
                                    .attach_card(parent_id, card_id, location, secret.log)
                                    .unwrap();
                            },CardEvent::MoveCard {
                                instance: None,
                                from: CardLocation {
                                    player: owner,
                                    location,
                                },
                                to: ExactCardLocation {
                                    player: parent_bucket_player,
                                    location: (
                                        Zone::Attachment {
                                            parent: parent_id.into(),
                                        },
                                        0,
                                    ),
                                },
                            });
                    }
                },
            }

            for deferred_log in deferred_logs {
                self.context.log(deferred_log);
            }

            for log_player in 0..2 {
                self.context.mutate_secret(log_player, |secret| {
                    for deferred_log in secret.secret.deferred_logs.drain(..) {
                        (secret.log)(deferred_log);
                    }
                });
            }

            Ok((
                CardLocation {
                    player: owner,
                    location,
                },
                card_id,
            ))
        })
    }

    fn sort_field(
        &mut self,
        player: Player,
        old_field: Vec<InstanceID>,
        actually_update: bool,
        logger: &mut dyn FnMut(<GameState<S> as arcadeum::store::State>::Event),
    ) {
        let mut field = self.player_cards(player).field.clone();

        field.sort_by(|a, b| {
            let a = self.instances[a.0]
                .instance_ref()
                .expect("field card is not public");

            let b = self.instances[b.0]
                .instance_ref()
                .expect("field card is not public");

            let a = CardInfo {
                instance: a,
                owner: player,
                zone: Zone::Field,
                attachment: a.attachment.map(|attachment| {
                    self.instances[attachment.0]
                        .instance_ref()
                        .expect("field card attachment is not public")
                }),
            };

            let b = CardInfo {
                instance: b,
                owner: player,
                zone: Zone::Field,
                attachment: b.attachment.map(|attachment| {
                    self.instances[attachment.0]
                        .instance_ref()
                        .expect("field card attachment is not public")
                }),
            };

            S::field_order(a, b)
        });

        if field != old_field {
            logger(CardEvent::SortField {
                player,
                field: field.clone(),
                real: actually_update,
            });
        }

        if actually_update {
            // Finally, actually update the field order in state.
            self.player_cards_mut(player).field = field;
        }
    }
}

#[derive(Debug)]
pub struct CardInfo<'a, S: State> {
    pub instance: &'a CardInstance<S>,
    pub owner: Player,
    pub zone: Zone,
    pub attachment: Option<&'a CardInstance<S>>,
}

impl<S: State> Deref for CardInfo<'_, S> {
    type Target = CardInstance<S>;

    fn deref(&self) -> &Self::Target {
        self.instance
    }
}

pub struct CardInfoMut<'a, S: State> {
    pub instance: &'a mut CardInstance<S>,
    pub owner: Player,
    pub zone: Zone,
    pub attachment: Option<&'a CardInstance<S>>,
    pub log: &'a mut dyn FnMut(<GameState<S> as arcadeum::store::State>::Event),
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

pub struct SecretCardsInfo<'a, S: State>(MutateSecretInfo<'a, S>);

impl<'a, S: State> Deref for SecretCardsInfo<'a, S> {
    type Target = MutateSecretInfo<'a, S>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S: State> DerefMut for SecretCardsInfo<'_, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a, S: State> From<MutateSecretInfo<'a, S>> for SecretCardsInfo<'a, S> {
    fn from(secret: MutateSecretInfo<'a, S>) -> Self {
        Self(secret)
    }
}

impl<S: State> SecretCardsInfo<'_, S> {
    pub fn new_card(&mut self, base: S::BaseCard) -> InstanceID {
        let mut next_instance = self.next_instance.expect(
            "`PlayerSecret::next_instance` missing during `CardGame::new_secret_cards` call",
        );

        let attachment = base.attachment().map(|attachment| {
            let state = attachment.new_card_state(None);
            let instance = CardInstance {
                id: next_instance,
                base: attachment,
                attachment: None,
                state,
            };

            self.instances.insert(next_instance, instance);

            next_instance
        });

        next_instance.0 += 1;

        let card = next_instance;
        let state = base.new_card_state(None);
        let instance = CardInstance {
            id: next_instance,
            base,
            attachment,
            state,
        };

        self.instances.insert(next_instance, instance);

        next_instance.0 += 1;

        self.next_instance = Some(next_instance);

        self.limbo.push(card);

        self.pointers.push(card);

        if let Some(attach_id) = attachment {
            let attachment = self.instance(attach_id).unwrap().clone();
            self.0
                .secret
                .modify_card_internal(card, self.0.log, |parent, _| {
                    S::on_attach(parent, &attachment);
                });
        }

        card
    }

    pub(crate) fn dust_card(
        &mut self,
        card: impl Into<Card>,
    ) -> Result<(), error::SecretMoveCardError> {
        self.0.secret.dust_card(card, self.0.log)
    }

    pub fn attach_card(
        &mut self,
        card: impl Into<Card>,
        attachment: impl Into<Card>,
    ) -> Result<(), error::SecretMoveCardError> {
        self.0
            .secret
            .attach_card(card, attachment, None, self.0.log)
    }

    pub fn modify_card(
        &mut self,
        card: impl Into<Card>,
        f: impl FnOnce(CardInfoMut<S>),
    ) -> Result<(), error::SecretModifyCardError> {
        self.0.secret.modify_card(card, self.0.log, f)
    }
}

pub struct SecretCardsWithFakesInfo<'a, S: State>(SecretCardsInfo<'a, S>);

impl<'a, S: State> Deref for SecretCardsWithFakesInfo<'a, S> {
    type Target = SecretCardsInfo<'a, S>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S: State> DerefMut for SecretCardsWithFakesInfo<'_, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a, S: State> From<SecretCardsInfo<'a, S>> for SecretCardsWithFakesInfo<'a, S> {
    fn from(secret: SecretCardsInfo<'a, S>) -> Self {
        Self(secret)
    }
}

impl<'a, S: State> From<MutateSecretInfo<'a, S>> for SecretCardsWithFakesInfo<'a, S> {
    fn from(secret: MutateSecretInfo<'a, S>) -> Self {
        Self(secret.into())
    }
}

impl<S: State> SecretCardsWithFakesInfo<'_, S> {
    pub fn new_fake_card(&mut self) {
        self.next_instance.as_mut().expect("`PlayerSecret::next_instance` missing during `CardGame::new_secret_cards_with_fakes` call").0 += 1;
    }
}

pub struct SecretPointersInfo<'a, S: State>(MutateSecretInfo<'a, S>);

impl<'a, S: State> Deref for SecretPointersInfo<'a, S> {
    type Target = MutateSecretInfo<'a, S>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S: State> DerefMut for SecretPointersInfo<'_, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a, S: State> From<MutateSecretInfo<'a, S>> for SecretPointersInfo<'a, S> {
    fn from(secret: MutateSecretInfo<'a, S>) -> Self {
        Self(secret)
    }
}

impl<S: State> SecretPointersInfo<'_, S> {
    pub fn new_pointer(&mut self, id: InstanceID) {
        self.pointers.push(id);
    }
}

type MutateSecretInfo<'a, S> = arcadeum::store::MutateSecretInfo<
    'a,
    <GameState<S> as arcadeum::store::State>::Secret,
    <GameState<S> as arcadeum::store::State>::Event,
>;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
enum Either<A, B> {
    A(A),
    B(B),
}
