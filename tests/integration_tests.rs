use arcadeum::store::Tester;
use card_movement_simulator::{
    Card, CardEvent, CardGame, CardInstance, CardLocation, ExactCardLocation, GameState,
    InstanceID, Player, PlayerSecret, Zone,
};
use pretty_assertions::{assert_eq, assert_ne};
use std::{cell::RefCell, convert::TryInto, future::Future, pin::Pin, rc::Rc};

#[derive(serde::Serialize, serde::Deserialize, Clone, Default, Debug)]
struct State;

impl card_movement_simulator::State for State {
    type ID = ID;

    type Nonce = u32;

    type Action = Action;

    type Event = ();

    type Secret = Secret;

    type BaseCard = BaseCard;

    fn version() -> &'static [u8] {
        b"Test"
    }

    fn verify(
        _game: &GameState<Self>,
        _player: Option<Player>,
        _action: &Self::Action,
    ) -> Result<(), String> {
        Ok(())
    }

    fn apply<'a>(
        live_game: &'a mut CardGame<Self>,
        _player: Option<Player>,
        action: Self::Action,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        Box::pin(async move {
            match action {
                Action::Move {
                    card_ptr_bucket,
                    base_card_type,
                    from_player,
                    to_player,
                    from_zone,
                    to_zone,
                } => {
                    eprintln!(
                        "Testing {:?} {:?} {:?} {:?} {:?}",
                        base_card_type, from_player, to_player, from_zone, to_zone
                    );

                    eprintln!(
                        "Instantiating a new card {:?} for player {:?}",
                        base_card_type, from_player
                    );

                    eprintln!("and then...");

                    let card_id = live_game.new_card(from_player, base_card_type).await;

                    assert_eq!(live_game.reveal_ok().await, Ok(()));

                    eprintln!(
                        "Moving card {:?} into its \"from\" zone: player {}'s {:?}",
                        card_id, from_player, from_zone,
                    );

                    live_game
                        .move_card(card_id, from_player, from_zone)
                        .await
                        .unwrap();

                    assert_eq!(live_game.reveal_ok().await, Ok(()));

                    let card: Card = if let Some(ptr_owner) = card_ptr_bucket {
                        live_game
                            .new_secret_pointers(ptr_owner, |mut secret| {
                                secret.new_pointer(card_id);
                            })
                            .await[0]
                    } else {
                        card_id.into()
                    };

                    assert_eq!(live_game.reveal_ok().await, Ok(()));

                    println!(
                        "Moving card {:?} into its \"to\" zone: player {}'s {:?}",
                        card, to_player, to_zone,
                    );
                    live_game.move_card(card, to_player, to_zone).await.unwrap();

                    assert_eq!(live_game.reveal_ok().await, Ok(()));
                }
                Action::Detach {
                    parent_zone,
                    attachment_ptr_bucket,
                    to_player,
                    to_zone,
                } => {
                    let parent_owner = 0;
                    let parent_id = live_game
                        .new_card(parent_owner, BaseCard::WithAttachment)
                        .await;

                    live_game
                        .move_card(parent_id, 0, parent_zone)
                        .await
                        .unwrap();

                    let attachment_id = live_game
                        .reveal_from_card(parent_id, |info| {
                            info.attachment
                                .expect("BaseCard::WithAttachment must have attachment.")
                                .id()
                        })
                        .await;

                    let attachment: Card = if let Some(ptr_owner) = attachment_ptr_bucket {
                        live_game
                            .new_secret_pointers(ptr_owner, |mut secret| {
                                secret.new_pointer(attachment_id);
                            })
                            .await[0]
                    } else {
                        attachment_id.into()
                    };

                    assert_eq!(
                        live_game
                            .reveal_from_card(parent_id, |info| { info.attachment_was_detached })
                            .await,
                        0
                    );

                    live_game
                        .move_card(attachment, to_player, to_zone)
                        .await
                        .unwrap();

                    assert_eq!(
                        live_game
                            .reveal_from_card(parent_id, |info| { info.attachment_was_detached })
                            .await,
                        1,
                        "Did not fire detach callback!"
                    );

                    assert!(
                        live_game
                            .reveal_from_card(attachment, move |info| info.owner == to_player)
                            .await
                    );
                    assert!(
                        live_game
                            .reveal_from_card(attachment, move |info| info
                                .zone
                                .eq(to_zone)
                                .unwrap_or(false))
                            .await
                    );
                    assert_eq!(live_game.reveal_ok().await, Ok(()));
                }
                Action::Attach {
                    parent_base_card,
                    parent_ptr_bucket,
                    parent_zone,
                    card_ptr_bucket,
                    card_owner,
                    card_zone,
                } => {
                    let parent_owner = 0;
                    let parent_id = live_game.new_card(parent_owner, parent_base_card).await;
                    live_game
                        .move_card(parent_id, parent_owner, parent_zone)
                        .await
                        .unwrap();

                    let parent: Card = if let Some(ptr_owner) = parent_ptr_bucket {
                        live_game
                            .new_secret_pointers(ptr_owner, |mut secret| {
                                secret.new_pointer(parent_id);
                            })
                            .await[0]
                    } else {
                        parent_id.into()
                    };

                    let original_attachment = live_game
                        .reveal_from_card(parent_id, |info| info.attachment.map(|c| c.id()))
                        .await;

                    let started_with_attach = original_attachment.is_some();
                    assert_eq!(
                        live_game
                            .reveal_from_card(parent_id, move |info| {
                                info.attachment_was_attached
                            })
                            .await,
                        if started_with_attach { 1 } else { 0 }
                    );

                    let card_id = live_game.new_card(card_owner, BaseCard::Attachment).await;
                    live_game
                        .move_card(card_id, card_owner, card_zone)
                        .await
                        .unwrap();
                    let card: Card = if let Some(ptr_owner) = card_ptr_bucket {
                        live_game
                            .new_secret_pointers(ptr_owner, |mut secret| {
                                secret.new_pointer(card_id);
                            })
                            .await[0]
                    } else {
                        card_id.into()
                    };

                    assert_eq!(live_game.reveal_ok().await, Ok(()));
                    live_game
                        .move_card(card, 0, Zone::Attachment { parent })
                        .await
                        .unwrap();
                    assert_eq!(live_game.reveal_ok().await, Ok(()));
                    assert_eq!(
                        live_game
                            .reveal_from_card(parent_id, |info| { info.attachment_was_attached })
                            .await,
                        if started_with_attach { 2 } else { 1 }
                    );

                    assert_eq!(
                        live_game
                            .reveal_from_card(parent, |info| CardInstance::attachment(
                                info.instance
                            ))
                            .await,
                        Some(
                            live_game
                                .reveal_from_card(card, |info| CardInstance::id(info.instance))
                                .await
                        )
                    );

                    if let Some(original_attachment) = original_attachment {
                        // original attachment should have been dusted
                        let parent_id = live_game
                            .reveal_from_card(parent, |info| CardInstance::id(info.instance))
                            .await;

                        let parent_card_is_public = parent_id.instance(&live_game, None).is_some();

                        assert!(
                            live_game
                                .reveal_from_card(original_attachment, move |info| {
                                    info.owner == parent_owner
                                        && if let Zone::Dust { public } = info.zone {
                                            public == parent_card_is_public
                                        } else {
                                            false
                                        }
                                })
                                .await,
                        );
                    }
                }

                Action::AttachFromAttached {
                    parent_base_card,
                    parent_ptr_bucket,
                    parent_zone,
                    card_ptr_bucket,
                    card_owner,
                    card_zone,
                } => {
                    let parent_owner = 0;
                    let parent_id = live_game.new_card(parent_owner, parent_base_card).await;

                    live_game
                        .move_card(parent_id, parent_owner, parent_zone)
                        .await
                        .unwrap();

                    let parent: Card = if let Some(ptr_owner) = parent_ptr_bucket {
                        live_game
                            .new_secret_pointers(ptr_owner, |mut secret| {
                                secret.new_pointer(parent_id);
                            })
                            .await[0]
                    } else {
                        parent_id.into()
                    };

                    let original_attachment = live_game
                        .reveal_from_card(parent_id, |info| info.attachment.map(|c| c.id()))
                        .await;

                    let started_with_attach = original_attachment.is_some();
                    assert_eq!(
                        live_game
                            .reveal_from_card(parent_id, move |info| {
                                info.attachment_was_attached
                            })
                            .await,
                        if started_with_attach { 1 } else { 0 }
                    );

                    let starting_parent_id = live_game
                        .new_card(card_owner, BaseCard::WithAttachment)
                        .await;
                    live_game
                        .move_card(starting_parent_id, card_owner, card_zone)
                        .await
                        .unwrap();

                    let card_id = live_game
                        .reveal_from_card(starting_parent_id, |c| c.attachment.unwrap().id())
                        .await;

                    let card: Card = if let Some(ptr_owner) = card_ptr_bucket {
                        live_game
                            .new_secret_pointers(ptr_owner, |mut secret| {
                                secret.new_pointer(card_id);
                            })
                            .await[0]
                    } else {
                        card_id.into()
                    };

                    assert_eq!(live_game.reveal_ok().await, Ok(()));
                    live_game
                        .move_card(card, 0, Zone::Attachment { parent })
                        .await
                        .unwrap();
                    assert_eq!(live_game.reveal_ok().await, Ok(()));
                    assert_eq!(
                        live_game
                            .reveal_from_card(parent_id, |info| { info.attachment_was_attached })
                            .await,
                        if started_with_attach { 2 } else { 1 }
                    );

                    assert_eq!(
                        live_game
                            .reveal_from_card(parent, |info| CardInstance::attachment(
                                info.instance
                            ))
                            .await,
                        Some(
                            live_game
                                .reveal_from_card(card, |info| CardInstance::id(info.instance))
                                .await
                        )
                    );

                    if let Some(original_attachment) = original_attachment {
                        // original attachment should have been dusted
                        let parent_id = live_game
                            .reveal_from_card(parent, |info| CardInstance::id(info.instance))
                            .await;

                        let parent_card_is_public = parent_id.instance(&live_game, None).is_some();

                        assert!(
                            live_game
                                .reveal_from_card(original_attachment, move |info| {
                                    info.owner == parent_owner
                                        && if let Zone::Dust { public } = info.zone {
                                            public == parent_card_is_public
                                        } else {
                                            false
                                        }
                                })
                                .await,
                        );
                    }
                }

                Action::ReplacingAttachOnSecretCardDoesNotLeakInfo => {
                    // All assertions that these methods work correctly are made in the auto-generated Attach tests.

                    let parent = live_game.new_card(0, BaseCard::WithAttachment).await;
                    live_game
                        .move_card(parent, 0, Zone::Hand { public: false })
                        .await
                        .unwrap();

                    let card = live_game.new_card(0, BaseCard::Attachment).await;
                    live_game
                        .move_card(
                            card,
                            0,
                            Zone::Attachment {
                                parent: parent.into(),
                            },
                        )
                        .await
                        .unwrap();
                }

                Action::OpaquePointerAssociationDoesntHoldThroughDraw => {
                    let card_id = live_game.new_card(0, BaseCard::Basic).await;
                    assert_eq!(live_game.player_cards(0).deck(), 0);
                    live_game.move_card(card_id, 0, Zone::Deck).await.unwrap();

                    assert_eq!(live_game.player_cards(0).deck(), 1);
                    // In a real scenario, the deck could have any number of cards.
                    // For our test, having 1 card is enough to prove that secrecy would hold for more,
                    // but an intuitive observer (us) can understand that there's only 1 card we could possibly draw.
                    // That's perfect for this test.

                    let drawn_card = live_game
                        .draw_card(0)
                        .await
                        .expect("Player should have drawn a card.");

                    // Now, we have to prove that there's no *public* association between these two references.
                    assert!(drawn_card.id().is_none(), "The drawn card must be secret.");

                    // But, the cards must be associated in secret.
                    let drawn_id = live_game
                        .reveal_from_card(drawn_card, |info| info.instance.id())
                        .await;

                    assert_eq!(card_id, drawn_id);
                }
                Action::InstanceFromIDSetup => {
                    let card = live_game.new_card(0, BaseCard::WithAttachment).await;
                    live_game.move_card(card, 0, Zone::Field).await.unwrap();

                    // Public Attachment
                    // Public WithAttachment

                    assert_eq!(live_game.instances(), 2);

                    let secret = live_game
                        .new_secret_cards(0, |mut info| {
                            info.new_card(BaseCard::Basic);
                        })
                        .await[0];
                    assert_eq!(live_game.instances(), 4); // attachment or not info shouldn't be leaked

                    live_game.move_card(secret, 0, Zone::Deck).await.unwrap();

                    // Public Attachment
                    // Public WithAttachment
                    // No card
                    // Secret 0 Basic

                    assert_eq!(live_game.instances(), 4);

                    let secret = live_game
                        .new_secret_cards(1, |mut info| {
                            info.new_card(BaseCard::Basic);
                        })
                        .await[0];

                    live_game.move_card(secret, 1, Zone::Deck).await.unwrap();

                    // Public Attachment
                    // Public WithAttachment
                    // No card
                    // Secret 0 Basic
                    // No card
                    // Secret 1 Basic

                    assert_eq!(live_game.instances(), 6);
                }
                Action::CopyCard {
                    card_ptr_bucket,
                    base_card_type,
                    card_zone,
                    deep,
                } => {
                    let owner = 0;
                    let card_id = live_game.new_card(owner, base_card_type).await;

                    live_game
                        .move_card(card_id, owner, card_zone)
                        .await
                        .unwrap();

                    let card: Card = if let Some(ptr_owner) = card_ptr_bucket {
                        live_game
                            .new_secret_pointers(ptr_owner, |mut secret| {
                                secret.new_pointer(card_id);
                            })
                            .await[0]
                    } else {
                        card_id.into()
                    };

                    assert_eq!(live_game.reveal_ok().await, Ok(()));

                    let copy = live_game.copy_card(card, deep).await;

                    assert_eq!(live_game.reveal_ok().await, Ok(()));

                    let (copy_id, was_attach_cloned) = live_game
                        .reveal_from_card(copy, |c| {
                            dbg!(&c.instance);
                            (c.id(), c.attachment.map(|a| a.was_cloned))
                        })
                        .await;

                    let should_have_attach = matches!(base_card_type, BaseCard::WithAttachment);

                    match (deep, should_have_attach, was_attach_cloned) {
                        (false, true, Some(false))
                        | (false, false, None)
                        | (true, true, Some(true))
                        | (true, false, None) => {
                            // ok!
                        }
                        _ => {
                            panic!("Failed to copy attach correctly. Card type is {:?},  deep: {:?}, should_have_attach: {:?}, was_attach_cloned: {:?}", base_card_type, deep, should_have_attach, was_attach_cloned);
                        }
                    }

                    assert_ne!(
                        card_id, copy_id,
                        "Copy ID must be different from parent ID!"
                    );
                }
                Action::ResetCard {
                    card_ptr_bucket,
                    base_card_type,
                    attachment_type,
                    card_zone,
                } => {
                    let owner = 0;
                    let card_id = live_game.new_card(owner, base_card_type).await;

                    let starting_attach = live_game
                        .reveal_from_card(card_id, |c| {
                            c.attachment
                                .map(|attachment| (attachment.id(), attachment.base().clone()))
                        })
                        .await;

                    if let Some(new_attach) = attachment_type {
                        let attach = live_game.new_card(owner, new_attach).await;
                        live_game
                            .move_card(
                                attach,
                                owner,
                                Zone::Attachment {
                                    parent: card_id.into(),
                                },
                            )
                            .await
                            .unwrap();
                    }

                    live_game
                        .move_card(card_id, owner, card_zone)
                        .await
                        .unwrap();

                    let card: Card = if let Some(ptr_owner) = card_ptr_bucket {
                        live_game
                            .new_secret_pointers(ptr_owner, |mut secret| {
                                secret.new_pointer(card_id);
                            })
                            .await[0]
                    } else {
                        card_id.into()
                    };

                    assert_eq!(live_game.reveal_ok().await, Ok(()));

                    live_game.reset_card(card).await;

                    assert_eq!(live_game.reveal_ok().await, Ok(()));

                    let ending_attach = live_game
                        .reveal_from_card(card_id, |c| {
                            c.attachment
                                .map(|attachment| (attachment.id(), attachment.base().clone()))
                        })
                        .await;

                    match (attachment_type, starting_attach, ending_attach) {
                        (None, Some((start_id, start_base)), Some((end_id, end_base))) => {
                            assert_eq!(start_base, end_base, "Attachment wasn't changed before reset, but attachment base changed.");
                            assert_eq!(start_id, end_id, "Attachment wasn't changed before reset, but attachment instance changed.");
                        }
                        (None, None, Some(..)) => panic!(
                            "Card didn't start with attach, didn't get one, reset, but has one now."
                        ),
                        (Some(..), None, Some(..)) => panic!(
                            "Card didn't start with attach, got one, reset, but still has one."
                        ),
                        (Some(..), Some(..), None) => panic!(
                            "Card started with an attach, got one, reset, but doesn't have one."
                        ),
                        (
                            Some(new_attach_base),
                            Some((start_id, start_base)),
                            Some((end_id, end_base)),
                        ) => {
                            assert_eq!(start_base, end_base, "Card started with an attach, got one, reset, but has non-original attach base.");
                            if new_attach_base == start_base {
                                assert_ne!(start_id, end_id, "Card started with an attach, got one with the same base, reset, but has the same attach instance it started with.")
                            }
                        }
                        (None, None, None) => {
                            // Ok! Didn't start with an attach, didn't get one, have none at the end.
                        }
                        (Some(_), None, None) => {
                            // Ok! Didn't start with an attach, got one, reset, have none at the end.                        
                        },
                        (None, Some(..), None) => panic!("Card started with an attach, didn't get one, reset, and lost its attach.")
                    }

                    let card_state = live_game
                        .reveal_from_card(card_id, |c| c.instance.clone())
                        .await;
                    let base_state =
                        card_movement_simulator::BaseCard::new_card_state(&base_card_type);

                    assert!(
                        card_movement_simulator::CardState::eq(&*card_state, &base_state,),
                        "Card failed to reset: {:?} != {:?}",
                        *card_state,
                        base_state
                    );

                    if let Some((attach_id, attach_base)) = ending_attach {
                        let attach_state = live_game
                            .reveal_from_card(attach_id, |c| c.instance.clone())
                            .await;
                        let attach_base_state =
                            card_movement_simulator::BaseCard::new_card_state(&attach_base);

                        assert!(
                            card_movement_simulator::CardState::eq(
                                &*attach_state,
                                &attach_base_state,
                            ),
                            "Card failed to reset: {:?} != {:?}",
                            *attach_state,
                            attach_base_state
                        );
                    }
                }
                Action::RevealSecretHandCard => {
                    let hand = live_game
                        .new_secret_cards(0, |mut secret| {
                            for _ in 0..5 {
                                secret.new_card(BaseCard::Basic);
                            }
                        })
                        .await;

                    for card in &hand {
                        live_game
                            .move_card(card, 0, Zone::Hand { public: false })
                            .await
                            .unwrap();
                    }

                    live_game
                        .move_card(hand[2], 0, Zone::Hand { public: true })
                        .await
                        .unwrap();

                    assert_eq!(live_game.reveal_ok().await, Ok(()));
                }
            }
        })
    }
    fn on_attach(parent: &mut CardInstance<Self>, new_attach: &CardInstance<Self>) {
        assert_eq!(parent.attachment(), Some(new_attach.id()));
        parent.attachment_was_attached += 1;
    }
    fn on_detach(parent: &mut CardInstance<Self>, _old_attach: &CardInstance<Self>) {
        assert!(parent.attachment().is_none());
        parent.attachment_was_detached += 1;
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default, Debug)]
struct Secret;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
enum BaseCard {
    Basic,
    WithAttachment,
    Attachment,
}

