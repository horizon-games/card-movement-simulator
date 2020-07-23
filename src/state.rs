use {
    crate::{Action, Address, BaseCard, CardGame, GameState, Nonce, Player, Secret, ID},
    std::{future::Future, pin::Pin},
};

pub trait State: serde::Serialize + serde::de::DeserializeOwned + Clone + 'static {
    /// Identifier type
    type ID: ID;

    /// Nonce type
    type Nonce: Nonce;

    /// Action type
    type Action: Action;

    /// Secret type
    type Secret: Secret;

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
}
