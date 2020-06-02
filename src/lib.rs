use {
    arcadeum::{
        store::{Context, Event, Secret, State, Tester},
        Player, ID,
    },
    std::{convert::TryInto, future::Future, pin::Pin},
};

pub struct LiveGame {
    game: CardGame,

    context: Context<CardGame>,
}

impl LiveGame {
    pub fn game(&self) -> &CardGame {
        &self.game
    }

    /// Checks if the state is valid.
    pub async fn ok(&mut self) -> Result<(), String> {
        todo!();
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

    /// Creates a card in a zone.
    ///
    /// If you want to create a secret card from a secret card view, use [CardGameSecret::new_card] from within a [LiveGame::mutate_secret] instead.
    pub async fn new_card(&mut self, view: CardView, zone: &PlayerZone) -> OpaquePointer {
        match &zone.zone {
            Zone::Deck => {
                let id = self.new_secret_card(zone.player, view);

                self.context.mutate_secret(zone.player, |secret, _, _| {
                    secret.deck.push(id);
                });

                self.game.player_mut(zone.player).deck += 1;

                self.new_public_pointer(id)
            }
            Zone::Hand { public: false } => {
                let id = self.new_secret_card(zone.player, view);

                // The instance ID is public even though the card view isn't.

                self.context.mutate_secret(zone.player, |secret, _, _| {
                    secret.hand.push(None);
                });

                self.game.player_mut(zone.player).hand.push(Some(id));

                self.new_public_pointer(id)
            }
            Zone::Hand { public: true } => {
                let id = self.new_public_card(view);

                self.context.mutate_secret(zone.player, |secret, _, _| {
                    secret.hand.push(None);
                });

                self.game.player_mut(zone.player).hand.push(Some(id));

                self.new_public_pointer(id)
            }
            Zone::Field => {
                let id = self.new_public_card(view);

                self.game.player_mut(zone.player).field.push(id);

                self.new_public_pointer(id)
            }
            Zone::Graveyard => {
                let id = self.new_public_card(view);

                self.game.player_mut(zone.player).graveyard.push(id);

                self.new_public_pointer(id)
            }
            Zone::Limbo { public: false } => {
                let id = self.new_secret_card(zone.player, view);

                // The instance ID is public even though the card view isn't.

                self.context.mutate_secret(zone.player, |secret, _, _| {
                    secret.limbo.push(None);
                });

                self.game.player_mut(zone.player).limbo.push(Some(id));

                self.new_public_pointer(id)
            }
            Zone::Limbo { public: true } => {
                let id = self.new_public_card(view);

                self.context.mutate_secret(zone.player, |secret, _, _| {
                    secret.limbo.push(None);
                });

                self.game.player_mut(zone.player).limbo.push(Some(id));

                self.new_public_pointer(id)
            }
            Zone::CardSelection => {
                let id = self.new_secret_card(zone.player, view);

                self.context.mutate_secret(zone.player, |secret, _, _| {
                    secret.card_selection.push(id);
                });

                self.game.player_mut(zone.player).card_selection += 1;

                self.new_public_pointer(id)
            }
            Zone::Casting => {
                let id = self.new_public_card(view);

                self.game.player_mut(zone.player).casting.push(id);

                self.new_public_pointer(id)
            }
            Zone::Dusted { public: false } => {
                let id = self.new_secret_card(zone.player, view);

                self.context.mutate_secret(zone.player, |secret, _, _| {
                    secret.dusted.push(id);
                });

                self.new_public_pointer(id)
            }
            Zone::Dusted { public: true } => {
                let id = self.new_public_card(view);

                self.game.player_mut(zone.player).dusted.push(id);

                self.new_public_pointer(id)
            }
            Zone::Attachment { parent } => {
                match self.game.opaque_ptrs[parent.0] {
                    MaybeSecretInstanceID::Secret(player) => {
                        let owners: Vec<_> = self.game.card_owners().collect();

                        let (owner, id) = self
                            .context
                            .reveal_unique(
                                player,
                                {
                                    let parent = *parent;

                                    move |secret| {
                                        let id = secret.opaque_ptrs[&parent];
                                        let owner = owners[id.0];

                                        // We don't need to reveal the ID if we're modifying internally.

                                        if owner == Some(player) {
                                            (owner, None)
                                        } else {
                                            (owner, Some(id))
                                        }
                                    }
                                },
                                |_| true,
                            )
                            .await;

                        if owner == Some(player) {
                            // Player-internal mutation

                            // Dust the secret card's attachment secretly.

                            todo!();
                        } else if owner == None {
                            // Public card mutation

                            let id = id.expect("no ID was revealed while modifying a public card");

                            self.publish_pointer_id(*parent, player, id);

                            // Dust the public card's attachment publicly.

                            todo!();
                        } else if let Some(owner) = owner {
                            // Cross-player mutation

                            let id = id.expect(
                                "no ID was revealed while modifying another player's secret card",
                            );

                            self.publish_pointer_id(*parent, player, id);

                            // Dust the secret card's attachment secretly.

                            todo!();
                        } else {
                            unreachable!("owner is neither None nor Some");
                        }
                    }
                    MaybeSecretInstanceID::Public(id) => {
                        match &mut self.game.cards[id.0] {
                            MaybeSecretCardView::Secret(player) => {
                                // Dust the secret card's attachment secretly.

                                todo!();
                            }
                            MaybeSecretCardView::Public(parent) => {
                                // Dust the public card's attachment publicly.

                                todo!();

                                let id = self.new_public_card(view);

                                parent.attachment = Some(id);

                                self.new_public_pointer(id)
                            }
                        }
                    }
                }
            }
        }
    }

    /// Gets a pointer to a player's deck card
    pub fn deck_card(&mut self, player: Player, index: usize) -> OpaquePointer {
        let card = OpaquePointer(self.game.opaque_ptrs.len());

        self.context.mutate_secret(player, |secret, _, _| {
            secret.opaque_ptrs.insert(card, secret.deck[index]);
        });

        self.game
            .opaque_ptrs
            .push(MaybeSecretInstanceID::Secret(player));

        card
    }

    /// Gets a pointer to a player's hand card
    pub fn hand_card(&mut self, player: Player, index: usize) -> OpaquePointer {
        match self.game.player(player).hand[index] {
            None => {
                let card = OpaquePointer(self.game.opaque_ptrs.len());

                self.context.mutate_secret(player, |secret, _, _| {
                    secret.opaque_ptrs.insert(
                        card,
                        secret.hand[index].expect("hand card is neither public nor secret"),
                    );
                });

                self.game
                    .opaque_ptrs
                    .push(MaybeSecretInstanceID::Secret(player));

                card
            }
            Some(id) => self.new_public_pointer(id),
        }
    }

    /// Draws a card from a player's deck to their hand
    pub async fn draw_card(&mut self, player: Player) -> Option<OpaquePointer> {
        match self.game.player(player).deck {
            0 => None,
            size => {
                let index =
                    rand::RngCore::next_u32(&mut self.context.random().await) as usize % size;

                let card = self.deck_card(player, index);

                self.move_card(
                    card,
                    &PlayerZone {
                        player,
                        zone: Zone::Hand { public: false },
                    },
                )
                .await;

                Some(card)
            }
        }
    }

    /// Moves a card to another zone.
    pub async fn move_card(&mut self, card: OpaquePointer, to: &PlayerZone) {
        todo!();
    }

    /// Reveals data about a card.
    // hack: T: Secret should be sufficient
    pub async fn reveal_from_card<T: Secret + serde::Serialize + serde::de::DeserializeOwned>(
        &mut self,
        card: OpaquePointer,
        f: impl Fn(&CardView) -> T + Clone + 'static,
    ) -> T {
        match self.game.opaque_ptrs[card.0] {
            MaybeSecretInstanceID::Secret(player) => {
                let owners: Vec<_> = self.game.card_owners().collect();

                // We're going to reveal either the data or where to find it.

                let (data, owner_id) = self
                    .context
                    .reveal_unique(
                        player,
                        {
                            let f = f.clone();

                            move |secret| {
                                let id = secret.opaque_ptrs[&card];
                                let owner = owners[id.0];

                                // We don't need to reveal the location if the data is here.

                                if owner == Some(player) {
                                    (Some(f(&secret.cards[&id])), None)
                                } else {
                                    (None, Some((owner, id)))
                                }
                            }
                        },
                        |_| true,
                    )
                    .await;

                match data {
                    None => {
                        let (owner, id) = owner_id.expect("no data and nowhere to find it");

                        self.publish_pointer_id(card, player, id);

                        match owner {
                            None => {
                                // The secret pointer pointed to a public card.

                                let view = self.game.cards[id.0]
                                    .expect_ref("the card should have been public");

                                f(view)
                            }
                            Some(owner) => {
                                // The secret pointer pointed to the other player's secret card.

                                self.context
                                    .reveal_unique(
                                        owner,
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
            MaybeSecretInstanceID::Public(id) => match &self.game.cards[id.0] {
                MaybeSecretCardView::Secret(player) => {
                    self.context
                        .reveal_unique(*player, move |secret| f(&secret.cards[&id]), |_| true)
                        .await
                }
                MaybeSecretCardView::Public(view) => f(view),
            },
        }
    }

    /// Modifies a card.
    ///
    /// If the card is public, it's modified publicly.
    /// If the card is secret, it's modified secretly.
    pub async fn modify_card(&mut self, card: OpaquePointer, f: impl Fn(&mut CardView)) {
        match self.game.opaque_ptrs[card.0] {
            MaybeSecretInstanceID::Secret(player) => {
                let owners: Vec<_> = self.game.card_owners().collect();

                // We're going to reveal who owns the card.

                // If it's not the same player, we have to reveal the ID.

                let (owner, id) = self
                    .context
                    .reveal_unique(
                        player,
                        move |secret| {
                            let id = secret.opaque_ptrs[&card];
                            let owner = owners[id.0];

                            // We don't need to reveal the ID if we're modifying internally.

                            if owner == Some(player) {
                                (owner, None)
                            } else {
                                (owner, Some(id))
                            }
                        },
                        |_| true,
                    )
                    .await;

                if owner == Some(player) {
                    // Player-internal mutation

                    self.context.mutate_secret(player, |secret, _, _| {
                        f(&mut secret.cards[&secret.opaque_ptrs[&card]]);
                    });
                } else if owner == None {
                    // Public card mutation

                    let id = id.expect("no ID was revealed while modifying a public card");

                    self.publish_pointer_id(card, player, id);

                    f(self.game.cards[id.0].expect_mut("the card should have been public"));
                } else if let Some(owner) = owner {
                    // Cross-player mutation

                    let id = id
                        .expect("no ID was revealed while modifying another player's secret card");

                    self.publish_pointer_id(card, player, id);

                    self.context.mutate_secret(owner, |secret, _, _| {
                        f(&mut secret.cards[&id]);
                    });
                }
            }
            MaybeSecretInstanceID::Public(id) => match &mut self.game.cards[id.0] {
                MaybeSecretCardView::Secret(player) => {
                    self.context.mutate_secret(*player, |secret, _, _| {
                        f(&mut secret.cards[&id]);
                    });
                }
                MaybeSecretCardView::Public(view) => {
                    f(view);
                }
            },
        }
    }

    /// Mutates a player's secret information.
    pub async fn mutate_secret(
        &mut self,
        player: crate::Player,
        mutate: impl Fn(&mut CardGameSecret, &mut dyn rand::RngCore, &mut dyn FnMut(&dyn Event)),
    ) {
        let next_id = InstanceID(self.game.cards.len());
        let next_ptr = OpaquePointer(self.game.opaque_ptrs.len());
        let public_state = self.game.player(player);

        self.context.mutate_secret(player, |secret, random, log| {
            secret.next_id = next_id;
            secret.next_ptr = next_ptr;
            secret.public_state = public_state.clone();

            mutate(secret, random, log);
        });

        let (next_id, next_ptr, public_state) = self
            .context
            .reveal_unique(
                player,
                |secret| (secret.next_id, secret.next_ptr, secret.public_state.clone()),
                |_| true,
            )
            .await;

        while self.game.cards.len() < next_id.0 {
            self.game.cards.push(MaybeSecretCardView::Secret(player));
        }

        while self.game.opaque_ptrs.len() < next_ptr.0 {
            self.game
                .opaque_ptrs
                .push(MaybeSecretInstanceID::Secret(player));
        }

        self.game.players[usize::from(player)] = public_state;
    }

    /// Requests a player's secret information.
    ///
    /// The random number generator is re-seeded after this call to prevent players from influencing the randomness of the state via trial and error.
    ///
    /// See [LiveGame::reveal_unique] for a faster non-re-seeding version of this method.
    pub async fn reveal<T: Secret>(
        &mut self,
        player: crate::Player,
        reveal: impl Fn(&CardGameSecret) -> T + 'static,
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
    pub async fn reveal_unique<T: Secret>(
        &mut self,
        player: crate::Player,
        reveal: impl Fn(&CardGameSecret) -> T + 'static,
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

    /// Creates a dangling card in public state.
    ///
    /// The returned instance ID must be added to some collection.
    #[must_use = "instance ID must be added to some collection"]
    fn new_public_card(&mut self, mut view: CardView) -> InstanceID {
        let id = InstanceID(self.game.cards.len());

        view.id = id;

        self.game.cards.push(MaybeSecretCardView::Public(view));

        id
    }

    /// Creates a dangling card in a player's secret state.
    ///
    /// This is for creating a secret card from a public card view.
    /// If you want to create a secret card from a secret card view, use [CardGameSecret::new_card] from within a [LiveGame::mutate_secret] instead.
    ///
    /// The returned instance ID must be added to some collection.
    #[must_use = "instance ID must be added to some collection"]
    fn new_secret_card(&mut self, player: Player, mut view: CardView) -> InstanceID {
        let id = InstanceID(self.game.cards.len());

        view.id = id;

        self.context.mutate_secret(player, |secret, _, _| {
            secret.cards.insert(id, view.clone());
        });

        self.game.cards.push(MaybeSecretCardView::Secret(player));

        id
    }

    /// Creates a public opaque pointer to a concrete instance ID.
    fn new_public_pointer(&mut self, id: InstanceID) -> OpaquePointer {
        let ptr = OpaquePointer(self.game.opaque_ptrs.len());

        self.game
            .opaque_ptrs
            .push(MaybeSecretInstanceID::Public(id));

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

        self.game.opaque_ptrs[ptr.0] = MaybeSecretInstanceID::Public(id);
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct CardGame {
    cards: Vec<MaybeSecretCardView>,

    opaque_ptrs: Vec<MaybeSecretInstanceID>,

    players: [PlayerState; 2],
}

impl CardGame {
    pub fn cards(&self) -> &Vec<MaybeSecretCardView> {
        &self.cards
    }

    pub fn card_owners(&self) -> impl Iterator<Item = Option<Player>> + '_ {
        self.cards.iter().map(MaybeSecretCardView::player)
    }

    pub fn opaque_ptrs(&self) -> &Vec<MaybeSecretInstanceID> {
        &self.opaque_ptrs
    }

    pub fn player(&self, player: Player) -> &PlayerState {
        &self.players[usize::from(player)]
    }

    pub fn player_mut(&mut self, player: Player) -> &mut PlayerState {
        &mut self.players[usize::from(player)]
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct PlayerState {
    deck: usize,

    hand: Vec<Option<InstanceID>>,

    field: Vec<InstanceID>,

    graveyard: Vec<InstanceID>,

    limbo: Vec<Option<InstanceID>>,

    card_selection: usize,

    casting: Vec<InstanceID>,

    dusted: Vec<InstanceID>,
}

impl PlayerState {
    pub fn deck(&self) -> usize {
        self.deck
    }

    pub fn hand(&self) -> &Vec<Option<InstanceID>> {
        &self.hand
    }

    pub fn field(&self) -> &Vec<InstanceID> {
        &self.field
    }

    pub fn graveyard(&self) -> &Vec<InstanceID> {
        &self.graveyard
    }

    pub fn limbo(&self) -> &Vec<Option<InstanceID>> {
        &self.limbo
    }

    pub fn card_selection(&self) -> usize {
        self.card_selection
    }

    pub fn casting(&self) -> &Vec<InstanceID> {
        &self.casting
    }

    pub fn dusted(&self) -> &Vec<InstanceID> {
        &self.dusted
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct CardGameSecret {
    cards: indexmap::IndexMap<InstanceID, CardView>,

    opaque_ptrs: indexmap::IndexMap<OpaquePointer, InstanceID>,

    deck: Vec<InstanceID>,

    hand: Vec<Option<InstanceID>>,

    limbo: Vec<Option<InstanceID>>,

    dusted: Vec<InstanceID>,

    card_selection: Vec<InstanceID>,

    next_id: InstanceID,

    next_ptr: OpaquePointer,

    public_state: PlayerState,
}

impl CardGameSecret {
    pub fn cards(&self) -> &indexmap::IndexMap<InstanceID, CardView> {
        &self.cards
    }

    pub fn opaque_ptrs(&self) -> &indexmap::IndexMap<OpaquePointer, InstanceID> {
        &self.opaque_ptrs
    }

    pub fn deck(&self) -> &Vec<InstanceID> {
        &self.deck
    }

    pub fn hand(&self) -> &Vec<Option<InstanceID>> {
        &self.hand
    }

    pub fn limbo(&self) -> &Vec<Option<InstanceID>> {
        &self.limbo
    }

    pub fn dusted(&self) -> &Vec<InstanceID> {
        &self.dusted
    }

    pub fn card_selection(&self) -> &Vec<InstanceID> {
        &self.card_selection
    }

    /// Creates a card in a secret zone.
    ///
    /// If attaching to a card, both the opaque pointer and its card must also be secret.
    pub fn new_card(&mut self, mut view: CardView, zone: &Zone) -> InstanceID {
        let id = self.next_id;

        view.id = id;

        self.cards.insert(id, view);

        match zone {
            Zone::Deck => {
                self.deck.push(id);

                self.public_state.deck += 1;
            }
            Zone::Hand { public: false } => {
                self.hand.push(Some(id));

                self.public_state.hand.push(None);
            }
            Zone::Limbo { public: false } => {
                self.limbo.push(Some(id));

                self.public_state.limbo.push(None);
            }
            Zone::CardSelection => {
                self.card_selection.push(id);

                self.public_state.card_selection += 1;
            }
            Zone::Dusted { public: false } => self.dusted.push(id),
            Zone::Attachment { parent } => todo!(),
            _ => unreachable!(),
        }

        self.next_id = InstanceID(id.0 + 1);

        id
    }
}

pub struct PlayerZone {
    pub player: Player,
    pub zone: Zone,
}

pub enum Zone {
    Deck,
    Hand { public: bool },
    Field,
    Graveyard,
    Limbo { public: bool },
    CardSelection,
    Casting,
    Dusted { public: bool },
    Attachment { parent: OpaquePointer },
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub enum MaybeSecretInstanceID {
    Secret(Player),
    Public(InstanceID),
}

impl MaybeSecretInstanceID {
    pub fn player(&self) -> Option<Player> {
        match self {
            Self::Secret(player) => Some(*player),
            Self::Public(..) => None,
        }
    }

    pub fn id(&self) -> Option<InstanceID> {
        match self {
            Self::Secret(..) => None,
            Self::Public(id) => Some(*id),
        }
    }

    pub fn expect(&self, message: &str) -> InstanceID {
        self.id().expect(message)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Hash, Eq, PartialEq, Default)]
pub struct OpaquePointer(usize);

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Hash, Eq, PartialEq, Default)]
pub struct InstanceID(usize);

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub enum MaybeSecretCardView {
    Secret(Player),
    Public(CardView),
}

impl MaybeSecretCardView {
    pub fn player(&self) -> Option<Player> {
        match self {
            Self::Secret(player) => Some(*player),
            Self::Public(..) => None,
        }
    }

    pub fn view(self) -> Option<CardView> {
        match self {
            Self::Secret(..) => None,
            Self::Public(view) => Some(view),
        }
    }

    pub fn view_ref(&self) -> Option<&CardView> {
        match self {
            Self::Secret(..) => None,
            Self::Public(view) => Some(view),
        }
    }

    pub fn view_mut(&mut self) -> Option<&mut CardView> {
        match self {
            Self::Secret(..) => None,
            Self::Public(view) => Some(view),
        }
    }

    pub fn expect(self, message: &str) -> CardView {
        self.view().expect(message)
    }

    pub fn expect_ref(&self, message: &str) -> &CardView {
        self.view_ref().expect(message)
    }

    pub fn expect_mut(&mut self, message: &str) -> &mut CardView {
        self.view_mut().expect(message)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct CardView {
    id: InstanceID,

    attachment: Option<InstanceID>,
}

impl CardView {
    pub fn new() -> Self {
        Self {
            id: InstanceID(0),
            attachment: None,
        }
    }

    pub fn id(&self) -> InstanceID {
        self.id
    }

    pub fn attachment(&self) -> &Option<InstanceID> {
        &self.attachment
    }
}

impl State for CardGame {
    type ID = CardGameID;

    type Nonce = u32;

    type Action = ();

    type Secret = CardGameSecret;

    fn version() -> &'static [u8] {
        // todo!();

        "todo!".as_bytes()
    }

    // fn challenge(address: &crate::crypto::Address) -> String {
    //     format!(
    //         "Sign to play! This won't cost anything.\n\n{}\n",
    //         crate::crypto::Addressable::eip55(address)
    //     )
    // }

    fn deserialize(data: &[u8]) -> Result<Self, String> {
        serde_cbor::from_slice(data).map_err(|error| error.to_string())
    }

    // fn is_serializable(&self) -> bool {
    //     self.serialize().is_some()
    // }

    fn serialize(&self) -> Option<Vec<u8>> {
        serde_cbor::to_vec(self).ok()
    }

    fn verify(&self, player: Option<Player>, action: &Self::Action) -> Result<(), String> {
        // todo!();

        Ok(())
    }

    fn apply(
        self,
        _player: Option<Player>,
        _action: &Self::Action,
        context: Context<Self>,
    ) -> Pin<Box<dyn Future<Output = (Self, Context<Self>)>>> {
        Box::pin(async {
            let mut live_game = LiveGame {
                game: self,
                context,
            };

            live_game.invalidate_pointers();

            for _ in 0..5 {
                let card = live_game
                    .new_card(
                        CardView::new(),
                        &PlayerZone {
                            player: 0,
                            zone: Zone::Limbo { public: true },
                        },
                    )
                    .await;

                live_game
                    .move_card(
                        card,
                        &PlayerZone {
                            player: 0,
                            zone: Zone::Deck,
                        },
                    )
                    .await;

                let card = live_game
                    .new_card(
                        CardView::new(),
                        &PlayerZone {
                            player: 0,
                            zone: Zone::Limbo { public: true },
                        },
                    )
                    .await;

                live_game
                    .move_card(
                        card,
                        &PlayerZone {
                            player: 0,
                            zone: Zone::Deck,
                        },
                    )
                    .await;

                let attachment = live_game
                    .new_card(
                        CardView::new(),
                        &PlayerZone {
                            player: 0,
                            zone: Zone::Limbo { public: true },
                        },
                    )
                    .await;

                live_game
                    .move_card(
                        attachment,
                        &PlayerZone {
                            player: 0,
                            zone: Zone::Attachment { parent: card },
                        },
                    )
                    .await;
            }

            (live_game.game, live_game.context)
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Default)]
pub struct CardGameID([u8; 8]);

impl ID for CardGameID {
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
fn test_generic_card_game() {
    Tester::new(CardGame::default(), Default::default(), Default::default())
        .unwrap()
        .apply(0, &())
        .unwrap();
}
