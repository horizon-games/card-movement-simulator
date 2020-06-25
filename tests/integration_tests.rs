use {
    arcadeum::store::Tester,
    card_movement_simulator::{CardGame, CardInstance, LiveGame, Player, Zone},
    std::{convert::TryInto, future::Future, pin::Pin},
};

#[derive(serde::Serialize, serde::Deserialize, Clone, Default, Debug)]
struct State;

impl card_movement_simulator::State for State {
    type ID = ID;

    type Nonce = u32;

    type Action = Action;

    type Secret = Secret;

    fn version() -> &'static [u8] {
        "Test".as_bytes()
    }

    fn verify(&self, _player: Option<Player>, _action: &Self::Action) -> Result<(), String> {
        Ok(())
    }

    fn apply(
        mut live_game: LiveGame<Self>,
        _player: Option<Player>,
        action: Self::Action,
    ) -> Pin<Box<dyn Future<Output = LiveGame<Self>>>> {
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

                    let card_opaque_ptr = live_game.new_card(from_player, base_card_type);

                    assert_eq!(live_game.reveal_ok().await, Ok(()));

                    eprintln!(
                        "Moving opaque ref {:?} into its \"from\" zone: player {}'s {:?}",
                        card_opaque_ptr, from_player, from_zone,
                    );

                    live_game
                        .move_card(card_opaque_ptr, from_player, from_zone)
                        .await;

                    assert_eq!(live_game.reveal_ok().await, Ok(()));

                    live_game
                        .move_pointer(card_opaque_ptr, &card_ptr_bucket)
                        .await;

                    assert_eq!(live_game.reveal_ok().await, Ok(()));

                    eprintln!(
                        "Moving opaque ref {:?} into its \"to\" zone: player {}'s {:?}",
                        card_opaque_ptr, to_player, to_zone,
                    );

                    live_game
                        .move_card(card_opaque_ptr, to_player, to_zone)
                        .await;

                    assert_eq!(live_game.reveal_ok().await, Ok(()));
                }
                Action::Detach {
                    parent_ptr_bucket,
                    attachment_ptr_bucket,
                    to_player,
                    to_zone,
                } => {
                    let parent_owner = 0;
                    let parent_ptr = live_game.new_card(parent_owner, BaseCard::WithAttachment);
                    let attachment_ptr = live_game
                        .attachment(parent_ptr)
                        .await
                        .expect("BaseCard::WithAttachment must have attachment.");
                    live_game.move_pointer(parent_ptr, &parent_ptr_bucket).await;
                    live_game
                        .move_pointer(attachment_ptr, &attachment_ptr_bucket)
                        .await;
                    live_game
                        .move_card(attachment_ptr, to_player, to_zone)
                        .await;
                    assert!(
                        live_game
                            .reveal_if_zone(attachment_ptr, move |player, zone| player == to_player
                                && zone == to_zone)
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
                    let parent_ptr = live_game.new_card(parent_owner, parent_base_card);
                    live_game
                        .move_card(parent_ptr, parent_owner, parent_zone)
                        .await;
                    live_game.move_pointer(parent_ptr, &parent_ptr_bucket).await;

                    let original_attachment = live_game.attachment(parent_ptr).await;

                    let card_ptr = live_game.new_card(card_owner, BaseCard::Attachment);
                    live_game.move_card(card_ptr, card_owner, card_zone).await;
                    live_game.move_pointer(card_ptr, &card_ptr_bucket).await;

                    assert_eq!(live_game.reveal_ok().await, Ok(()));
                    live_game
                        .move_card(card_ptr, 0, Zone::Attachment { parent: parent_ptr })
                        .await;
                    assert_eq!(live_game.reveal_ok().await, Ok(()));

                    assert_eq!(
                        live_game
                            .reveal_from_card(parent_ptr, CardInstance::attachment)
                            .await,
                        Some(live_game.reveal_from_card(card_ptr, CardInstance::id).await)
                    );

                    if let Some(original_attachment) = original_attachment {
                        // original attachment should have been dusted
                        let parent_id = live_game
                            .reveal_from_card(parent_ptr, CardInstance::id)
                            .await;

                        let parent_card_is_public = live_game.is_card_public(parent_id);

                        assert!(
                            live_game
                                .reveal_if_zone(original_attachment, move |player, zone| player
                                    == parent_owner
                                    && zone
                                        == Zone::Dusted {
                                            public: parent_card_is_public
                                        })
                                .await,
                        );
                    }
                }

                Action::ReplacingAttachOnSecretCardDoesNotLeakInfo => {
                    // All assertions that these methods work correctly are made in the auto-generated Attach tests.

                    let parent_ptr = live_game.new_card(0, BaseCard::WithAttachment);
                    live_game
                        .move_card(parent_ptr, 0, Zone::Hand { public: false })
                        .await;

                    let card_ptr = live_game.new_card(0, BaseCard::Attachment);
                    live_game
                        .move_card(card_ptr, 0, Zone::Attachment { parent: parent_ptr })
                        .await;
                }

                Action::OpaquePointerAssociationDoesntHoldThroughDraw => {
                    let card_ptr = live_game.new_card(0, BaseCard::Basic);
                    assert_eq!(live_game.player(0).deck(), 0);
                    live_game.move_card(card_ptr, 0, Zone::Deck).await;

                    assert_eq!(live_game.player(0).deck(), 1);
                    // In a real scenario, the deck could have any number of cards.
                    // For our test, having 1 card is enough to prove that secrecy would hold for more,
                    // but an intuitive observer (us) can understand that there's only 1 card we could possibly draw.
                    // That's perfect for this test.

                    let drawn_card = live_game
                        .draw_card(0)
                        .await
                        .expect("Player should have drawn a card.");

                    // Now, we have to prove that there's no *public* association between these two references.
                    assert_ne!(
                        card_ptr, drawn_card,
                        "These must be different opaque references."
                    );
                    assert!(match (live_game.id_for_pointer(card_ptr), live_game.id_for_pointer(drawn_card)) {
                        (Some(id_1), Some(id_2)) if id_1 == id_2 => false,
                        (Some(id_1), Some(id_2)) if id_1 != id_2 => panic!("There was only one card in deck but we drew a different one..."),
                    _ => true
                    }, "The MaybeSecretIDs pointed to must not be publicly reconcilable to the same value");

                    // But, the cards must have been associated in secret.

                    let card_id = live_game.reveal_from_card(card_ptr, CardInstance::id).await;

                    let drawn_id = live_game
                        .reveal_from_card(drawn_card, CardInstance::id)
                        .await;

                    assert_eq!(card_id, drawn_id);
                }
                Action::InstanceFromIDSetup => {
                    let card = live_game.new_card(0, BaseCard::WithAttachment);
                    live_game.move_card(card, 0, Zone::Field).await;

                    // Public Attachment
                    // Public WithAttachment

                    assert_eq!(live_game.cards_len(), 2);

                    let secret = live_game
                        .new_secret_cards(0, |secret, _, _| {
                            secret.new_card(BaseCard::Basic);
                        })
                        .await[0];

                    live_game.move_card(secret, 0, Zone::Deck).await;

                    // Public Attachment
                    // Public WithAttachment
                    // No card
                    // Secret 0 Basic

                    assert_eq!(live_game.cards_len(), 4);

                    let secret = live_game
                        .new_secret_cards(1, |secret, _, _| {
                            secret.new_card(BaseCard::Basic);
                        })
                        .await[0];

                    live_game.move_card(secret, 1, Zone::Deck).await;

                    // Public Attachment
                    // Public WithAttachment
                    // No card
                    // Secret 0 Basic
                    // No card
                    // Secret 1 Basic

                    assert_eq!(live_game.cards_len(), 6);
                }
            }

            live_game
        })
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default, Debug)]
struct Secret;

