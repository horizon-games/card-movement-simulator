//! Infrastructure for writing arcadeum-state card games involving imperfect information
//!
//! Card Movement Simulator provides a framework for card games that allow cards to move between different states of secrecy, move between different "zones", and be attached to each other in parent/child relationships.
//! It makes an effort to ignore the concrete game mechanics specific to any particular game.
//! However, it's written for SkyWeaver to consume, and thus currently has a specific set of [Zone]s that SkyWeaver uses (PRs welcome!).
//!
//! Card Movement Simulator is designed to eliminate information leakage, and only reveal the minimum amount of information required for a given card motion.
//!
//! **Card Movement Simulator currently requires trusting your opponent (in arcadeum-state's p2p mode) or trusting the game's owner, because reveals of secret information are not verified.**

#![cfg_attr(feature = "reveal-backtrace", feature(backtrace))]
#![warn(missing_docs)]
mod instance_id;
mod opaque_ptr;

use std::{
    convert::TryInto,
    fmt::{Debug, Error, Formatter},
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
};

pub use {
    arcadeum::{crypto::Address, store::Event, Nonce, Player, ID},
    instance_id::InstanceID,
    opaque_ptr::OpaquePointer,
};

/// Game-specific public state structure
pub trait State: serde::Serialize + serde::de::DeserializeOwned + Clone + Debug + 'static {
    /// Identifier type
    type ID: ID;

    /// Nonce type
    type Nonce: Nonce;

    /// Action type
    type Action: Action;

    /// Secret type
    type Secret: Secret;

    /// Gets the ABI version of this implementation.
    ///
    /// See [arcadeum::tag] and [arcadeum::version::version] for potentially helpful utilities.
    fn version() -> &'static [u8];

    /// Gets the challenge that must be signed in order to certify the subkey with the given address.
    fn challenge(address: &Address) -> String {
        format!(
            "Sign to play! This won't cost anything.\n\n{}\n",
            arcadeum::crypto::Addressable::eip55(address)
        )
    }

    /// Verifies if an action by a given player is valid for the state.
    fn verify(&self, player: Option<Player>, action: &Self::Action) -> Result<(), String>;

    /// Applies an action by a given player to the state.
    fn apply(
        game: LiveGame<Self>,
        player: Option<Player>,
        action: Self::Action,
    ) -> Pin<Box<dyn Future<Output = LiveGame<Self>>>>;
}

/// Game-specific secret state structure
pub trait Secret:
    arcadeum::store::Secret + serde::Serialize + serde::de::DeserializeOwned + Debug
{
    /// Game-specific card template
    type BaseCard: BaseCard;
}

/// Game-specific card template
// todo!(): remove these trait bounds
pub trait BaseCard: Clone + Debug {
    /// The card state structure associated with this template
    type CardState: CardState;

    /// The base card to be attached to new card instances
    fn attachment(&self) -> Option<Self>;

    /// Creates a new CardState from this BaseCard
    fn new_card_state(&self) -> Self::CardState;
}

/// Game-specific card state structure
pub trait CardState: serde::Serialize + serde::de::DeserializeOwned + Clone + Debug {}

impl<T: serde::Serialize + serde::de::DeserializeOwned + Clone + Debug> CardState for T {}

/// Game-specific state transition
pub trait Action: arcadeum::Action + Debug {}

impl<T: arcadeum::Action + Debug> Action for T {}

/// A live game
pub struct LiveGame<S: State> {
    game: CardGame<S>,

    /// The interface to the game's secret state.
    pub context: Context<CardGame<S>>,
}

impl<S: State> LiveGame<S> {
    /// Constructs a new live game.
    pub fn new(game: CardGame<S>, context: arcadeum::store::Context<CardGame<S>>) -> Self {
        Self {
            game,
            context: Context(context),
        }
    }

    /// Invalidates all pointers.
    pub fn invalidate_pointers(&mut self) {
        for player in 0..2 {
            self.context.mutate_secret(player, |secret, _, _| {
                secret.opaque_ptrs.clear();
            });
        }

        self.game.opaque_ptrs.clear();
    }

    /// Creates a new public card in a player's limbo.
    ///
    /// If you want to secretly create a card in a player's secret limbo, use [CardGameSecret::new_card] from within a [LiveGame::new_secret_cards] instead.
    pub fn new_card(
        &mut self,
        player: Player,
        base: <<S as State>::Secret as Secret>::BaseCard,
    ) -> OpaquePointer {
        let attachment = base.attachment().map(|attachment| {
            let id = InstanceID::from_raw(self.game.cards.len());

            self.game.cards.push(MaybeSecretCard::Public(CardInstance {
                id,
                attachment: None,
                card_state: attachment.new_card_state(),
            }));

            id
        });

        let id = InstanceID::from_raw(self.game.cards.len());

        self.game.cards.push(MaybeSecretCard::Public(CardInstance {
            id,
            attachment,
            card_state: base.new_card_state(),
        }));

        self.game.player_mut(player).limbo.push(id);

        self.new_public_pointer(id)
    }

    /// Gets a pointer to a player's deck card.
    pub fn deck_card(&mut self, player: Player, index: usize) -> OpaquePointer {
        let card = OpaquePointer::from_raw(self.game.opaque_ptrs.len());

        self.context.mutate_secret(player, |secret, _, _| {
            secret.opaque_ptrs.insert(card, secret.deck[index]);
        });

        self.game.opaque_ptrs.push(MaybeSecretID::Secret(player));

        card
    }

    /// Gets a pointer to a player's hand card.
    pub fn hand_card(&mut self, player: Player, index: usize) -> OpaquePointer {
        match self.game.player(player).hand[index] {
            None => {
                let card = OpaquePointer::from_raw(self.game.opaque_ptrs.len());

                self.context.mutate_secret(player, |secret, _, _| {
                    secret.opaque_ptrs.insert(
                        card,
                        secret.hand[index].expect("hand card is neither public nor secret"),
                    );
                });

                self.game.opaque_ptrs.push(MaybeSecretID::Secret(player));

                card
            }
            Some(id) => self.new_public_pointer(id),
        }
    }

    /// Gets a pointer to a player's field card.
    pub fn field_card(&mut self, player: Player, index: usize) -> OpaquePointer {
        self.new_public_pointer(self.game.player(player).field[index])
    }

    /// Gets a pointer to a player's graveyard card.
    pub fn graveyard_card(&mut self, player: Player, index: usize) -> OpaquePointer {
        self.new_public_pointer(self.game.player(player).graveyard[index])
    }

    /// Gets a pointer to a player's card selection card.
    pub fn card_selection_card(&mut self, player: Player, index: usize) -> OpaquePointer {
        let card = OpaquePointer::from_raw(self.game.opaque_ptrs.len());

        self.context.mutate_secret(player, |secret, _, _| {
            secret
                .opaque_ptrs
                .insert(card, secret.card_selection[index]);
        });

        self.game.opaque_ptrs.push(MaybeSecretID::Secret(player));

        card
    }

    /// Gets a pointer to a player's casting card.
    pub fn casting_card(&mut self, player: Player, index: usize) -> OpaquePointer {
        self.new_public_pointer(self.game.player(player).casting[index])
    }

    /// Gets a pointer to a player's publicly dusted card.
    pub fn dusted_card(&mut self, player: Player, index: usize) -> OpaquePointer {
        self.new_public_pointer(self.game.player(player).dusted[index])
    }

    /// Gets a pointer to a card's attachment, if any.
    ///
    /// This reveals the knowledge of whether or not the card has an attachment.
    pub async fn attachment(&mut self, card: OpaquePointer) -> Option<OpaquePointer> {
        // in what bucket is the card?

        // we only have to reveal the instance id if the card is in a different bucket than the pointer

        // case 1: the card is in the same bucket as the pointer
        // - reveal if the card has an attachment or not, by revealing Option<OpaquePointer> where the pointer does not yet exist in the secret state.
        // - if it does, (mutate_secret) insert in secret that opaque pointer to that attachment instance id
        //   - return the next pointer
        // - if it doesn't, return None

        // case 2: the card is in a different bucket from the pointer
        // - publish the card pointer
        // - it's either public or the other's player's secret
        // - if it's public, create a new public pointer to it, if it exists (None if it doesn't)
        // - if it's secret, insert in secret a new opaque pointer to the secret attachment
        //   - return the next pointer

        // we might possibly need to reveal whether or not the card has an attachment

        // if the pointer is in a player's secret
        //      if the card is in the same secret, we can reveal the opaque pointer attachment.
        //      else, we have to reveal the instance ID of the parent, which can then
        //           be used to look up the attachment in either public state or the other player's secret

        match self.game.opaque_ptrs[usize::from(card)] {
            MaybeSecretID::Secret(player) => {
                let card_bucket = self.reveal_card_bucket(card).await;

                if card_bucket == Bucket::Secret(player) {
                    let has_attachment = self
                        .context
                        .reveal_unique(
                            player,
                            move |secret| {
                                secret.cards[&secret.opaque_ptrs[&card]]
                                    .attachment
                                    .is_some()
                            },
                            |_| true,
                        )
                        .await;

                    if has_attachment {
                        let ptr = OpaquePointer::from_raw(self.game.opaque_ptrs.len());

                        self.context.mutate_secret(player, |secret, _, _| {
                            secret.opaque_ptrs.insert(
                                ptr,
                                secret.cards[&secret.opaque_ptrs[&card]]
                                    .attachment
                                    .expect("The attachment disappeared"),
                            );
                        });

                        self.game.opaque_ptrs.push(MaybeSecretID::Secret(player));

                        Some(ptr)
                    } else {
                        None
                    }
                } else {
                    let id = self.reveal_id(card).await;

                    match &self.game.cards[usize::from(id)] {
                        MaybeSecretCard::Secret(player) => {
                            let has_attachment = self
                                .context
                                .reveal_unique(
                                    *player,
                                    move |secret| secret.cards[&id].attachment.is_some(),
                                    |_| true,
                                )
                                .await;

                            if has_attachment {
                                let ptr = OpaquePointer::from_raw(self.game.opaque_ptrs.len());

                                self.context.mutate_secret(*player, |secret, _, _| {
                                    let attachment = secret.cards[&id]
                                        .attachment
                                        .expect("The attachment vanished");
                                    secret.opaque_ptrs.insert(ptr, attachment);
                                });

                                self.game.opaque_ptrs.push(MaybeSecretID::Secret(*player));

                                Some(ptr)
                            } else {
                                None
                            }
                        }
                        MaybeSecretCard::Public(instance) => instance
                            .attachment
                            .map(|attachment| self.new_public_pointer(attachment)),
                    }
                }
            }
            MaybeSecretID::Public(id) => {
                match &self.game.cards[usize::from(id)] {
                    MaybeSecretCard::Secret(player) => {
                        // Reveal whether or not there's an attachment
                        if self
                            .context
                            .reveal_unique(
                                *player,
                                move |secret| secret.cards[&id].attachment.is_some(),
                                |_| true,
                            )
                            .await
                        {
                            let ptr = OpaquePointer::from_raw(self.game.opaque_ptrs.len());

                            // Create an opaque pointer to the attachment
                            self.context.mutate_secret(*player, |secret, _, _| {
                                let attachment = secret.cards[&id]
                                    .attachment
                                    .expect("The attachment disappeared");

                                secret.opaque_ptrs.insert(ptr, attachment);
                            });

                            self.game.opaque_ptrs.push(MaybeSecretID::Secret(*player));

                            Some(ptr)
                        } else {
                            None
                        }
                    }
                    MaybeSecretCard::Public(instance) => instance
                        .attachment
                        .map(|attachment| self.new_public_pointer(attachment)),
                }
            }
        }
    }

