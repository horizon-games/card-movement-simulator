use {
    arcadeum::store::Tester,
    card_movement_simulator::{Card, CardGame, CardInstance, GameState, InstanceID, Player, Zone},
    std::{convert::TryInto, future::Future, pin::Pin},
};

#[derive(serde::Serialize, serde::Deserialize, Clone, Default, Debug)]
struct State;

impl card_movement_simulator::State for State {
    type ID = ID;

    type Nonce = u32;

    type Action = Action;

    type Secret = Secret;

    type BaseCard = BaseCard;

    fn version() -> &'static [u8] {
        "Test".as_bytes()
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

                    let card_id = live_game.new_card(from_player, base_card_type);

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

                    eprintln!(
                        "Moving card {:?} into its \"to\" zone: player {}'s {:?}",
                        card, to_player, to_zone,
                    );

                    live_game.move_card(card, to_player, to_zone).await.unwrap();

                    assert_eq!(live_game.reveal_ok().await, Ok(()));
                }
                Action::Detach {
                    parent_ptr_bucket,
                    attachment_ptr_bucket,
                    to_player,
                    to_zone,
                } => {
                    // TODO parent_ptr_bucket is never used (wasn't actually used for anything in previous tests)
                    let parent_owner = 0;
                    let parent_id = live_game.new_card(parent_owner, BaseCard::WithAttachment);
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
                    live_game
                        .move_card(attachment, to_player, to_zone)
                        .await
                        .unwrap();
                    assert!(
                        live_game
                            .reveal_from_card(attachment, move |info| info.owner == to_player)
                            .await
                    );
                    assert!(
                        live_game
                            .reveal_from_card(attachment, move |info| info.zone.eq(to_zone).unwrap_or(false))
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
                    let parent_id = live_game.new_card(parent_owner, parent_base_card);
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

                    let card_id = live_game.new_card(card_owner, BaseCard::Attachment);
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
                        .move_card(card, 0, Zone::Attachment { parent: parent })
                        .await
                        .unwrap();
                    assert_eq!(live_game.reveal_ok().await, Ok(()));

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

                    let parent = live_game.new_card(0, BaseCard::WithAttachment);
                    live_game
                        .move_card(parent, 0, Zone::Hand { public: false })
                        .await
                        .unwrap();

                    let card = live_game.new_card(0, BaseCard::Attachment);
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
                    let card_id = live_game.new_card(0, BaseCard::Basic);
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
                    let card = live_game.new_card(0, BaseCard::WithAttachment);
                    live_game.move_card(card, 0, Zone::Field).await.unwrap();

                    // Public Attachment
                    // Public WithAttachment

                    assert_eq!(live_game.instances(), 2);

                    let secret = live_game
                        .new_secret_cards(0, |info| {
                            info.secret.new_card(BaseCard::Basic);
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
                        .new_secret_cards(1, |info| {
                            info.secret.new_card(BaseCard::Basic);
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
            }
        })
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
        CardState
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
struct CardState;

impl card_movement_simulator::CardState for CardState {
    fn eq(&self, _other: &Self) -> bool {
        true
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
        parent_ptr_bucket: Option<Player>,     // 3
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
    ReplacingAttachOnSecretCardDoesNotLeakInfo,
    OpaquePointerAssociationDoesntHoldThroughDraw,
    InstanceFromIDSetup,
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
        Default::default(),
        Default::default(),
    )
    .unwrap()
    .apply(Some(0), &Action::ReplacingAttachOnSecretCardDoesNotLeakInfo)
    .unwrap();

    assert_eq!(reveals.len(), 0, "No reveals need to be made when attaching a public card to a secret card, even if it already has an attachment.")
}

#[test]
fn opaque_pointer_association_does_not_hold_through_draw() {
    Tester::new(
        GameState::<State>::default(),
        Default::default(),
        Default::default(),
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
        Default::default(),
        Default::default(),
    )
    .unwrap();

    tester.apply(Some(0), &Action::InstanceFromIDSetup).unwrap();

    // This is an implementation detail.
    // Constructing a public card in public limbo gives the attachment the ID after the parent card.

    let id: InstanceID = serde_cbor::from_slice(&[1]).unwrap();

    assert!(id.instance(tester.state(), None).is_some());
}

#[test]
fn secret_instance_from_id() {
    let mut tester = Tester::new(
        GameState::<State>::default(),
        Default::default(),
        Default::default(),
    )
    .unwrap();

    tester.apply(Some(0), &Action::InstanceFromIDSetup).unwrap();

    // This is an implementation detail.
    // Constructing a public card in public limbo gives the parent card the ID after the attachment.

    let id: InstanceID = serde_cbor::from_slice(&[2]).unwrap();

    assert!(id
        .instance(tester.state(), Some(&tester.secret(0)))
        .is_some());
}

#[test]
fn opponent_instance_from_id() {
    let mut tester = Tester::new(
        GameState::<State>::default(),
        Default::default(),
        Default::default(),
    )
    .unwrap();

    tester.apply(Some(0), &Action::InstanceFromIDSetup).unwrap();

    // This is an implementation detail.
    // Constructing a public card in public limbo gives the parent card the ID after the attachment.

    let id: InstanceID = serde_cbor::from_slice(&[4]).unwrap();

    assert!(id
        .instance(tester.state(), Some(&tester.secret(0)))
        .is_none());
}

include!(concat!(env!("OUT_DIR"), "/generated_tests.rs"));
