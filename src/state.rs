use {
    crate::{
        Action, Address, BaseCard, CardGame, CardInfo, CardInstance, GameState, Nonce, Player,
        Secret, ID,
    },
    std::{cmp::Ordering, fmt::Debug, future::Future, pin::Pin},
};

pub trait State: serde::Serialize + serde::de::DeserializeOwned + Clone + Debug + 'static {
    /// Identifier type
    type ID: ID;

    /// Nonce type
    type Nonce: Nonce;

    /// Action type
    type Action: Action;

    /// Event type
    type Event: Clone + Debug + serde::Serialize + serde::de::DeserializeOwned;

    /// Secret type
    type Secret: Secret + Debug;

    /// Base card type
    type BaseCard: BaseCard;

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
    fn verify(
        state: &GameState<Self>,
        player: Option<Player>,
        action: &Self::Action,
    ) -> Result<(), String>;

    /// Applies an action by a given player to the state.
    fn apply<'a>(
        game: &'a mut CardGame<Self>,
        player: Option<Player>,
        action: Self::Action,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>>;

    /// Used to sort the field. This is called every time a card is added to the field, removed from the field, or a card on the field is modified.
    fn field_order(_a: CardInfo<Self>, _b: CardInfo<Self>) -> Ordering {
        Ordering::Equal
    }

    /// A callback that lets you modify a parent card right after it gets a new attachment.
    fn on_attach(_parent: &mut CardInstance<Self>, _new_attach: &CardInstance<Self>) {}

    /// A callback that lets you modify a parent card right before it loses an attachment.
    fn on_detach(_parent: &mut CardInstance<Self>, _old_attach: &CardInstance<Self>) {}
}
