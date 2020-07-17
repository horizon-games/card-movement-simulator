use {
    crate::{
        Address, Card, CardGame, CardInstance, Context, InstanceID, Player, PlayerSecret,
        PlayerCards, State, Zone,
    },
    std::{
        future::Future,
        ops::{Deref, DerefMut},
        pin::Pin,
    },
};

#[derive(Clone)]
pub struct GameState<S: State> {
    instances: Vec<InstanceOrPlayer<S>>,

    player_cards: [PlayerCards; 2],

    state: S,
}

impl<S: State> Deref for GameState<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<S: State> DerefMut for GameState<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl<S: State> GameState<S> {
    pub fn new(state: S) -> Self {
        Self {
            instances: Default::default(),
            player_cards: Default::default(),
            state,
        }
    }

    pub fn all_player_cards(&self) -> &[PlayerCards] {
        &self.player_cards
    }

    pub fn all_player_cards_mut(&mut self) -> &mut [PlayerCards] {
        &mut self.player_cards
    }

    pub fn player_cards(&self, player: Player) -> &PlayerCards {
        &self.player_cards[usize::from(player)]
    }

    pub fn player_cards_mut(&mut self, player: Player) -> &mut PlayerCards {
        &mut self.player_cards[usize::from(player)]
    }

    pub fn owner(&self, id: InstanceID) -> Player {
        todo!();
    }

    pub fn zone(&self, id: InstanceID) -> (Player, Option<Zone>) {
        todo!();
    }

    pub fn location(&self, id: InstanceID) -> (Player, Option<(Zone, usize)>) {
        todo!();
    }
}

impl<S: State> arcadeum::store::State for GameState<S> {
    type ID = S::ID;
    type Nonce = S::Nonce;
    type Action = S::Action;
    type Secret = PlayerSecret<S>;

    fn version() -> &'static [u8] {
        S::version()
    }

    fn challenge(address: &Address) -> String {
        S::challenge(address)
    }

    fn deserialize(data: &[u8]) -> Result<Self, String> {
        todo!();
    }

    fn is_serializable(&self) -> bool {
        self.serialize().is_some()
    }

    fn serialize(&self) -> Option<Vec<u8>> {
        todo!();
    }

    fn verify(&self, player: Option<Player>, action: &Self::Action) -> Result<(), String> {
        S::verify(self, player, action)
    }

    fn apply(
        self,
        player: Option<crate::Player>,
        action: &Self::Action,
        context: Context<S>,
    ) -> Pin<Box<dyn Future<Output = (Self, Context<S>)>>> {
        let action = action.clone();

        Box::pin(async move {
            let mut game = CardGame {
                state: self,
                context,
            };

            S::apply(&mut game, player, action).await;

            let CardGame { state, context } = game;

            (state, context)
        })
    }
}

#[derive(Clone)]
enum InstanceOrPlayer<S: State> {
    Instance(CardInstance<S>),
    Player(Player),
}