impl card_movement_simulator::BaseCard for BaseCard {
    type CardState = CardState;

    fn attachment(&self) -> Option<Self> {
        match self {
            Self::Basic => None,
            Self::WithAttachment => Some(Self::Attachment),
            Self::Attachment => None,
        }
    }

    fn new_card_state(&self) -> Self::CardState {
        CardState {
            attachment_was_detached: 0,
            attachment_was_attached: 0,
            was_cloned: false,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
struct CardState {
    attachment_was_detached: usize,
    attachment_was_attached: usize,
    was_cloned: bool,
}

impl card_movement_simulator::CardState for CardState {
    fn eq(&self, other: &Self) -> bool {
        self.was_cloned == other.was_cloned
            && self.attachment_was_attached == other.attachment_was_attached
            && self.attachment_was_detached == other.attachment_was_detached
    }
    fn copy_card(&self) -> CardState {
        let mut copy = self.clone();
        copy.was_cloned = true;
        copy
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
enum Action {
    Move {
        card_ptr_bucket: Option<Player>, // 3
        base_card_type: BaseCard,        // 2
        from_player: Player,             // 2
        to_player: Player,               // 2
        from_zone: Zone,                 // 11
        to_zone: Zone,                   // 11
    },
    Detach {
        parent_zone: Zone,                     // 11
        attachment_ptr_bucket: Option<Player>, // 3
        to_player: Player,                     // 2
        to_zone: Zone,                         // 11
    },
    Attach {
        parent_base_card: BaseCard,        // 2
        parent_ptr_bucket: Option<Player>, // 3
        parent_zone: Zone,                 // 11
        card_ptr_bucket: Option<Player>,   // 3
        card_owner: Player,                // 2
        card_zone: Zone,                   // 11
    },
    AttachFromAttached {
        parent_base_card: BaseCard,        // 2
        parent_ptr_bucket: Option<Player>, // 3
        parent_zone: Zone,                 // 11
        card_ptr_bucket: Option<Player>,   // 3
        card_owner: Player,                // 2
        card_zone: Zone,                   // 11
    },
    CopyCard {
        card_ptr_bucket: Option<Player>, // 3
        card_zone: Zone,                 // 11
        base_card_type: BaseCard,        // 2
        deep: bool,                      // 2
    },
    ResetCard {
        card_ptr_bucket: Option<Player>,
        base_card_type: BaseCard,
        attachment_type: Option<BaseCard>,
        card_zone: Zone,
    },
    ReplacingAttachOnSecretCardDoesNotLeakInfo,
    OpaquePointerAssociationDoesntHoldThroughDraw,
    InstanceFromIDSetup,
    RevealSecretHandCard,
}

#[derive(Clone, Copy, Eq, PartialEq, Default)]
pub struct ID([u8; 8]);

impl card_movement_simulator::ID for ID {
    fn deserialize(data: &mut &[u8]) -> Result<Self, String> {
        if data.len() < 8 {
            return Err("data.len() < 8".to_string());
        }

        let id = data[..8].try_into().map_err(|error| format!("{}", error))?;
        *data = &data[8..];
        Ok(Self(id))
    }

    fn serialize(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

#[test]
fn replacing_attach_on_secret_card_does_not_leak_existence_of_current_attachment() {
    let reveals = Tester::new(
        GameState::<State>::default(),
        [
            PlayerSecret::new(0, Default::default()),
            PlayerSecret::new(1, Default::default()),
        ],
        Default::default(),
        |_, _, _| {},
        |_, _, _| {},
    )
    .unwrap()
    .apply(Some(0), &Action::ReplacingAttachOnSecretCardDoesNotLeakInfo)
    .unwrap();

    println!("{:?}", reveals);

    assert_eq!(reveals.len(), 0, "No reveals need to be made when attaching a public card to a secret card, even if it already has an attachment.")
}

#[test]
fn opaque_pointer_association_does_not_hold_through_draw() {
    Tester::new(
        GameState::<State>::default(),
        [
            PlayerSecret::new(0, Default::default()),
            PlayerSecret::new(1, Default::default()),
        ],
        Default::default(),
        |_, _, _| {},
        |_, _, _| {},
    )
    .unwrap()
    .apply(
        Some(0),
        &Action::OpaquePointerAssociationDoesntHoldThroughDraw,
    )
    .unwrap();
}

#[test]
fn public_instance_from_id() {
    let mut tester = Tester::new(
        GameState::<State>::default(),
        [
            PlayerSecret::new(0, Default::default()),
            PlayerSecret::new(1, Default::default()),
        ],
        Default::default(),
        |_, _, _| {},
        |_, _, _| {},
    )
    .unwrap();

    tester.apply(Some(0), &Action::InstanceFromIDSetup).unwrap();

    // This is an implementation detail.
    // Constructing a public card in public limbo gives the attachment the ID after the parent card.

    let id: InstanceID = InstanceID::from_raw(1);

    assert!(id.instance(tester.state(), None).is_some());
}

#[test]
fn secret_instance_from_id() {
    let mut tester = Tester::new(
        GameState::<State>::default(),
        [
            PlayerSecret::new(0, Default::default()),
            PlayerSecret::new(1, Default::default()),
        ],
        Default::default(),
        |_, _, _| {},
        |_, _, _| {},
    )
    .unwrap();

    tester.apply(Some(0), &Action::InstanceFromIDSetup).unwrap();

    // This is an implementation detail.
    // Constructing a public card in public limbo gives the parent card the ID after the attachment.

    let id: InstanceID = InstanceID::from_raw(0);

    assert!(id
        .instance(tester.state(), Some(&tester.secret(0)))
        .is_some());
}

#[test]
fn opponent_instance_from_id() {
    let mut tester = Tester::new(
        GameState::<State>::default(),
        [
            PlayerSecret::new(0, Default::default()),
            PlayerSecret::new(1, Default::default()),
        ],
        Default::default(),
        |_, _, _| {},
        |_, _, _| {},
    )
    .unwrap();

    tester.apply(Some(0), &Action::InstanceFromIDSetup).unwrap();

    // This is an implementation detail.
    // Constructing a public card in public limbo gives the parent card the ID after the attachment.

    let id: InstanceID = InstanceID::from_raw(4);

    assert!(id
        .instance(tester.state(), Some(&tester.secret(0)))
        .is_none());
}

#[test]
fn move_secret_hand_to_public_hand() {
    let (mut tester, _owner_logs, player_logs) = make_tester();
    tester
        .apply(Some(0), &Action::RevealSecretHandCard)
        .unwrap();

    println!(
        "

All Logs:"
    );
    for card in player_logs.try_borrow_mut().unwrap()[0].clone() {
        println!("{}", card);
    }
    println!(
        "
"
    );

    let mut actual_player_logs = player_logs.try_borrow_mut().unwrap()[0].clone().into_iter();

    for i in 0..5 {
        let move_to_secret_hand_event = actual_player_logs
            .next()
            .expect("Expected Some(CardEvent::MoveCard), got None.");
        if let CardEvent::MoveCard {
            to:
                ExactCardLocation {
                    player: 0,
                    location: (Zone::Hand { public: false }, index),
                },
            ..
        } = move_to_secret_hand_event
        {
            assert_eq!(index, i);
        } else {
            unreachable!(
                "Expected MoveCard to secret hand, got {:?}",
                move_to_secret_hand_event
            );
        };
    }

    let reveal_hand_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::MoveCard), got None.");
    assert!(
        matches!(reveal_hand_event, CardEvent::MoveCard { to: ExactCardLocation { player: 0, location: (Zone::Hand { public: true }, 2) }, .. }),
        "Expected MoveCard to public hand position 2, got {:#?}",
        reveal_hand_event
    );
}

fn make_tester() -> (
    Tester<GameState<State>>,
    Rc<RefCell<Vec<CardEvent<State>>>>,
    Rc<RefCell<[Vec<CardEvent<State>>; 2]>>,
) {
    let owner_logs: Rc<RefCell<Vec<CardEvent<State>>>> = Rc::new(RefCell::new(vec![]));
    let player_logs: Rc<RefCell<[Vec<CardEvent<State>>; 2]>> =
        Rc::new(RefCell::new([vec![], vec![]]));

    let owner_logs_clone = owner_logs.clone();
    let player_logs_clone = player_logs.clone();
    let tester = Tester::new(
        GameState::<State>::default(),
        [
            PlayerSecret::new(0, Default::default()),
            PlayerSecret::new(1, Default::default()),
        ],
        Default::default(),
        |_, _, _| {},
        move |player, _, message| match player {
            None => owner_logs_clone.try_borrow_mut().unwrap().push(message),
            Some(p) => player_logs_clone.try_borrow_mut().unwrap()[p as usize].push(message),
        },
    )
    .unwrap();

    (tester, owner_logs, player_logs)
}

fn test_move(
    card_ptr_bucket: Option<Player>,
    base_card_type: BaseCard,
    from_player: Player,
    from_zone: Zone,
    to_player: Player,
    to_zone: Zone,
) {
    let (mut tester, _owner_logs, player_logs) = make_tester();
    tester
        .apply(
            Some(0),
            &Action::Move {
                card_ptr_bucket,
                base_card_type,
                from_player,
                to_player,
                from_zone,
                to_zone,
            },
        )
        .unwrap();

    println!("\n\nAll Logs:");
    for card in player_logs.try_borrow_mut().unwrap()[0].clone() {
        {
            println!("{}", card);
        }
    }
    println!("\n");

    let mut actual_player_logs = player_logs.try_borrow_mut().unwrap()[0].clone().into_iter();

    // If our BaseCard has an attachment, we'll see a MoveCard to attach it upon creation.
    if base_card_type == BaseCard::WithAttachment {
        let attach_event = actual_player_logs
            .next()
            .expect("Expected attach event, got None.");
        assert!(
            matches!(attach_event, CardEvent::MoveCard {
                to: ExactCardLocation {
                    location: (Zone::Attachment{parent: Card::ID(_)}, _),
                    ..
                },
                instance: Some(_),
                ..
            }),
            "Base card has attachment, so expected attach event.\nGot {:#?}.",
            attach_event
        );

        // ModifyCard of parent from attach callback.
        let modify_event = actual_player_logs
            .next()
            .expect("Expected Some(CardEvent::ModifyCard), got None.");
        assert!(matches!(modify_event, CardEvent::ModifyCard {
            ..
        }));
    };

    let move_to_start_zone_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::MoveCard), got None.");
    assert!(matches!(move_to_start_zone_event, CardEvent::MoveCard{..}));

    // Event should fire if we moved to a different zone.
    // Player 0 should see the card instance in every event involving it.
    let move_to_end_zone_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::MoveCard), got None.");
    if from_player == 0
        || to_player == 0
        || to_zone.is_public().unwrap()
        || from_zone.is_public().unwrap()
    {
        assert!(matches!(move_to_end_zone_event, CardEvent::MoveCard{
            instance: Some(_),
            ..
        }));
    } else {
        assert!(matches!(move_to_end_zone_event, CardEvent::MoveCard{
            instance: None,
            ..
        }));
    }
}

fn test_detach(
    parent_zone: Zone,
    attachment_ptr_bucket: Option<Player>,
    to_player: Player,
    to_zone: Zone,
) {
    let (mut tester, _owner_logs, player_logs) = make_tester();

    tester
        .apply(
            Some(0),
            &Action::Detach {
                parent_zone,
                attachment_ptr_bucket,
                to_player,
                to_zone,
            },
        )
        .unwrap();

    println!("\n\nAll Logs:");
    for card in player_logs.try_borrow_mut().unwrap()[0].clone() {
        println!("{:#?}\n", card);
    }
    println!("\n");

    let mut actual_player_logs = player_logs.try_borrow_mut().unwrap()[0].clone().into_iter();

    // Our BaseCard has an attachment, so we'll see a MoveCard to attach it upon creation.
    let attach_event = actual_player_logs
        .next()
        .expect("Expected attach event, got None.");
    assert!(
        matches!(attach_event, CardEvent::MoveCard {
            to: ExactCardLocation {
                location: (Zone::Attachment{parent: Card::ID(_)}, _),
                ..
            },
            instance: Some(_),
            ..
        }),
        "Base card has attachment, so expected attach event.\nGot {:#?}.",
        attach_event
    );

    // ModifyCard of parent from attach callback.
    let modify_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::ModifyCard), got None.");
    assert!(matches!(modify_event, CardEvent::ModifyCard {
        ..
    }));

    let move_to_start_zone_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::MoveCard), got None.");
    assert!(matches!(move_to_start_zone_event, CardEvent::MoveCard{..}));

    let move_to_end_zone_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::MoveCard), got None.");
    assert!(
        matches!(move_to_end_zone_event, CardEvent::MoveCard {
            instance: Some(_),
            ..
        }),
        "Expected MoveCard to End Zone, got {:#?}",
        move_to_end_zone_event
    );

    // Player 0 should see the card instance in every event involving it.
    // ModifyCard for parent being modified because of detach.
    let modify_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::ModifyCard), got None.");
    if let CardEvent::ModifyCard { instance } = modify_event {
        assert!(instance.attachment().is_none());
    } else {
        unreachable!("Expected ModifyCard, got {:#?}", modify_event);
    }
}