impl card_movement_simulator::Secret for Secret {
    type BaseCard = BaseCard;
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
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
        CardGame::<State>::default(),
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
        CardGame::<State>::default(),
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
        CardGame::<State>::default(),
        Default::default(),
        Default::default(),
    )
    .unwrap();

    tester.apply(Some(0), &Action::InstanceFromIDSetup).unwrap();

    // This is an implementation detail.
    // Constructing a public card in public limbo gives the attachment the ID after the parent card.

    let id = card_movement_simulator::InstanceID::from_raw(1);

    assert!(id.instance(tester.state(), None).is_some());
}

#[test]
fn secret_instance_from_id() {
    let mut tester = Tester::new(
        CardGame::<State>::default(),
        Default::default(),
        Default::default(),
    )
    .unwrap();

    tester.apply(Some(0), &Action::InstanceFromIDSetup).unwrap();

    // This is an implementation detail.
    // Constructing a public card in public limbo gives the parent card the ID after the attachment.

    let id = card_movement_simulator::InstanceID::from_raw(2);

    assert!(id
        .instance(tester.state(), Some(&tester.secret(0)))
        .is_some());
}

#[test]
fn opponent_instance_from_id() {
    let mut tester = Tester::new(
        CardGame::<State>::default(),
        Default::default(),
        Default::default(),
    )
    .unwrap();

    tester.apply(Some(0), &Action::InstanceFromIDSetup).unwrap();

    // This is an implementation detail.
    // Constructing a public card in public limbo gives the parent card the ID after the attachment.

    let id = card_movement_simulator::InstanceID::from_raw(4);

    assert!(id
        .instance(tester.state(), Some(&tester.secret(0)))
        .is_none());
}

include!(concat!(env!("OUT_DIR"), "/generated_tests.rs"));