    /// Draws a card from a player's deck to their hand.
    pub async fn draw_card(&mut self, player: Player) -> Option<OpaquePointer> {
        match self.game.player(player).deck {
            0 => None,
            size => {
                let index =
                    rand::RngCore::next_u32(&mut self.context.random().await) as usize % size;

                let card = self.deck_card(player, index);

                self.move_card(card, player, Zone::Hand { public: false })
                    .await;

                Some(card)
            }
        }
    }

    /// Moves a card to another zone.
    pub async fn move_card(&mut self, card: OpaquePointer, to_player: Player, to_zone: Zone) {
        let to_bucket = match to_zone {
            Zone::Deck => Bucket::Secret(to_player),
            Zone::Hand { public: false } => Bucket::Secret(to_player),
            Zone::Hand { public: true } => Bucket::Public,
            Zone::Field => Bucket::Public,
            Zone::Graveyard => Bucket::Public,
            Zone::Limbo { public: false } => Bucket::Secret(to_player),
            Zone::Limbo { public: true } => Bucket::Public,
            Zone::CardSelection => Bucket::Secret(to_player),
            Zone::Casting => Bucket::Public,
            Zone::Dusted { public: false } => Bucket::Secret(to_player),
            Zone::Dusted { public: true } => Bucket::Public,
            Zone::Attachment { parent } => {
                self.attach_card(card, parent).await;
                return;
            }
        };

        // We always need to know who owns the card instance itself.

        // Either this card is in Public state (None) or a player's secret (Some(player)).
        // We also need to know who owns the card, regardless of its secrecy, so we can later update the public state for that player.
        let (bucket, owner) = match self.game.opaque_ptrs[usize::from(card)] {
            MaybeSecretID::Secret(player) => {
                let buckets: Vec<_> = self.game.card_buckets().collect();

                let owner_table: indexmap::IndexMap<_, _> = self
                    .game
                    .cards
                    .iter()
                    .filter_map(|msc| {
                        msc.instance_ref().map(|instance| {
                            (
                                instance.id,
                                if self.game.does_own(0, instance.id) {
                                    0
                                } else if self.game.does_own(1, instance.id) {
                                    1
                                } else {
                                    unreachable!("It's public but neither player owns it");
                                },
                            )
                        })
                    })
                    .collect();

                self.context
                    .reveal_unique(
                        player,
                        move |secret| {
                            let id = secret.opaque_ptrs[&card];
                            let bucket = buckets[usize::from(id)];
                            let is_in_my_secret = secret.does_own(id);

                            let owner = if is_in_my_secret {
                                // If our secret contains this card, we're obviously its owner
                                player
                            } else {
                                // If our secret doesn't contain this card,
                                // since we have the instance ID now, we should scan through the public player states to determine the owner.

                                if owner_table.contains_key(&id) {
                                    owner_table[&id]
                                } else {
                                    // If we don't see anyone claiming ownership in our secret or the public state,
                                    // the only remaining case is that the other player owns this card.
                                    1 - player
                                }
                            };
                            (bucket, owner)
                        },
                        |_| true,
                    )
                    .await
            }
            MaybeSecretID::Public(id) => {
                if self.game.does_own(0, id) {
                    (Bucket::Public, 0)
                } else if self.game.does_own(1, id) {
                    (Bucket::Public, 1)
                } else if self
                    .context
                    .reveal_unique(0, move |secret| secret.does_own(id), |_| true)
                    .await
                {
                    (Bucket::Secret(0), 0)
                } else if self
                    .context
                    .reveal_unique(1, move |secret| secret.does_own(id), |_| true)
                    .await
                {
                    (Bucket::Secret(1), 1)
                } else {
                    unreachable!("Nobody has this ID {:?}", id);
                }
            }
        };

        let id = match self.game.opaque_ptrs[usize::from(card)] {
            MaybeSecretID::Secret(player) => {
                if bucket != Bucket::Secret(player) || to_bucket != Bucket::Secret(player) {
                    Some(
                        self.context
                            .reveal_unique(
                                player,
                                move |secret| secret.opaque_ptrs[&card],
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
            MaybeSecretID::Public(id) => Some(id),
        };

        let location = match bucket {
            Bucket::Public => {
                let id = id.expect("ID should have been revealed in this case");

                Some(self.game.player(owner).id_location(id)
                .or_else(|| self.game.cards.iter().flat_map(MaybeSecretCard::instance_ref).find_map(|instance| if instance.attachment == Some(id) {
                    Some(instance.id)
                } else {
                    None
                }.map(|parent| PublicLocation::PublicAttachment { parent }))).expect("Bucket is None, player state claims that it has card, but somehow we couldn't find it in any zones."))
            }
            Bucket::Secret(player) => {
                self.context
                    .reveal_unique(
                        player,
                        move |secret| {
                            secret.id_location(id.unwrap_or_else(|| secret.opaque_ptrs[&card]))
                        },
                        |_| true,
                    )
                    .await
            }
        };

        // Special case, secret -> secret for a single player
        if let Bucket::Secret(bucket_owner) = bucket {
            if to_bucket == bucket {
                self.context.mutate_secret(bucket_owner, |secret, _, _| {
                    let id = id.unwrap_or_else(|| secret.opaque_ptrs[&card]);
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
                        Zone::Dusted { public: false } => secret.dusted.push(id),
                        Zone::Dusted { public: true } => unreachable!(),
                        Zone::Attachment { .. } => {
                            unreachable!("Can't attach a spell with move_card.")
                        }
                    }
                });

                if let Some(location) = location {
                    self.game.player_mut(bucket_owner).remove_from(location);
                }

                // Update the public state about where we put this card
                let player_state = self.game.player_mut(to_player);
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
                    Zone::Dusted { public: false } => {
                        // do nothing, this is a secret
                    }
                    Zone::Dusted { public: true } => {
                        unreachable!();
                    }
                    Zone::Attachment { .. } => unreachable!("Cannot move card to attachment zone"),
                }

                return;
            }
        }

        let (instance, attachment_instance) = match bucket {
            Bucket::Public => {
                let id = id.expect("Card is in public state, but we don't know its id.");

                if let Bucket::Secret(to_bucket_player) = to_bucket {
                    let instance = std::mem::replace(
                        &mut self.game.cards[usize::from(id)],
                        MaybeSecretCard::Secret(to_bucket_player),
                    )
                    .expect(
                        "Card was identified as public, but it's actually MaybeSecretCard::Secret",
                    );

                    let attachment = instance.attachment.map(|attachment_id| {
                        std::mem::replace(&mut self.game.cards[usize::from(attachment_id)], MaybeSecretCard::Secret(to_bucket_player)).expect("Since parent Card is public, attachment was identified as public, but it's actually MaybeSecretCard::Secret")
                    });

                    self.context.mutate_secret(owner, |secret, _, _| {
                        if let Some(PublicLocation::Hand(index)) = location {
                            secret.hand.remove(index);
                        }
                    });

                    (Some(instance), attachment)
                } else {
                    // we're moving from public to public
                    (None, None)
                }
            }
            Bucket::Secret(player) => {
                let (instance, attachment_instance) = self
                    .context
                    .reveal_unique(
                        player,
                        move |secret| {
                            let id = id.unwrap_or_else(|| secret.opaque_ptrs[&card]);

                            let instance = &secret.cards[&id];

                            (
                                Some(instance.clone()),
                                instance
                                    .attachment
                                    .map(|attachment| secret.cards[&attachment].clone()),
                            )
                        },
                        |_| true,
                    )
                    .await;

                self.context.mutate_secret(player, move |secret, _, _| {
                    let id = id.unwrap_or_else(|| secret.opaque_ptrs[&card]);

                    // We're removing a card with an attachment from the secret
                    if let Some(attachment_id) = secret.cards[&id].attachment {
                        secret.cards.remove(&attachment_id);
                    }

                    secret.cards.remove(&id);

                    // find what collection id is in and remove it
                    secret.deck.retain(|i| *i != id);
                    secret.hand.retain(|i| *i != Some(id));
                    secret.limbo.retain(|i| *i != id);
                    secret.card_selection.retain(|i| *i != id);
                    secret.dusted.retain(|i| *i != id);

                    // We're removing the attachment from a card in the secret
                    if let Some(parent_instance) =
                        secret.cards.values_mut().find(|c| c.attachment == Some(id))
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

        let player_state = self.game.player_mut(to_player);
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
            Zone::Dusted { public: false } => {
                self.context.mutate_secret(to_player, |secret, _, _| {
                    secret.dusted.push(id);
                });
            }
            Zone::Dusted { public: true } => {
                player_state.dusted.push(id);
            }
            Zone::Attachment { .. } => unreachable!("Cannot move card to attachment zone"),
        }

        if let Some(instance) = instance {
            // we have a new instance, need to put it somewhere.
            let id = instance.id;

            match to_bucket {
                Bucket::Public => {
                    self.game.cards[usize::from(id)] = MaybeSecretCard::Public(instance);
                }
                Bucket::Secret(to_bucket_player) => {
                    self.game.cards[usize::from(id)] = MaybeSecretCard::Secret(to_bucket_player);

                    self.context
                        .mutate_secret(to_bucket_player, move |secret, _, _| {
                            secret.cards.insert(instance.id, instance.clone());
                        });
                }
            }

            // If we have an attachment_instance, we also need to put it somewhere the same way.
            if let Some(attachment_instance) = attachment_instance {
                let attachment_id = attachment_instance.id;

                match to_bucket {
                    Bucket::Public => {
                        self.game.cards[usize::from(attachment_id)] =
                            MaybeSecretCard::Public(attachment_instance);
                    }
                    Bucket::Secret(to_bucket_player) => {
                        let attachment_id = attachment_instance.id;
                        self.game.cards[usize::from(attachment_id)] =
                            MaybeSecretCard::Secret(to_bucket_player);

                        self.context
                            .mutate_secret(to_bucket_player, move |secret, _, _| {
                                secret
                                    .cards
                                    .insert(attachment_instance.id, attachment_instance.clone());
                            });
                    }
                }
            }
        }

        match location {
            Some(PublicLocation::PublicAttachment { parent }) => {
                self.game.cards[usize::from(parent)]
                    .expect_mut("Card should have been attached to a public parent")
                    .attachment = None;
            }
            Some(location) => {
                self.game.player_mut(owner).remove_from(location);
            }
            None => (),
        }
    }

    /// Attaches a card to a parent card, dusting the parent's old attachment if necessary.
    ///
    /// This process is very similar to move_card, but it must also figure out the destination parent's ID
    /// to be able to attach to it.
    ///
    /// 1. Dust parent's current attachment, if any.
    /// 2. Remove card from its current zone.
    /// 3. Remove card from its current bucket.
    /// 4. Add card to parent's bucket.
    /// 5. Set parent's attachment to card.
    ///
    /// Step 5 is the inverse of step 2.
    /// Step 4 is the inverse of step 3.
    ///
    /// This can't be a regular async function, otherwise we get a recursive type,
    /// because this calls move_card to dust, and move_card calls attach_card to attach.
    fn attach_card(
        &mut self,
        card: OpaquePointer,
        parent: OpaquePointer,
    ) -> Pin<Box<dyn '_ + Future<Output = ()>>> {
        Box::pin(async move {
            let card_bucket = self.reveal_card_bucket(card).await;

            let parent_bucket = self.reveal_card_bucket(parent).await;

            // Dust parent's current attachment, if any.

            let parent_id = match parent_bucket {
                Bucket::Public => {
                    let id = self.reveal_id(parent).await;

                    if let Some(attachment_id) = self.game.cards[usize::from(id)]
                        .expect_ref("Parent card should have been public")
                        .attachment
                    {
                        let attachment_owner = if self.game.does_own(0, attachment_id) {
                            0
                        } else if self.game.does_own(1, attachment_id) {
                            1
                        } else {
                            unreachable!(
                                "No player publicly owns ID {:?}, but its bucket is public",
                                id
                            );
                        };
                        let attachment = self.new_public_pointer(attachment_id);
                        self.move_card(attachment, attachment_owner, Zone::Dusted { public: true })
                            .await
                    }

                    Some(id)
                }
                Bucket::Secret(parent_card_player) => {
                    match self.game.opaque_ptrs[usize::from(parent)] {
                        MaybeSecretID::Secret(ptr_player) if ptr_player == parent_card_player => {
                            self.context
                                .mutate_secret(parent_card_player, |secret, _, log| {
                                    let id = secret.opaque_ptrs[&parent];

                                    if let Some(attachment) = secret.cards[&id].attachment {
                                        secret.dust_secretly(attachment, log);
                                    }
                                });

                            None
                        }
                        MaybeSecretID::Secret(..) => {
                            let id = self.reveal_id(parent).await;

                            self.context
                                .mutate_secret(parent_card_player, |secret, _, log| {
                                    if let Some(attachment) = secret.cards[&id].attachment {
                                        secret.dust_secretly(attachment, log);
                                    }
                                });

                            Some(id)
                        }
                        MaybeSecretID::Public(id) => {
                            self.context
                                .mutate_secret(parent_card_player, |secret, _, log| {
                                    if let Some(attachment) = secret.cards[&id].attachment {
                                        secret.dust_secretly(attachment, log);
                                    }
                                });

                            Some(id)
                        }
                    }
                }
            };

            // Remove card from its current zone, secretly and possibly publicly.

            let card_id = match self.game.opaque_ptrs[usize::from(card)] {
                MaybeSecretID::Secret(card_ptr_player) => {
                    if Bucket::Secret(card_ptr_player) == card_bucket {
                        None
                    } else {
                        Some(self.reveal_id(card).await)
                    }
                }
                MaybeSecretID::Public(card_id) => Some(card_id),
            };

            if let (card_owner, Some(card_location)) = self.reveal_id_location(card).await {
                self.game.player_mut(card_owner).remove_from(card_location);
            }

            if let Some(card_id) = card_id {
                self.game.remove_id(card_id);
            }

            for player in 0..2 {
                self.context.mutate_secret(player, |secret, _, _| {
                    if let Some(card_id) =
                        card_id.or_else(|| secret.opaque_ptrs.get(&card).copied())
                    {
                        secret.remove_id(card_id);
                    }
                });
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
                                self.game.opaque_ptrs[usize::from(card)]
                                    .player()
                                    .expect("Card pointer should be secret"),
                                move |secret| secret.opaque_ptrs[&card],
                                |_| true,
                            )
                            .await
                    }
                    Some(card_id) => card_id,
                };

                let instance = match card_bucket {
                    Bucket::Public => {
                        let parent_bucket_player = parent_bucket
                            .player()
                            .expect("parent bucket isn't public, but also not a player's secret");

                        std::mem::replace(
                            &mut self.game.cards[usize::from(card_id)],
                            MaybeSecretCard::Secret(parent_bucket_player),
                        )
                        .expect("the card was public but wasn't in the global array")
                    }
                    Bucket::Secret(card_bucket_player) => {
                        let instance = self
                            .context
                            .reveal_unique(
                                card_bucket_player,
                                move |secret| secret.cards[&card_id].clone(),
                                |_| true,
                            )
                            .await;

                        self.context
                            .mutate_secret(card_bucket_player, |secret, _, _| {
                                secret.cards.remove(&card_id);
                            });

                        instance
                    }
                };

                // Step 4:
                // Add card to parent's bucket.
                match parent_bucket {
                    Bucket::Public => {
                        self.game.cards[usize::from(card_id)] = MaybeSecretCard::Public(instance);
                    }
                    Bucket::Secret(parent_bucket_player) => {
                        self.game.cards[usize::from(card_id)] =
                            MaybeSecretCard::Secret(parent_bucket_player);

                        self.context
                            .mutate_secret(parent_bucket_player, |secret, _, _| {
                                secret.cards.insert(card_id, instance.clone());
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
                    let parent_bucket_player = parent_bucket.player();
                    if let Some(parent_bucket_player) = parent_bucket_player {
                        if card_bucket != Bucket::Secret(parent_bucket_player) {
                            Some(
                                self
                                    .context
                                    .reveal_unique(
                                        card_bucket.player().expect("We would have had a card_id if the card was in the public bucket"),
                                        move |secret| {
                                            secret.opaque_ptrs[&card]
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
                        .player()
                        .expect("Parent pointer and card are both in some player's secret");

                    self.context
                        .mutate_secret(parent_bucket_player, |secret, _, _| {
                            let card_id = card_id.unwrap_or_else(|| secret.opaque_ptrs[&card]);

                            secret.cards[&secret.opaque_ptrs[&parent]].attachment = Some(card_id);
                        });
                }
                Some(parent_id) => match parent_bucket {
                    Bucket::Public => {
                        let card_id = match card_id {
                            None => self.reveal_id(card).await,
                            Some(card_id) => card_id,
                        };

                        self.game.cards[usize::from(parent_id)].expect_mut("If the parent bucket is public, the public state must have that card").attachment = Some(card_id);
                    }
                    Bucket::Secret(parent_bucket_player) => {
                        self.context
                            .mutate_secret(parent_bucket_player, |secret, _, _| {
                                let card_id = card_id.unwrap_or_else(|| secret.opaque_ptrs[&card]);

                                secret.cards[&parent_id].attachment = Some(card_id);
                            })
                    }
                },
            }
        })
    }

    /// Reveals the card's location in public state.
    async fn reveal_id_location(
        &mut self,
        card: OpaquePointer,
    ) -> (Player, Option<PublicLocation>) {
        let card_bucket = self.reveal_card_bucket(card).await;

        let card_id = match self.game.opaque_ptrs[usize::from(card)] {
            MaybeSecretID::Secret(card_ptr_player) => {
                if Bucket::Secret(card_ptr_player) == card_bucket {
                    None
                } else {
                    Some(self.reveal_id(card).await)
                }
            }
            MaybeSecretID::Public(id) => Some(id),
        };

        match card_id {
            None => {
                // Card ID and card are both in the same player's secret
                let player = self.game.opaque_ptrs[usize::from(card)]
                    .player()
                    .expect("Should have the player whose secret has both the card ID and card");

                (
                    player,
                    self.context
                        .reveal_unique(
                            player,
                            move |secret| secret.id_location(secret.opaque_ptrs[&card]),
                            |_| true,
                        )
                        .await,
                )
            }
            Some(card_id) => match card_bucket {
                Bucket::Public => {
                    let owner = (0u8..2)
                        .find(|p| self.game.does_own(*p, card_id))
                        .expect("The card is in a public bucket, but nobody claims ownership.");

                    let location = self.game.player(owner).id_location(card_id)
                    .or_else(|| self.game.cards.iter().flat_map(MaybeSecretCard::instance_ref).find_map(|instance| if instance.attachment == Some(card_id) {
                        Some(instance.id)
                    } else {
                        None
                    }.map(|parent| PublicLocation::PublicAttachment { parent }))).expect("Bucket is None, player state claims that it has card, but somehow we couldn't find it in any zones.");

                    (owner, Some(location))
                }
                Bucket::Secret(player) => (
                    player,
                    self.context
                        .reveal_unique(player, move |secret| secret.id_location(card_id), |_| true)
                        .await,
                ),
            },
        }
    }

    async fn reveal_id(&mut self, card: OpaquePointer) -> InstanceID {
        match self.game.opaque_ptrs[usize::from(card)] {
            MaybeSecretID::Secret(player) => {
                let id = self
                    .context
                    .reveal_unique(player, move |secret| secret.opaque_ptrs[&card], |_| true)
                    .await;

                self.publish_pointer_id(card, player, id);

                id
            }
            MaybeSecretID::Public(id) => id,
        }
    }

    /// Checks if a card's player and zone satisfy the given conditions.
    pub async fn reveal_if_zone(
        &mut self,
        card: OpaquePointer,
        f: impl Fn(Player, Zone) -> bool + Clone + 'static,
    ) -> bool {
        match self.game.opaque_ptrs[usize::from(card)] {
            MaybeSecretID::Secret(player) => {
                let (result, id) = self
                    .context
                    .reveal_unique(
                        player,
                        {
                            let f = f.clone();
                            move |secret| {
                                let id = secret.opaque_ptrs[&card];

                                if secret.deck.iter().any(|deck_id| *deck_id == id) {
                                    (Some(f(player, Zone::Deck)), None)
                                } else if secret.hand.iter().any(|hand_id| *hand_id == Some(id)) {
                                    (Some(f(player, Zone::Hand { public: false })), None)
                                } else if secret.limbo.iter().any(|limbo_id| *limbo_id == id) {
                                    (Some(f(player, Zone::Limbo { public: false })), None)
                                } else if secret.dusted.iter().any(|dusted_id| *dusted_id == id) {
                                    (Some(f(player, Zone::Dusted { public: false })), None)
                                } else if secret
                                    .card_selection
                                    .iter()
                                    .any(|card_selection_id| *card_selection_id == id)
                                {
                                    (Some(f(player, Zone::CardSelection)), None)
                                } else {
                                    (None, Some(id))
                                }
                            }
                        },
                        |_| true,
                    )
                    .await;

                match result {
                    None => {
                        let id = id.expect("No result, but also no instance ID");

                        for player in 0..2 {
                            let player_state = self.game.player(player);

                            let zone = if player_state
                                .hand
                                .iter()
                                .any(|deck_id| *deck_id == Some(id))
                            {
                                Some(Zone::Hand { public: true })
                            } else if player_state.field.iter().any(|field_id| *field_id == id) {
                                Some(Zone::Field)
                            } else if player_state
                                .graveyard
                                .iter()
                                .any(|graveyard_id| *graveyard_id == id)
                            {
                                Some(Zone::Graveyard)
                            } else if player_state.limbo.iter().any(|limbo_id| *limbo_id == id) {
                                Some(Zone::Limbo { public: true })
                            } else if player_state
                                .casting
                                .iter()
                                .any(|casting_id| *casting_id == id)
                            {
                                Some(Zone::Casting)
                            } else if player_state.dusted.iter().any(|dusted_id| *dusted_id == id) {
                                Some(Zone::Dusted { public: true })
                            } else {
                                None
                            };

                            if let Some(zone) = zone {
                                return f(player, zone);
                            }
                        }

                        for player in 0..2 {
                            let result =
                                self.context
                                    .reveal_unique(
                                        player,
                                        {
                                            let f = f.clone();
                                            move |secret| {
                                                if secret.deck.iter().any(|deck_id| *deck_id == id)
                                                {
                                                    Some(f(player, Zone::Deck))
                                                } else if secret
                                                    .hand
                                                    .iter()
                                                    .any(|hand_id| *hand_id == Some(id))
                                                {
                                                    Some(f(player, Zone::Hand { public: false }))
                                                } else if secret
                                                    .limbo
                                                    .iter()
                                                    .any(|limbo_id| *limbo_id == id)
                                                {
                                                    Some(f(player, Zone::Limbo { public: false }))
                                                } else if secret
                                                    .dusted
                                                    .iter()
                                                    .any(|dusted_id| *dusted_id == id)
                                                {
                                                    Some(f(player, Zone::Dusted { public: false }))
                                                } else if secret.card_selection.iter().any(
                                                    |card_selection_id| *card_selection_id == id,
                                                ) {
                                                    Some(f(player, Zone::CardSelection))
                                                } else {
                                                    None
                                                }
                                            }
                                        },
                                        |_| true,
                                    )
                                    .await;

                            if let Some(result) = result {
                                return result;
                            }
                        }

                        unreachable!(
                            "Instance ID does not exist in any player's public or secret state"
                        );
                    }
                    Some(result) => result,
                }
            }
            MaybeSecretID::Public(id) => {
                for player in 0..2 {
                    let player_state = self.game.player(player);

                    let zone = if player_state.hand.iter().any(|deck_id| *deck_id == Some(id)) {
                        Some(Zone::Hand { public: true })
                    } else if player_state.field.iter().any(|field_id| *field_id == id) {
                        Some(Zone::Field)
                    } else if player_state
                        .graveyard
                        .iter()
                        .any(|graveyard_id| *graveyard_id == id)
                    {
                        Some(Zone::Graveyard)
                    } else if player_state.limbo.iter().any(|limbo_id| *limbo_id == id) {
                        Some(Zone::Limbo { public: true })
                    } else if player_state
                        .casting
                        .iter()
                        .any(|casting_id| *casting_id == id)
                    {
                        Some(Zone::Casting)
                    } else if player_state.dusted.iter().any(|dusted_id| *dusted_id == id) {
                        Some(Zone::Dusted { public: true })
                    } else {
                        None
                    };

                    if let Some(zone) = zone {
                        return f(player, zone);
                    }
                }

                for player in 0..2 {
                    let result = self
                        .context
                        .reveal_unique(
                            player,
                            {
                                let f = f.clone();
                                move |secret| {
                                    if secret.deck.iter().any(|deck_id| *deck_id == id) {
                                        Some(f(player, Zone::Deck))
                                    } else if secret.hand.iter().any(|hand_id| *hand_id == Some(id))
                                    {
                                        Some(f(player, Zone::Hand { public: false }))
                                    } else if secret.limbo.iter().any(|limbo_id| *limbo_id == id) {
                                        Some(f(player, Zone::Limbo { public: false }))
                                    } else if secret.dusted.iter().any(|dusted_id| *dusted_id == id)
                                    {
                                        Some(f(player, Zone::Dusted { public: false }))
                                    } else if secret
                                        .card_selection
                                        .iter()
                                        .any(|card_selection_id| *card_selection_id == id)
                                    {
                                        Some(f(player, Zone::CardSelection))
                                    } else {
                                        None
                                    }
                                }
                            },
                            |_| true,
                        )
                        .await;

                    if let Some(result) = result {
                        return result;
                    }
                }

                unreachable!("Instance ID does not exist in any player's public or secret state");
            }
        }
    }

    /// Reveals data about a card.
    pub async fn reveal_from_card<
        T: arcadeum::store::Secret + serde::Serialize + serde::de::DeserializeOwned + Debug,
    >(
        &mut self,
        card: OpaquePointer,
        f: impl Fn(&CardInstance<<<S as State>::Secret as Secret>::BaseCard>) -> T + Clone + 'static,
    ) -> T {
        match self.game.opaque_ptrs[usize::from(card)] {
            MaybeSecretID::Secret(ptr_player) => {
                let card_buckets: Vec<_> = self.game.card_buckets().collect();

                // We're going to reveal either the data or where to find it.

                let (data, location) = self
                    .context
                    .reveal_unique(
                        ptr_player,
                        {
                            let f = f.clone();

                            move |secret| {
                                let id = secret.opaque_ptrs[&card];
                                let card_bucket = card_buckets[usize::from(id)];

                                // We don't need to reveal the location if the data is already here.

                                if card_bucket == Bucket::Secret(ptr_player) {
                                    (Some(f(&secret.cards[&id])), None)
                                } else {
                                    (None, Some((id, card_bucket)))
                                }
                            }
                        },
                        |_| true,
                    )
                    .await;

                match data {
                    None => {
                        let (id, card_bucket) = location.expect("no data and nowhere to find it");

                        self.publish_pointer_id(card, ptr_player, id);

                        match card_bucket {
                            Bucket::Public => {
                                // The secret pointer pointed to a public card.

                                f(self.game.cards[usize::from(id)]
                                    .expect_ref("the card should have been public"))
                            }
                            Bucket::Secret(card_bucket_player) => {
                                // The secret pointer pointed to the other player's secret card.

                                self.context
                                    .reveal_unique(
                                        card_bucket_player,
                                        move |secret| f(&secret.cards[&id]),
                                        |_| true,
                                    )
                                    .await
                            }
                        }
                    }
                    Some(data) => data,
                }
            }
            MaybeSecretID::Public(id) => match &self.game.cards[usize::from(id)] {
                MaybeSecretCard::Secret(card_bucket_player) => {
                    self.context
                        .reveal_unique(
                            *card_bucket_player,
                            move |secret| f(&secret.cards[&id]),
                            |_| true,
                        )
                        .await
                }
                MaybeSecretCard::Public(instance) => f(instance),
            },
        }
    }

    /// Modifies a card.
    ///
    /// If the card is public, it's modified publicly.
    /// If the card is secret, it's modified secretly.
    pub async fn modify_card(
        &mut self,
        card: OpaquePointer,
        f: impl Fn(&mut CardInstance<<<S as State>::Secret as Secret>::BaseCard>),
    ) {
        match self.game.opaque_ptrs[usize::from(card)] {
            MaybeSecretID::Secret(ptr_player) => {
                let card_buckets: Vec<_> = self.game.card_buckets().collect();

                // We're going to reveal which bucket the card is in.

                // If it isn't in the same bucket as the pointer, we have to reveal the ID.

                let (card_bucket, id) = self
                    .context
                    .reveal_unique(
                        ptr_player,
                        move |secret| {
                            let id = secret.opaque_ptrs[&card];
                            let card_bucket = card_buckets[usize::from(id)];

                            // We don't need to reveal the ID if the card instance is in the same bucket as the pointer.

                            if card_bucket == Bucket::Secret(ptr_player) {
                                (card_bucket, None)
                            } else {
                                (card_bucket, Some(id))
                            }
                        },
                        |_| true,
                    )
                    .await;

                if card_bucket == Bucket::Secret(ptr_player) {
                    // Player-internal mutation

                    self.context.mutate_secret(ptr_player, |secret, _, _| {
                        f(&mut secret.cards[&secret.opaque_ptrs[&card]]);
                    });
                } else if card_bucket == Bucket::Public {
                    // Public card mutation

                    let id = id.expect("no ID was revealed while modifying a public card");

                    self.publish_pointer_id(card, ptr_player, id);

                    f(self.game.cards[usize::from(id)]
                        .expect_mut("the card should have been public"));
                } else if let Bucket::Secret(card_bucket_player) = card_bucket {
                    // Cross-player mutation

                    let id = id
                        .expect("no ID was revealed while modifying another player's secret card");

                    self.publish_pointer_id(card, ptr_player, id);

                    self.context
                        .mutate_secret(card_bucket_player, |secret, _, _| {
                            f(&mut secret.cards[&id]);
                        });
                }
            }
            MaybeSecretID::Public(id) => match &mut self.game.cards[usize::from(id)] {
                MaybeSecretCard::Secret(card_bucket_player) => {
                    self.context
                        .mutate_secret(*card_bucket_player, |secret, _, _| {
                            f(&mut secret.cards[&id]);
                        });
                }
                MaybeSecretCard::Public(instance) => {
                    f(instance);
                }
            },
        }
    }

    /// Creates new secret cards in a player's limbo.
    ///
    /// Cards can be created in the provided closure using [CardGameSecret::new_card].
    ///
    /// # Arguments
    ///
    /// * `player` - The player whose secret state will have the cards created in
    /// * `mutate` - A closure to mutate `player`'s secret state, using [CardGameSecret::new_card] to create new cards
    ///
    /// # Return value
    ///
    /// A vector of [OpaquePointer]s to the constructed cards
    pub async fn new_secret_cards(
        &mut self,
        player: Player,
        mutate: impl Fn(
            &mut CardGameSecret<<S as State>::Secret>,
            &mut dyn rand::RngCore,
            &mut dyn FnMut(&dyn Event),
        ),
    ) -> Vec<OpaquePointer> {
        let next_id = InstanceID::from_raw(self.game.cards.len());
        let next_ptr = OpaquePointer::from_raw(self.game.opaque_ptrs.len());

        self.context.mutate_secret(player, |secret, random, log| {
            secret.next_id = next_id;
            secret.next_ptr = next_ptr;

            mutate(secret, random, log);
        });

        let (next_id, next_ptr) = self
            .context
            .reveal_unique(player, |secret| (secret.next_id, secret.next_ptr), |_| true)
            .await;

        // This will create some [MaybeSecretCard::Secret]s pointing to non-existent cards in a player's secret.
        // This is required to prevent leaking information about secretly instantiated cards having attachments or not.
        // This is safe as long as we don't construct pointers to arbitrary instance IDs since non-existent cards are never added to a zone.

        while self.game.cards.len() < next_id.into() {
            self.game.cards.push(MaybeSecretCard::Secret(player));
        }

        let mut cards = Vec::new();

        while self.game.opaque_ptrs.len() < next_ptr.into() {
            // All cards created within the mutate closure are assigned an opaque reference.
            // Child cards are not, and we don't want to publicly assign them to any zone.

            cards.push(OpaquePointer::from_raw(self.game.opaque_ptrs.len()));

            self.game.opaque_ptrs.push(MaybeSecretID::Secret(player));
        }

        cards
    }

    /// Requests a player's secret information.
    ///
    /// The random number generator is re-seeded after this call to prevent players from influencing the randomness of the state via trial and error.
    ///
    /// See [LiveGame::reveal_unique] for a faster non-re-seeding version of this method.
    pub async fn reveal<T: arcadeum::store::Secret + Debug>(
        &mut self,
        player: Player,
        reveal: impl Fn(&CardGameSecret<<S as State>::Secret>) -> T + 'static,
        verify: impl Fn(&T) -> bool + 'static,
    ) -> T {
        self.context.reveal(player, reveal, verify).await
    }

    /// Requests a player's secret information.
    ///
    /// The random number generator is not re-seeded after this call, so care must be taken to guarantee that the verify function accepts only one possible input.
    /// Without this guarantee, players can influence the randomness of the state via trial and error.
    ///
    /// See [LiveGame::reveal] for a slower re-seeding version of this method.
    pub async fn reveal_unique<T: arcadeum::store::Secret + Debug>(
        &mut self,
        player: Player,
        reveal: impl Fn(&CardGameSecret<<S as State>::Secret>) -> T + 'static,
        verify: impl Fn(&T) -> bool + 'static,
    ) -> T {
        self.context.reveal_unique(player, reveal, verify).await
    }

    /// Constructs a random number generator via commit-reveal.
    pub async fn random(&mut self) -> impl rand::Rng {
        self.context.random().await
    }

    /// Logs an event.
    pub fn log(&mut self, event: &impl Event) {
        self.context.log(event)
    }

    #[cfg(debug_assertions)]
    /// Reveals secrets and checks if the state is valid.
    ///
    /// Do not use this in production.
    /// It reveals complete secrets for both players.
    /// Use this only for debugging.
    pub async fn reveal_ok(&mut self) -> Result<(), String> {
        let secrets = {
            let secret0 = self
                .context
                .reveal_unique(0, |secret| secret.clone(), |_| true)
                .await;

            let secret1 = self
                .context
                .reveal_unique(1, |secret| secret.clone(), |_| true)
                .await;

            [secret0, secret1]
        };

        // make sure we don't mutate self anywhere in this function by taking an immutable reference
        // that lasts until the end of the function.
        let game = &self;

        // Only one bucket may contain the CardInstance for an InstanceID.
        // If a CardInstance has an attachment, the attachment must be in the same Bucket.
        let real_instance_ids = self
            .game
            .cards
            .iter()
            .flat_map(|msc| msc.instance_ref().map(|instance| instance.id))
            .chain(
                secrets
                    .iter()
                    .flat_map(|secret| secret.cards.keys().copied()),
            );

        for id in real_instance_ids {
            match &self.game.cards[usize::from(id)] {
                MaybeSecretCard::Secret(player) => {
                    // The card must be in that player's secret cards

                    if !secrets[usize::from(*player)].cards.contains_key(&id) {
                        return Err(format!(
                            "Card should have been in player {}'s secret",
                            player
                        ));
                    }

                    // The card must not be in the other player's secret cards

                    if secrets[usize::from(1 - *player)].cards.contains_key(&id) {
                        return Err(format!(
                            "Card should not have been in player {}'s secret",
                            1 - player
                        ));
                    }

                    // The instance's attachment, if any, should also be in this player's secret.
                    if let Some(attachment_id) = secrets[usize::from(*player)]
                        .cards
                        .get(&id)
                        .unwrap()
                        .attachment
                    {
                        if !secrets[usize::from(*player)]
                            .cards
                            .contains_key(&attachment_id)
                        {
                            return Err(format!(
                                "Card's attachment should have been in player {}'s secret",
                                player
                            ));
                        }
                    }
                }
                MaybeSecretCard::Public(instance) => {
                    // The card shouldn't be in either player's secret cards

                    for (player_id, secret) in secrets.iter().enumerate() {
                        if secret.cards.contains_key(&id) {
                            return Err(format!(
                                "InstanceID {:?} is both public and in player {:?}'s secret",
                                id, player_id
                            ));
                        }
                    }

                    // The instance's attachment, if any, should also be public.

                    if let Some(attachment) = instance.attachment {
                        if let MaybeSecretCard::Secret(player) =
                            self.game.cards[usize::from(attachment)]
                        {
                            return Err(format!("The instance for card {} is public, but its attachment {} is in player {}'s secret", usize::from(id), usize::from(attachment), player));
                        }
                    }
                }
            }
        }

        // An InstanceID must occur in all zones combined at most once.
        // It can be 0, because some InstanceIDs correspond to non-existent attachments.
        for id in 0..self.game.cards.len() {
            let id = InstanceID::from_raw(id);

            // Count the number of times id occurs in public and secret state.

            let mut count = 0;

            for (player_id, player) in self.game.players.iter().enumerate() {
                let player_id: Player =
                    player_id.try_into().map_err(|error| format!("{}", error))?;

                count += player
                    .hand
                    .iter()
                    .filter(|hand_id| **hand_id == Some(id))
                    .count();
                count += player
                    .field
                    .iter()
                    .filter(|field_id| **field_id == id)
                    .count();
                count += player
                    .graveyard
                    .iter()
                    .filter(|graveyard_id| **graveyard_id == id)
                    .count();
                count += player
                    .limbo
                    .iter()
                    .filter(|limbo_id| **limbo_id == id)
                    .count();
                count += player
                    .casting
                    .iter()
                    .filter(|casting_id| **casting_id == id)
                    .count();
                count += player
                    .dusted
                    .iter()
                    .filter(|dusted_id| **dusted_id == id)
                    .count();
                count += self
                    .game
                    .cards
                    .iter()
                    .filter(|msc| {
                        if let MaybeSecretCard::Public(parent_instance) = msc {
                            parent_instance.attachment == Some(id)
                                && self.game.does_own(player_id, parent_instance.id)
                        } else {
                            false
                        }
                    })
                    .count();
            }

            for secret in &secrets {
                count += secret.deck.iter().filter(|deck_id| **deck_id == id).count();
                count += secret
                    .hand
                    .iter()
                    .filter(|hand_id| **hand_id == Some(id))
                    .count();
                count += secret
                    .limbo
                    .iter()
                    .filter(|limbo_id| **limbo_id == id)
                    .count();
                count += secret
                    .dusted
                    .iter()
                    .filter(|dusted_id| **dusted_id == id)
                    .count();
                count += secret
                    .card_selection
                    .iter()
                    .filter(|card_selection_id| **card_selection_id == id)
                    .count();
                count += secret
                    .cards
                    .values()
                    .filter(|instance| instance.attachment == Some(id))
                    .count();
            }

            if count > 1 {
                return Err(format!(
                    "Instance ID {} occurs {} times in public and secret state",
                    usize::from(id),
                    count
                ));
            }
        }

        // Public state deck must match secret state deck length.
        for (player_id, player) in self.game.players.iter().enumerate() {
            if secrets[player_id].deck.len() != player.deck {
                return Err(format!(
                    "Player {}'s public deck size is {}, but their private deck size is {}.",
                    player_id,
                    player.deck,
                    secrets[player_id].deck.len()
                ));
            }
        }

        // Public state card selection must match secret state card selection length.
        for (player_id, player) in self.game.players.iter().enumerate() {
            if secrets[player_id].card_selection.len() != player.card_selection {
                return Err(format!("Player {}'s public card selection size is {}, but their private card selection size is {}.", player_id, player.card_selection, secrets[player_id].card_selection.len()));
            }
        }

        // For each card in Public & Secret hand, if one Bucket has None, the other must have Some(ID).
        for (player, secret) in self.game.players.iter().zip(secrets.iter()) {
            for (index, (public_hand, secret_hand)) in
                player.hand.iter().zip(secret.hand.iter()).enumerate()
            {
                match (public_hand, secret_hand) {
                    (Some(_), None) | (None, Some(_)) => {
                        // ok! only one state has it
                    }
                    (Some(public_some), Some(private_some)) => {
                        return Err(format!("Both public state & private state({:?}) have Some(_) at hand position {:?} .\nPublic: Some({:?}), Private: Some({:?})", player, index, public_some, private_some));
                    }
                    (None, None) => {
                        return Err(
                            "Both public & private state have None at this hand position."
                                .to_string(),
                        );
                    }
                }
            }
        }

        for id in self
            .game
            .cards
            .iter()
            .flat_map(|card| card.instance_ref().map(|instance| instance.id))
        {
            if !game.game.does_own(0, id) && !game.game.does_own(1, id) {
                return Err(format!("Nobody claims to own public InstanceID {:?}", id));
            }
        }

        Ok(())
    }

    /// Creates a public opaque pointer to a concrete instance ID.
    fn new_public_pointer(&mut self, id: InstanceID) -> OpaquePointer {
        let ptr = OpaquePointer::from_raw(self.game.opaque_ptrs.len());

        self.game.opaque_ptrs.push(MaybeSecretID::Public(id));

        ptr
    }

    /// Moves a player's secret pointer to public state.
    fn publish_pointer_id(&mut self, ptr: OpaquePointer, player: Player, id: InstanceID) {
        self.context.mutate_secret(player, |secret, _, _| {
            match secret.opaque_ptrs.remove(&ptr) {
                None => unreachable!("pointer doesn't belong to player"),
                Some(ptr_id) => {
                    if id != ptr_id {
                        unreachable!("published pointer with wrong ID");
                    }
                }
            }
        });

        self.game.opaque_ptrs[usize::from(ptr)] = MaybeSecretID::Public(id);
    }

    async fn reveal_card_bucket(&mut self, card: OpaquePointer) -> Bucket {
        match self.game.opaque_ptrs[usize::from(card)] {
            MaybeSecretID::Secret(player) => {
                let buckets: Vec<_> = self.game.card_buckets().collect();

                self.context
                    .reveal_unique(
                        player,
                        move |secret| buckets[usize::from(secret.opaque_ptrs[&card])],
                        |_| true,
                    )
                    .await
            }
            MaybeSecretID::Public(id) => match self.game.cards[usize::from(id)] {
                MaybeSecretCard::Secret(player) => Bucket::Secret(player),
                MaybeSecretCard::Public(..) => Bucket::Public,
            },
        }
    }

    /// Moves a pointer to a bucket.
    ///
    /// Do not use this in production.
    /// Use this only for debugging.
    #[cfg(debug_assertions)]
    pub async fn move_pointer(&mut self, ptr: OpaquePointer, bucket: &Option<Player>) {
        let ptr_bucket = self.game.opaque_ptrs[usize::from(ptr)].player();

        if ptr_bucket != *bucket {
            let id = match ptr_bucket {
                None => self.game.opaque_ptrs[usize::from(ptr)].expect("Pointer should be public"),
                Some(ptr_player) => {
                    let id = self
                        .context
                        .reveal_unique(ptr_player, move |secret| secret.opaque_ptrs[&ptr], |_| true)
                        .await;

                    self.context.mutate_secret(ptr_player, |secret, _, _| {
                        secret.opaque_ptrs.remove(&ptr);
                    });

                    id
                }
            };

            match bucket {
                None => {
                    self.game.opaque_ptrs[usize::from(ptr)] = MaybeSecretID::Public(id);
                }
                Some(bucket_player) => {
                    self.game.opaque_ptrs[usize::from(ptr)] = MaybeSecretID::Secret(*bucket_player);

                    self.context.mutate_secret(*bucket_player, |secret, _, _| {
                        secret.opaque_ptrs.insert(ptr, id);
                    });
                }
            }
        }
    }

    /// Reveals secrets and prints to stdout.
    ///
    /// Do not use this in production.
    /// Use this only for debugging.
    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub async fn print(&mut self) {
        let secrets = {
            let secret0 = self
                .context
                .reveal_unique(0, |secret| secret.clone(), |_| true)
                .await;

            let secret1 = self
                .context
                .reveal_unique(1, |secret| secret.clone(), |_| true)
                .await;

            [secret0, secret1]
        };

        println!();
        println!(
            "================================================================================"
        );
        println!();
        println!(
            "--------------------------------- Shared state ---------------------------------"
        );
        println!();
        println!("{:#?}", self.game);
        println!();
        println!(
            "----------------------------------- Secret 0 -----------------------------------"
        );
        println!();
        println!("{:#?}", secrets[0]);
        println!();
        println!(
            "----------------------------------- Secret 1 -----------------------------------"
        );
        println!();
        println!("{:#?}", secrets[1]);
        println!();
        println!(
            "================================================================================"
        );
        println!();
    }
}

impl<S: State> Deref for LiveGame<S> {
    type Target = CardGame<S>;

    fn deref(&self) -> &Self::Target {
        &self.game
    }
}

impl<S: State> DerefMut for LiveGame<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.game
    }
}

/// A card game's public state.
#[derive(serde::Serialize, serde::Deserialize, Clone, Default, Debug)]
pub struct CardGame<S: State> {
    #[serde(bound = "S: State")]
    cards: Vec<MaybeSecretCard<<<S as State>::Secret as Secret>::BaseCard>>,

    opaque_ptrs: Vec<MaybeSecretID>,

    players: [PlayerState; 2],

    /// Any additional public information that a game implementing [State] needs to track.
    #[serde(bound = "S: State")]
    pub state: S,
}

impl<S: State> CardGame<S> {
    /// Gets the public state of the given player.
    pub fn player(&self, player: Player) -> &PlayerState {
        &self.players[usize::from(player)]
    }

    /// Gets the mutable public state of the given player.
    pub fn player_mut(&mut self, player: Player) -> &mut PlayerState {
        &mut self.players[usize::from(player)]
    }

    /// Checks if the instance with the given instance ID belongs to a collection of the given player.
    pub fn does_own(&self, player: Player, id: InstanceID) -> bool {
        let player_state = self.player(player);

        player_state.hand.contains(&Some(id))
            || player_state.field.contains(&id)
            || player_state.graveyard.contains(&id)
            || player_state.limbo.contains(&id)
            || player_state.casting.contains(&id)
            || player_state.dusted.contains(&id)
            || self.cards.iter().any(|msc| {
                if let MaybeSecretCard::Public(parent_instance) = msc {
                    parent_instance.attachment == Some(id)
                        && self.does_own(player, parent_instance.id)
                } else {
                    false
                }
            })
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    /// Returns a boolean indicating whether the InstanceID is public.
    ///
    /// This function is only used in integration tests to make assertions about the internal state of the system.
    /// Regular consumers of this library should not use this function.
    pub fn is_card_public(&self, id: InstanceID) -> bool {
        match self.cards[usize::from(id)] {
            MaybeSecretCard::Public(_) => true,
            MaybeSecretCard::Secret(_) => false,
        }
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    /// Gets the ID for a public opaque pointer.
    ///
    /// Returns [None] if the pointer is secret.
    ///
    /// This function is only used in integration tests to make assertions about the internal state of the system.
    /// Regular consumers of this library should not use this function.
    pub fn id_for_pointer(&self, pointer: OpaquePointer) -> Option<InstanceID> {
        self.opaque_ptrs[usize::from(pointer)].id()
    }

    #[doc(hidden)]
    pub fn cards_len(&self) -> usize {
        self.cards.len()
    }

    #[doc(hidden)]
    pub fn new(state: S) -> Self {
        Self {
            cards: Default::default(),
            opaque_ptrs: Default::default(),
            players: Default::default(),
            state,
        }
    }

    fn remove_id(&mut self, id: InstanceID) {
        for player in &mut self.players {
            player.hand.retain(|hand_id| *hand_id != Some(id));
            player.field.retain(|field_id| *field_id != id);
            player.graveyard.retain(|graveyard_id| *graveyard_id != id);
            player.limbo.retain(|limbo_id| *limbo_id != id);
            player.casting.retain(|casting_id| *casting_id != id);
            player.dusted.retain(|dusted_id| *dusted_id != id);
        }

        for card in self.cards.iter_mut() {
            if let MaybeSecretCard::Public(instance) = card {
                if instance.attachment == Some(id) {
                    instance.attachment = None;
                }
            }
        }
    }

    fn card_buckets(&self) -> impl Iterator<Item = Bucket> + '_ {
        self.cards.iter().map(MaybeSecretCard::bucket)
    }
}

impl<S: State> arcadeum::store::State for CardGame<S> {
    type ID = S::ID;

    type Nonce = <S as State>::Nonce;

    type Action = <S as State>::Action;

    type Secret = CardGameSecret<<S as State>::Secret>;

    fn version() -> &'static [u8] {
        <S as State>::version()
    }

    fn challenge(address: &Address) -> String {
        <S as State>::challenge(address)
    }

    fn deserialize(data: &[u8]) -> Result<Self, String> {
        serde_cbor::from_slice(data)
            .map_err(|err| format!("failed to deserialize SkyWeaver - {:?}", err))
    }

    fn serialize(&self) -> Option<Vec<u8>> {
        serde_cbor::to_vec(self).ok()
    }

    fn is_serializable(&self) -> bool {
        true
    }

    fn verify(&self, player: Option<Player>, action: &Self::Action) -> Result<(), String> {
        self.state.verify(player, action)
    }

    fn apply(
        self,
        player: Option<Player>,
        action: &Self::Action,
        context: arcadeum::store::Context<Self>,
    ) -> Pin<Box<dyn Future<Output = (Self, arcadeum::store::Context<Self>)>>> {
        let action = action.clone();

        Box::pin(async move {
            let LiveGame { game, context } =
                S::apply(LiveGame::new(self, context), player, action).await;

            (game, context.0)
        })
    }
}

/// [State::apply] utilities
pub struct Context<S: arcadeum::store::State>(arcadeum::store::Context<S>);

impl<S: arcadeum::store::State> Context<S> {
    /// Create a new context. This is a debug-only function.
    #[cfg(debug_assertions)]
    pub fn new(context: arcadeum::store::Context<S>) -> Self {
        Self(context)
    }

    /// Mutates a player's secret information.
    pub fn mutate_secret(
        &mut self,
        player: crate::Player,
        mutate: impl Fn(&mut S::Secret, &mut dyn rand::RngCore, &mut dyn FnMut(&dyn Event)),
    ) {
        self.0.mutate_secret(player, mutate);
    }

    /// Requests a player's secret information.
    ///
    /// The random number generator is re-seeded after this call to prevent players from influencing the randomness of the state via trial and error.
    ///
    /// See [Context::reveal_unique] for a faster non-re-seeding version of this method.
    pub async fn reveal<T: arcadeum::store::Secret + Debug>(
        &mut self,
        player: crate::Player,
        reveal: impl Fn(&S::Secret) -> T + 'static,
        verify: impl Fn(&T) -> bool + 'static,
    ) -> T {
        #[cfg(feature = "reveal-backtrace")]
        let backtrace = format!("{}", std::backtrace::Backtrace::force_capture());

        self.0
            .reveal(
                player,
                move |secret| {
                    #[cfg(feature = "reveal-backtrace")]
                    {
                        let revealed = reveal(secret);
                        println!("reveal type: {}", std::any::type_name::<T>());
                        println!("reveal: {:#?}", &revealed);
                        println!("{}", backtrace);
                        revealed
                    }

                    #[cfg(not(feature = "reveal-backtrace"))]
                    {
                        reveal(secret)
                    }
                },
                verify,
            )
            .await
    }

    /// Requests a player's secret information.
    ///
    /// The random number generator is not re-seeded after this call, so care must be taken to guarantee that the verify function accepts only one possible input.
    /// Without this guarantee, players can influence the randomness of the state via trial and error.
    ///
    /// See [Context::reveal] for a slower re-seeding version of this method.
    pub async fn reveal_unique<T: arcadeum::store::Secret + Debug>(
        &mut self,
        player: crate::Player,
        reveal: impl Fn(&S::Secret) -> T + 'static,
        verify: impl Fn(&T) -> bool + 'static,
    ) -> T {
        #[cfg(feature = "reveal-backtrace")]
        let backtrace = format!("{}", std::backtrace::Backtrace::force_capture());

        self.0
            .reveal_unique(
                player,
                move |secret| {
                    #[cfg(feature = "reveal-backtrace")]
                    {
                        let revealed = reveal(secret);
                        println!("reveal type: {}", std::any::type_name::<T>());
                        println!("reveal: {:#?}", &revealed);
                        println!("{}", backtrace);
                        revealed
                    }

                    #[cfg(not(feature = "reveal-backtrace"))]
                    {
                        reveal(secret)
                    }
                },
                verify,
            )
            .await
    }

    /// Constructs a random number generator via commit-reveal.
    pub fn random(&mut self) -> impl Future<Output = impl rand::Rng> {
        self.0.random()
    }

    /// Logs an event.
    pub fn log(&mut self, event: &impl Event) {
        self.0.log(event);
    }
}

/// A player's public state.
#[derive(serde::Serialize, serde::Deserialize, Clone, Default, Debug)]
pub struct PlayerState {
    deck: usize,

    hand: Vec<Option<InstanceID>>,

    field: Vec<InstanceID>,

    graveyard: Vec<InstanceID>,

    limbo: Vec<InstanceID>,

    card_selection: usize,

    casting: Vec<InstanceID>,

    dusted: Vec<InstanceID>,
}

impl PlayerState {
    /// Gets the size of the player's deck.
    pub fn deck(&self) -> usize {
        self.deck
    }

    /// Gets the player's public portion of their hand.
    pub fn hand(&self) -> &Vec<Option<InstanceID>> {
        &self.hand
    }

    /// Gets the player's field.
    pub fn field(&self) -> &Vec<InstanceID> {
        &self.field
    }

    /// Gets the player's graveyard.
    pub fn graveyard(&self) -> &Vec<InstanceID> {
        &self.graveyard
    }

    /// Gets the player's public limbo.
    pub fn limbo(&self) -> &Vec<InstanceID> {
        &self.limbo
    }

    /// Gets the size of the player's card selection.
    pub fn card_selection(&self) -> usize {
        self.card_selection
    }

    /// Gets the player's casted cards.
    pub fn casting(&self) -> &Vec<InstanceID> {
        &self.casting
    }

    /// Gets the player's dusted cards.
    pub fn dusted(&self) -> &Vec<InstanceID> {
        &self.dusted
    }

    fn id_location(&self, id: InstanceID) -> Option<PublicLocation> {
        self.hand
            .iter()
            .position(|v| v == &Some(id))
            .map(PublicLocation::Hand)
            .or_else(|| {
                self.field
                    .iter()
                    .position(|v| v == &id)
                    .map(PublicLocation::Field)
            })
            .or_else(|| {
                self.graveyard
                    .iter()
                    .position(|v| v == &id)
                    .map(PublicLocation::Graveyard)
            })
            .or_else(|| {
                self.casting
                    .iter()
                    .position(|v| v == &id)
                    .map(PublicLocation::Casting)
            })
            .or_else(|| {
                self.limbo
                    .iter()
                    .position(|v| v == &id)
                    .map(PublicLocation::PublicLimbo)
            })
            .or_else(|| {
                self.dusted
                    .iter()
                    .position(|v| v == &id)
                    .map(PublicLocation::PublicDusted)
            })
    }

    fn remove_from(&mut self, location: PublicLocation) {
        match location {
            PublicLocation::Deck => {
                self.deck -= 1;
            }
            PublicLocation::Hand(index) => {
                self.hand.remove(index);
            }
            PublicLocation::Field(index) => {
                self.field.remove(index);
            }
            PublicLocation::Graveyard(index) => {
                self.graveyard.remove(index);
            }
            PublicLocation::PublicLimbo(index) => {
                self.limbo.remove(index);
            }
            PublicLocation::CardSelection => {
                self.card_selection -= 1;
            }
            PublicLocation::Casting(index) => {
                self.casting.remove(index);
            }
            PublicLocation::PublicDusted(index) => {
                self.dusted.remove(index);
            }
            PublicLocation::PublicAttachment { .. } => {
                unreachable!("PlayerState::remove_from can't remove an attachment");
            }
        }
    }
}

/// A player's secret state.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CardGameSecret<S: Secret> {
    #[serde(bound = "S: Secret")]
    cards: indexmap::IndexMap<InstanceID, CardInstance<S::BaseCard>>,

    opaque_ptrs: indexmap::IndexMap<OpaquePointer, InstanceID>,

    deck: Vec<InstanceID>,

    hand: Vec<Option<InstanceID>>,

    limbo: Vec<InstanceID>,

    dusted: Vec<InstanceID>,

    card_selection: Vec<InstanceID>,

    next_id: InstanceID,

    next_ptr: OpaquePointer,

    /// Any additional secret information that a game implementing [State] needs to track.
    #[serde(bound = "S: Secret")]
    pub state: S,
}

impl<S: Secret> CardGameSecret<S> {
    /// Constructs a new secret with secret card collections.
    pub fn new(state: S) -> Self {
        Self {
            cards: Default::default(),
            opaque_ptrs: Default::default(),
            deck: Default::default(),
            hand: Default::default(),
            limbo: Default::default(),
            dusted: Default::default(),
            card_selection: Default::default(),
            next_id: Default::default(),
            next_ptr: Default::default(),
            state,
        }
    }

    /// Gets all of the cards the player secretly knows.
    pub fn cards(&self) -> &indexmap::IndexMap<InstanceID, CardInstance<S::BaseCard>> {
        &self.cards
    }

    /// Gets all of the opaque pointers the player secretly knows.
    pub fn opaque_ptrs(&self) -> &indexmap::IndexMap<OpaquePointer, InstanceID> {
        &self.opaque_ptrs
    }

    /// Gets the player's deck.
    pub fn deck(&self) -> &Vec<InstanceID> {
        &self.deck
    }

    /// Gets the player's secret portion of their hand.
    pub fn hand(&self) -> &Vec<Option<InstanceID>> {
        &self.hand
    }

    /// Gets the player's secret limbo.
    pub fn limbo(&self) -> &Vec<InstanceID> {
        &self.limbo
    }

    /// Gets the player's secretly dusted cards.
    pub fn dusted(&self) -> &Vec<InstanceID> {
        &self.dusted
    }

    /// Gets the player's card selection.
    pub fn card_selection(&self) -> &Vec<InstanceID> {
        &self.card_selection
    }

    /// Creates a new secret card in a player's limbo.
    ///
    /// The card is created with its attachment if it has one.
    /// This can only be called from within a [LiveGame::new_secret_cards] closure.
    pub fn new_card(&mut self, base: <S as Secret>::BaseCard) -> InstanceID {
        // Always allocate two instance IDs, even if there isn't an attachment.

        let id = self.next_id;
        let attachment_id = InstanceID::from_raw(usize::from(self.next_id) + 1);
        self.next_id = InstanceID::from_raw(usize::from(self.next_id) + 2);

        // Allocate only one opaque pointer for the base card.
        // We can always call [LiveGame::attachment] to try to obtain its attachment.

        let ptr = self.next_ptr;
        self.next_ptr = OpaquePointer::from_raw(usize::from(self.next_ptr) + 1);

        let attachment = base.attachment().map(|attachment| {
            self.cards.insert(
                attachment_id,
                CardInstance {
                    id: attachment_id,
                    attachment: None,
                    card_state: attachment.new_card_state(),
                },
            );

            attachment_id
        });

        self.cards.insert(
            id,
            CardInstance {
                id,
                attachment,
                card_state: base.new_card_state(),
            },
        );

        self.opaque_ptrs.insert(ptr, id);

        self.limbo.push(id);

        id
    }

    /// Checks if the instance with the given instance ID exists in this secret.
    pub fn does_own(&self, id: InstanceID) -> bool {
        self.cards.contains_key(&id)
    }

    /// Secretly dusts a card either in the player's limbo, or attached to one of their cards.
    pub fn dust_secretly(&mut self, id: InstanceID, _log: impl FnMut(&dyn Event)) {
        // Remove the card from limbo or its parent.

        match self.limbo.iter().position(|limbo_id| *limbo_id == id) {
            None => {
                self.cards
                    .values_mut()
                    .find(|instance| instance.attachment == Some(id))
                    .expect("Instance ID is neither in secret limbo nor attached to a secret card")
                    .attachment = None;
            }
            Some(index) => {
                self.limbo.remove(index);
            }
        }

        // Add the card to dusted.

        self.dusted.push(id);

        // Log a dust event.

        // todo!();
    }

    /// Attaches a card to another without announcing it.
    ///
    /// The event isn't broadcasted publicly to all players.
    /// It's only logged by the player who owns the new parent card.
    pub fn attach_secretly(
        &mut self,
        parent: InstanceID,
        attachment: InstanceID,
        log: impl FnMut(&dyn Event),
    ) {
        // Dust parent's current attachment, if any.

        if let Some(attachment) = self.cards[&parent].attachment {
            self.dust_secretly(attachment, log);
        }

        // Attach the new attachment.

        self.cards[&parent].attachment = Some(attachment);

        // Log an attachment event.

        // todo!();
    }

    /// Modifies the card instance with the given ID and logs the new card.
    pub fn modify_card(
        &mut self,
        id: InstanceID,
        _log: impl FnMut(&dyn Event),
        f: impl Fn(&mut CardInstance<S::BaseCard>),
    ) {
        f(&mut self.cards[&id]);

        // todo!(): log self.cards[id]
    }

    fn id_location(&self, id: InstanceID) -> Option<PublicLocation> {
        if self.deck.contains(&id) {
            Some(PublicLocation::Deck)
        } else if self.card_selection.contains(&id) {
            Some(PublicLocation::CardSelection)
        } else {
            None
        }
        .or_else(|| {
            self.hand
                .iter()
                .position(|v| v == &Some(id))
                .map(PublicLocation::Hand)
        })
    }

    fn remove_id(&mut self, id: InstanceID) {
        self.deck.retain(|deck_id| *deck_id != id);
        self.hand.retain(|hand_id| *hand_id != Some(id));
        self.limbo.retain(|limbo_id| *limbo_id != id);
        self.card_selection
            .retain(|card_selection_id| *card_selection_id != id);
        self.dusted.retain(|dusted_id| *dusted_id != id);
        // Attached.
        for card in self.cards.values_mut() {
            if card.attachment == Some(id) {
                card.attachment = None;
            }
        }
    }
}

impl<S: Secret + Default> Default for CardGameSecret<S> {
    fn default() -> Self {
        Self {
            cards: Default::default(),
            opaque_ptrs: Default::default(),
            deck: Default::default(),
            hand: Default::default(),
            limbo: Default::default(),
            dusted: Default::default(),
            card_selection: Default::default(),
            next_id: Default::default(),
            next_ptr: Default::default(),
            state: Default::default(),
        }
    }
}

/// A card must be located in exactly one of these Zones at all times.
/// Each Zone has an implicit or explicit association with the public state or its owner's secret state.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum Zone {
    /// `Zone::Deck` cards are only visible to the deck's owner.
    /// There's a public count of how many cards are in each deck.
    Deck,

    /// `Zone::Hand` cards may optionally be public. This represents cards in a player's hand.
    Hand {
        /// `Zone::Hand { public: true }` cards are completely public, and are revealed visually to the other player.
        /// `Zone::Hand { public: false }` cards are partially secret:
        /// The total count and order of Zone::Hand cards are public.
        public: bool,
    },

    /// `Zone::Field` cards are always public. This represents cards "in-play."
    Field,

    /// `Zone::Graveyard` are always public. This represents the "discard pile".
    Graveyard,

    /// `Zone::Limbo` has two associated collections.
    ///
    /// This zone is mostly used to hold newly instantiated cards before they're sent to a specific zone.
    Limbo {
        /// `Zone::Limbo { public: true }` cards are completely public.
        /// `Zone::Limbo { public: false }` cards are completely secret, and don't have any associated public information.
        public: bool,
    },

    /// `Zone::CardSelection` cards are only visible to their owner.
    /// There's a public count of how many cards are in a CardSelection zone.
    /// This is used to hold the cards players choose from at the beginning of a game.
    CardSelection,

    /// `Zone::Casting` cards are public. Cards are moved here while their effects are resolving.
    Casting,

    /// `Zone::Dusted` has two associated collections.
    ///
    /// Most dusted cards will end up in the public zone.
    /// So far, the only case for `Dusted { public: false }` cards is when an attachment is replaced on a secret card in-hand.
    Dusted {
        /// `Zone::Dusted { public: true }` cards are completely public.
        /// `Zone::Dusted { public: false }` cards are completely secret, and don't have any associated public information.
        public: bool,
    },

    /// `Zone::Attachment` represents a card being attached to another card.
    /// The secrecy of the card will always follow the secrecy of its parent.
    /// The ownership of the card will always follow the ownership of its parent.
    Attachment {
        /// Even if you know a card's zone is `Zone::Attachment`, you don't automatically know the zone its parent is in, or which card the parent is.
        parent: OpaquePointer,
    },
}

/// A concrete card.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CardInstance<T: BaseCard> {
    id: InstanceID,

    attachment: Option<InstanceID>,

    card_state: T::CardState,
}

impl<T: BaseCard> CardInstance<T> {
    /// Gets the card's ID.
    pub fn id(this: &Self) -> InstanceID {
        this.id
    }

    /// Gets the card's attachment, if any.
    ///
    /// Returns [None] if it doesn't have an attachment.
    pub fn attachment(this: &Self) -> Option<InstanceID> {
        this.attachment
    }
}

impl<T: BaseCard> Deref for CardInstance<T> {
    type Target = T::CardState;

    fn deref(&self) -> &Self::Target {
        &self.card_state
    }
}

impl<T: BaseCard> DerefMut for CardInstance<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.card_state
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
enum MaybeSecretID {
    Secret(Player),
    Public(InstanceID),
}

impl MaybeSecretID {
    /// Gets the player whose secret has this secret pointer.
    ///
    /// [None] if the pointer is public.
    pub fn player(&self) -> Option<Player> {
        match self {
            Self::Secret(player) => Some(*player),
            Self::Public(..) => None,
        }
    }

    /// Gets the public instance ID.
    ///
    /// [None] if the pointer is secret.
    pub fn id(&self) -> Option<InstanceID> {
        match self {
            Self::Secret(..) => None,
            Self::Public(id) => Some(*id),
        }
    }

    /// Gets the public instance ID.
    ///
    /// Panics if the pointer is secret.
    pub fn expect(&self, message: &str) -> InstanceID {
        self.id().expect(message)
    }
}

impl Debug for MaybeSecretID {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self {
            Self::Secret(player) => write!(f, "player {}", player),
            Self::Public(id) => {
                if f.alternate() {
                    write!(f, "{:#?}", id)
                } else {
                    write!(f, "{:?}", id)
                }
            }
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
enum MaybeSecretCard<T: BaseCard> {
    Secret(Player),
    #[serde(bound = "T: BaseCard")]
    Public(CardInstance<T>),
}

impl<T: BaseCard> MaybeSecretCard<T> {
    /// Gets the public card instance.
    ///
    /// [None] if the card is secret.
    pub fn instance(self) -> Option<CardInstance<T>> {
        match self {
            Self::Secret(..) => None,
            Self::Public(instance) => Some(instance),
        }
    }

    /// Gets the public card instance.
    ///
    /// [None] if the card is secret.
    pub fn instance_ref(&self) -> Option<&CardInstance<T>> {
        match self {
            Self::Secret(..) => None,
            Self::Public(instance) => Some(instance),
        }
    }

    /// Gets the public card instance.
    ///
    /// [None] if the card is secret.
    pub fn instance_mut(&mut self) -> Option<&mut CardInstance<T>> {
        match self {
            Self::Secret(..) => None,
            Self::Public(instance) => Some(instance),
        }
    }

    /// Gets the public card instance.
    ///
    /// Panics if the card is secret.
    pub fn expect(self, message: &str) -> CardInstance<T> {
        self.instance().expect(message)
    }

    /// Gets the public card instance.
    ///
    /// Panics if the card is secret.
    pub fn expect_ref(&self, message: &str) -> &CardInstance<T> {
        self.instance_ref().expect(message)
    }

    /// Gets the public card instance.
    ///
    /// Panics if the card is secret.
    pub fn expect_mut(&mut self, message: &str) -> &mut CardInstance<T> {
        self.instance_mut().expect(message)
    }

    fn bucket(&self) -> Bucket {
        match self {
            Self::Secret(player) => Bucket::Secret(*player),
            Self::Public(..) => Bucket::Public,
        }
    }
}

impl<T: BaseCard> Debug for MaybeSecretCard<T> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self {
            Self::Secret(player) => write!(f, "player {}", player),
            Self::Public(instance) => instance.fmt(f),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Eq, PartialEq, Debug)]
enum Bucket {
    Public,
    Secret(Player),
}

impl Bucket {
    fn player(&self) -> Option<Player> {
        match self {
            Self::Public => None,
            Self::Secret(player) => Some(*player),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
enum PublicLocation {
    Deck,
    Hand(usize),
    Field(usize),
    Graveyard(usize),
    PublicLimbo(usize),
    CardSelection,
    Casting(usize),
    PublicDusted(usize),
    PublicAttachment { parent: InstanceID },
}
