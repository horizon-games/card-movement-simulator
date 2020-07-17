use {
    crate::{
        Card, CardInstance, Context, Event, GameState, InstanceID, Player, PlayerSecret, State,
        Zone,
    },
    std::ops::{Deref, DerefMut},
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
        todo!();
    }

    pub fn deck_card(&mut self, player: Player, index: impl Into<usize>) -> Card {
        todo!();
    }

    pub fn hand_card(&mut self, player: Player, index: impl Into<usize>) -> Card {
        todo!();
    }

    pub fn field_card(&self, player: Player, index: impl Into<usize>) -> InstanceID {
        todo!();
    }

    pub fn graveyard_card(&self, player: Player, index: impl Into<usize>) -> InstanceID {
        todo!();
    }

    pub fn public_dust_card(&self, player: Player, index: impl Into<usize>) -> InstanceID {
        todo!();
    }

    pub fn secret_dust_card(&mut self, player: Player, index: impl Into<usize>) -> Card {
        todo!();
    }

    pub fn public_limbo_card(&self, player: Player, index: impl Into<usize>) -> InstanceID {
        todo!();
    }

    pub fn secret_limbo_card(&mut self, player: Player, index: impl Into<usize>) -> Card {
        todo!();
    }

    pub fn casting_card(&self, player: Player, index: impl Into<usize>) -> InstanceID {
        todo!();
    }

    pub fn card_selection_card(&mut self, player: Player, index: impl Into<usize>) -> Card {
        todo!();
    }

    pub fn deck_cards(&mut self, player: Player) -> Vec<Card> {
        todo!();
    }

    pub fn hand_cards(&mut self, player: Player) -> Vec<Card> {
        todo!();
    }

    pub fn field_cards(&self, player: Player) -> &Vec<InstanceID> {
        todo!();
    }

    pub fn graveyard_cards(&self, player: Player) -> &Vec<InstanceID> {
        todo!();
    }

    pub fn public_dust_cards(&self, player: Player) -> &Vec<InstanceID> {
        todo!();
    }

    pub fn secret_dust_cards(&mut self, player: Player) -> Vec<Card> {
        todo!();
    }

    pub fn public_limbo_cards(&self, player: Player) -> &Vec<InstanceID> {
        todo!();
    }

    pub fn secret_limbo_cards(&mut self, player: Player) -> Vec<Card> {
        todo!();
    }

    pub fn casting_cards(&self, player: Player) -> &Vec<InstanceID> {
        todo!();
    }

    pub fn card_selection_cards(&mut self, player: Player) -> Vec<Card> {
        todo!();
    }

    pub async fn reveal_if_cards_eq(&mut self, a: impl Into<Card>, b: impl Into<Card>) -> bool {
        todo!();
    }

    pub async fn reveal_if_cards_ne(&mut self, a: impl Into<Card>, b: impl Into<Card>) -> bool {
        todo!();
    }

    pub async fn reveal_from_card<T>(
        &mut self,
        card: impl Into<Card>,
        f: impl Fn(CardInfo<S>) -> T,
    ) -> T {
        todo!();
    }

    pub async fn reveal_from_cards<T>(
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
    ) -> Vec<Card> {
        todo!();
    }

    pub async fn modify_card(&mut self, card: impl Into<Card>, f: impl Fn(CardInfoMut<S>)) {
        todo!();
    }

    pub async fn modify_cards(&mut self, cards: Vec<Card>, f: impl Fn(CardInfoMut<S>)) {
        todo!();
    }

    pub async fn move_card(&mut self, card: impl Into<Card>, to_player: Player, to_zone: Zone) {
        todo!();
    }

    pub async fn move_cards(&mut self, cards: Vec<Card>, to_player: Player, to_zone: Zone) {
        todo!();
    }

    pub async fn draw_card(&mut self, player: Player) -> Card {
        todo!();
    }

    pub async fn draw_cards(&mut self, player: Player, count: impl Into<usize>) -> Vec<Card> {
        todo!();
    }

    pub async fn new_secret_cards(
        &mut self,
        player: Player,
        f: impl Fn(SecretInfo<S>),
    ) -> Vec<Card> {
        todo!();
    }

    pub async fn new_secret_pointers(
        &mut self,
        player: Player,
        f: impl Fn(SecretInfo<S>),
    ) -> Vec<Card> {
        todo!();
    }
}

pub struct CardInfo<'a, S: State> {
    pub card: &'a CardInstance<S>,
    pub owner: Player,
    pub zone: Zone,
    pub attachment: Option<&'a CardInstance<S>>,
}

pub struct CardInfoMut<'a, S: State> {
    pub card: &'a mut CardInstance<S>,
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
        self.card
    }
}

impl<S: State> Deref for CardInfoMut<'_, S> {
    type Target = CardInstance<S>;

    fn deref(&self) -> &Self::Target {
        self.card
    }
}

impl<S: State> DerefMut for CardInfoMut<'_, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.card
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