fn test_attach(
    parent_base_card: BaseCard,
    parent_ptr_bucket: Option<Player>,
    parent_zone: Zone,
    card_ptr_bucket: Option<Player>,
    card_owner: Player,
    card_zone: Zone,
) {
    let (mut tester, _owner_logs, player_logs) = make_tester();

    tester
        .apply(
            Some(0),
            &Action::Attach {
                parent_base_card,
                parent_ptr_bucket,
                parent_zone,
                card_ptr_bucket,
                card_owner,
                card_zone,
            },
        )
        .unwrap();

    println!("\n\nAll Logs:");
    for card in player_logs.try_borrow_mut().unwrap()[0].clone() {
        println!("{}\n", card);
    }
    println!("\n");

    let mut actual_player_logs = player_logs.try_borrow_mut().unwrap()[0].clone().into_iter();

    if parent_base_card == BaseCard::WithAttachment {
        // Our BaseCard has an attachment, so we'll see a MoveCard to attach it upon creation.
        let attach_event = actual_player_logs
            .next()
            .expect("Expected attach event, got None.");
        assert!(
            matches!(attach_event, CardEvent::MoveCard {
                to: ExactCardLocation {
                    location: (Zone::Attachment{parent: Card::ID(_)}, _),
                    ..
                },
                instance: Some(_),
                ..
            }),
            "Base card has attachment, so expected attach event.\nGot {:#?}.",
            attach_event
        );

        // ModifyCard of parent from attach callback.
        let modify_event = actual_player_logs
            .next()
            .expect("Expected Some(CardEvent::ModifyCard), got None.");
        assert!(matches!(modify_event, CardEvent::ModifyCard {
            ..
        }));
    }
    let move_parent_to_start_zone_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::MoveCard), got None.");
    assert!(matches!(move_parent_to_start_zone_event, CardEvent::MoveCard{..}));

    let move_attach_to_start_zone_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::MoveCard), got None.");
    assert!(matches!(move_attach_to_start_zone_event, CardEvent::MoveCard{..}));

    if parent_base_card == BaseCard::WithAttachment {
        // Dust current attach
        let dust_current_attach = actual_player_logs
            .next()
            .expect("Expected Some(CardEvent::MoveCard), got None.");
        assert!(
            matches!(dust_current_attach, CardEvent::MoveCard {
                instance: Some(_),
                ..
            }),
            "Expected MoveCard from Zone::Attach to Zone::Dust, got {:#?}.",
            dust_current_attach
        );

        // ModifyCard for parent being modified because of detach.
        let modify_event = actual_player_logs
            .next()
            .expect("Expected Some(CardEvent::ModifyCard), got None.");
        assert!(
            matches!(modify_event, CardEvent::ModifyCard {
                ..
            }),
            "Expected CardEvent::ModifyCard for parent because of child being detached, got {:#?}",
            modify_event
        );
    }
    let attach_attachment_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::MoveCard), got None.");

    let zone = card_zone;
    let is_mine = card_owner == 0;
    let has_public_location = match zone {
        Zone::Deck => true,
        Zone::Hand { .. } => true,
        Zone::Field => true,
        Zone::Graveyard => true,
        Zone::Dust { .. } => true,
        Zone::Attachment { .. } => false,
        Zone::Limbo { public } => is_mine || public,
        Zone::Casting => true,
        Zone::CardSelection => true,
    };
    if has_public_location {
        assert!(
            matches!(attach_attachment_event, CardEvent::MoveCard {
                instance: Some(_),
                from: CardLocation {
                    location: Some(_),
                    ..
                },
                ..
            }),
            "Expected MoveCard to Zone::Attachment with Some(location), got {:#?}.",
            attach_attachment_event
        );
    } else {
        assert!(
            matches!(attach_attachment_event, CardEvent::MoveCard {
                instance: Some(_),
                from: CardLocation {
                    location: None,
                    ..
                },
                ..
            }),
            "Expected MoveCard to Zone::Attachment with location: None, got {:#?}.",
            attach_attachment_event
        );
    }

    // Parent being modified because of new attach.
    let modify_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::ModifyCard), got None.");
    assert!(
        matches!(modify_event, CardEvent::ModifyCard {
            ..
        }),
        "Expected CardEvent::ModifyCard for parent because of new attach, got {:#?}",
        modify_event
    );
}

