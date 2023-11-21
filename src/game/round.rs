use std::num::NonZeroUsize;

use indexmap::IndexMap;
use rand::{
    distributions::{Distribution, Uniform},
    thread_rng,
};

use crate::{
    bet::Bet,
    game::{state::UnfinishedRound, Betting, Called, NewRound, PlayerRef, RoundState},
};

type RollSet = std::sync::Arc<[NonZeroUsize]>;

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct PlayerRolls {
    pub player: PlayerRef,
    pub rolls: RollSet,
}

impl From<(PlayerRef, RollSet)> for PlayerRolls {
    fn from(value: (PlayerRef, RollSet)) -> Self {
        Self {
            player: value.0,
            rolls: value.1,
        }
    }
}

impl From<(&PlayerRef, &RollSet)> for PlayerRolls {
    //note: this clones, im not sure if it shouldn't semantically
    fn from(value: (&PlayerRef, &RollSet)) -> Self {
        (value.0.clone(), value.1.clone()).into()
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct Round<State: RoundState = NewRound> {
    players_rolls: IndexMap<PlayerRef, RollSet>,
    turns: Vec<Turn>,
    state_data: State,
}

impl<State: RoundState> Round<State> {
    pub fn state_data(&self) -> &State {
        &self.state_data
    }
}

impl<State: UnfinishedRound> Round<State> {
    fn init_next_state(&self, turn: &Turn) -> Betting {
        let next_player_index = (self
            .players_rolls
            .get_index_of(&turn.player)
            .expect("Current player should be in player rolls")
            + 1)
            % self.players_rolls.len();
        let next_player_rolls = self
            .players_rolls
            .get_index(next_player_index)
            .expect("Next player index should be in player rolls")
            .into();
        Betting {
            curr_player_rolls: next_player_rolls,
            prev_bet: turn.bet,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct FirstPlayerNotInGivenPlayers {}

impl Round<NewRound> {
    #[must_use]
    pub fn new(
        player_dice_counts: &IndexMap<PlayerRef, usize>,
        max_roll: NonZeroUsize,
    ) -> Round<NewRound> {
        let dist = Uniform::new_inclusive(1, max_roll.get());
        let rolls: IndexMap<PlayerRef, RollSet> = player_dice_counts
            .into_iter()
            .filter(|(_, dice_count)| 0.ne(*dice_count))
            .map(|(player_ref, dice_count)| {
                (
                    player_ref.clone(),
                    dist.sample_iter(thread_rng())
                        .take(*dice_count)
                        .filter_map(NonZeroUsize::new)
                        .collect(),
                )
            })
            .collect();
        let first_player_rolls = rolls
            .first()
            .expect("Players rolls should not be empty")
            .into();
        Round {
            players_rolls: rolls,
            turns: Vec::new(),
            state_data: NewRound { first_player_rolls },
        }
    }

    pub fn new_with_first_player(
        player_dice_counts: &IndexMap<PlayerRef, usize>,
        max_roll: NonZeroUsize,
        first_player: &PlayerRef,
    ) -> Result<Round<NewRound>, FirstPlayerNotInGivenPlayers> {
        let mut round = Self::new(player_dice_counts, max_roll);
        round.state_data.first_player_rolls = round
            .players_rolls
            .get_key_value(first_player)
            .ok_or(FirstPlayerNotInGivenPlayers {})?
            .into();
        Ok(round)
    }

    #[must_use]
    pub fn raise_bet(self, bet: Bet) -> Round<Betting> {
        let turn = Turn {
            player: self.state_data.first_player_rolls.player.clone(),
            bet,
        };
        let state_data = self.init_next_state(&turn);
        let turns = {
            let mut turns = self.turns;
            turns.push(turn);
            turns
        };
        Round {
            players_rolls: self.players_rolls,
            turns,
            state_data,
        }
    }
}

impl Round<Betting> {
    fn is_fluff(&self) -> bool {
        self.state_data
            .prev_bet
            .is_fluff(self.players_rolls.values().flat_map(|x| x.iter().copied()))
    }

    pub fn raise_bet(&mut self, bet: Bet) -> Result<(), crate::bet::RaiseError> {
        bet.is_raised_from(&self.state_data.prev_bet)?;
        let turn = Turn {
            player: self.state_data.curr_player_rolls.player.clone(),
            bet,
        };
        self.state_data = self.init_next_state(&turn);
        self.turns.push(turn);
        Ok(())
    }

    #[must_use]
    pub fn call_fluff(self) -> Round<Called> {
        let was_fluff = self.is_fluff();
        let caller = self.state_data.curr_player_rolls.player.clone();
        let better = self
            .turns
            .last()
            .expect("There should be past rounds, otherwise this shouldn't be Betting")
            .player
            .clone();
        Round {
            players_rolls: self.players_rolls,
            turns: self.turns,
            state_data: Called {
                caller,
                better,
                was_fluff,
            },
        }
    }
}

impl Round<Called> {
    #[must_use]
    pub fn turns(&self) -> &Vec<Turn> {
        &self.turns
    }

    #[must_use]
    pub fn players_rolls(&self) -> &IndexMap<PlayerRef, RollSet> {
        &self.players_rolls
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Turn {
    pub player: PlayerRef,
    pub bet: Bet,
}
