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
        todo!();
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
        let dust = self
            .context
            .reveal_unique(player, |secret| secret.dust().len(), |_| true)
            .await;

        self.context.mutate_secret(player, |secret, _, _| {
            secret.append_dust_to_pointers();
        });

        let player_cards = self.player_cards_mut(player);

        player_cards.pointers += dust;

        (player_cards.pointers - dust..player_cards.pointers)
            .map(|index| OpaquePointer { player, index }.into())
            .collect()
    }

    pub fn public_limbo_cards(&self, player: Player) -> &Vec<InstanceID> {
        self.player_cards(player).limbo()
    }

    /// This reveals the number of cards in a player's secret limbo.
    pub async fn secret_limbo_cards(&mut self, player: Player) -> Vec<Card> {
        let limbo = self
            .context
            .reveal_unique(player, |secret| secret.limbo().len(), |_| true)
            .await;

        self.context.mutate_secret(player, |secret, _, _| {
            secret.append_limbo_to_pointers();
        });

        let player_cards = self.player_cards_mut(player);

        player_cards.pointers += limbo;

        (player_cards.pointers - limbo..player_cards.pointers)
            .map(|index| OpaquePointer { player, index }.into())
            .collect()
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
                                    .reveal_from_card(secret.pointers[index], |info| {
                                        Either::A(f(info))
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
        todo!();
    }

    pub async fn reveal_parents(&mut self, cards: Vec<Card>) -> Vec<Option<Card>> {
        todo!();
    }

    pub async fn filter_cards(
        &mut self,
        cards: Vec<Card>,
        f: impl Fn(CardInfo<S>) -> bool,
    ) -> impl Iterator<Item = Card> {
        cards
            .clone()
            .into_iter()
            .zip(self.reveal_from_cards(cards, f).await)
            .filter_map(|(card, f)| if f { Some(card) } else { None })
    }

    pub async fn modify_card(&mut self, card: impl Into<Card>, f: impl Fn(CardInfoMut<S>)) {
        todo!();
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