fn test_attach_from_attached(
    parent_base_card: BaseCard,
    parent_ptr_bucket: Option<Player>,
    parent_zone: Zone,
    card_ptr_bucket: Option<Player>,
    card_owner: Player,
    card_zone: Zone,
) {
    let (mut tester, _owner_logs, player_logs) = make_tester();

    tester
        .apply(
            Some(0),
            &Action::AttachFromAttached {
                parent_base_card,
                parent_ptr_bucket,
                parent_zone,
                card_ptr_bucket,
                card_owner,
                card_zone,
            },
        )
        .unwrap();

    println!("\n\nAll Logs:");
    for card in player_logs.try_borrow_mut().unwrap()[0].clone() {
        println!("{:#?}\n", card);
    }
    println!("\n");

    let mut actual_player_logs = player_logs.try_borrow_mut().unwrap()[0].clone().into_iter();

    if parent_base_card == BaseCard::WithAttachment {
        // Our BaseCard has an attachment, so we'll see a MoveCard to attach it upon creation.
        let attach_event = actual_player_logs
            .next()
            .expect("Expected attach event, got None.");
        assert!(
            matches!(attach_event, CardEvent::MoveCard {
                to: ExactCardLocation {
                    location: (Zone::Attachment{parent: Card::ID(_)}, _),
                    ..
                },
                instance: Some(_),
                ..
            }),
            "Base card has attachment, so expected attach event.\nGot {:#?}.",
            attach_event
        );

        // ModifyCard of parent from attach callback.
        let modify_event = actual_player_logs
            .next()
            .expect("Expected Some(CardEvent::ModifyCard), got None.");
        assert!(matches!(modify_event, CardEvent::ModifyCard {
            ..
        }));
    }
    let move_parent_to_start_zone_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::MoveCard), got None.");
    assert!(matches!(move_parent_to_start_zone_event, CardEvent::MoveCard{..}));

    let start_parent_gains_attach = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::MoveCard), got None.");
    assert!(
        matches!(start_parent_gains_attach, CardEvent::MoveCard{
            to: ExactCardLocation {
                location: (Zone::Attachment {..}, _),
                ..
            },
            ..
        }),
        "Expected Some(CardEvent::MoveCard) for attach coming to starting parent, got {:#?}",
        start_parent_gains_attach
    );

    let start_parent_gains_attach_modify = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::ModifyCard), got None.");
    assert!(
        matches!(start_parent_gains_attach_modify, CardEvent::ModifyCard{..}),
        "Expected Some(CardEvent::ModifyCard) for attach callback on starting parent, got {:#?}",
        start_parent_gains_attach_modify
    );

    let move_attach_to_start_zone_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::MoveCard), got None.");
    assert!(matches!(move_attach_to_start_zone_event, CardEvent::MoveCard{..}));

    if parent_base_card == BaseCard::WithAttachment {
        // Dust current attach
        let dust_current_attach = actual_player_logs
            .next()
            .expect("Expected Some(CardEvent::MoveCard), got None.");
        assert!(
            matches!(dust_current_attach, CardEvent::MoveCard {
                instance: Some(_),
                ..
            }),
            "Expected MoveCard from Zone::Attach to Zone::Dust, got {:#?}.",
            dust_current_attach
        );

        // ModifyCard for parent being modified because of detach.
        let modify_event = actual_player_logs
            .next()
            .expect("Expected Some(CardEvent::ModifyCard), got None.");
        assert!(
            matches!(modify_event, CardEvent::ModifyCard {
                ..
            }),
            "Expected CardEvent::ModifyCard for parent because of child being detached, got {:#?}",
            modify_event
        );
    }
    let attach_attachment_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::MoveCard), got None.");

    let zone = { card_zone };
    let is_mine = { card_owner } == 0;
    let has_public_location = match zone {
        Zone::Deck => is_mine,
        Zone::Hand { public } => is_mine || public, // attachment secrecy depends on parent secrecy
        Zone::Field => true,
        Zone::Graveyard => true,
        Zone::Dust { public } => is_mine || public,
        Zone::Attachment { .. } => unreachable!(),
        Zone::Limbo { public } => is_mine || public,
        Zone::Casting => true,
        Zone::CardSelection => is_mine,
    };
    if has_public_location {
        assert!(
            matches!(attach_attachment_event, CardEvent::MoveCard {
                instance: Some(_),
                from: CardLocation {
                    location: Some(_),
                    ..
                },
                ..
            }),
            "Expected MoveCard to Zone::Attachment with Some(location), got {:#?}.",
            attach_attachment_event
        );
    } else {
        assert!(
            matches!(attach_attachment_event, CardEvent::MoveCard {
                instance: Some(_),
                from: CardLocation {
                    location: None,
                    ..
                },
                ..
            }),
            "Expected MoveCard to Zone::Attachment with location: None, got {:#?}.",
            attach_attachment_event
        );
    }

    // Parent being modified because of new attach.
    let modify_event = actual_player_logs
        .next()
        .expect("Expected Some(CardEvent::ModifyCard), got None.");
    assert!(
        matches!(modify_event, CardEvent::ModifyCard {
            ..
        }),
        "Expected CardEvent::ModifyCard for parent because of new attach, got {:#?}",
        modify_event
    );
}

fn test_copy_card(
    card_ptr_bucket: Option<Player>,
    base_card_type: BaseCard,
    card_zone: Zone,
    deep: bool,
) {
    let (mut tester, _owner_logs, player_logs) = make_tester();

    tester
        .apply(
            Some(0),
            &Action::CopyCard {
                card_ptr_bucket,
                base_card_type,
                card_zone,
                deep,
            },
        )
        .unwrap();

    println!("\n\nAll Logs:");
    for card in player_logs.try_borrow_mut().unwrap()[0].clone() {
        println!("{:#?}\n", card);
    }
    println!("\n");
}

fn test_reset_card(
    card_ptr_bucket: Option<Player>,   // 3
    attachment_type: Option<BaseCard>, //2
    base_card_type: BaseCard,          // 2
    card_zone: Zone,
) {
    let (mut tester, _owner_logs, player_logs) = make_tester();
    tester
        .apply(
            Some(0),
            &Action::ResetCard {
                card_ptr_bucket,
                attachment_type,
                base_card_type,
                card_zone,
            },
        )
        .unwrap();

    println!("\n\nAll Logs:");
    for card in player_logs.try_borrow_mut().unwrap()[0].clone() {
        println!("{:#?}\n", card);
    }
    println!("\n");
}
include!(concat!(env!("OUT_DIR"), "/generated_tests.rs"));
