use std::num::NonZeroUsize;

use indexmap::IndexMap;

pub use round::Round;
use state::{Betting, Called, GameOver, GameState, InRound, NewRound, RoundState};

use crate::{
    bet::{self, Bet},
    player::Player,
};

pub mod round;
pub mod state;

pub type PlayerRef = std::sync::Arc<Player>;

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Copy, Clone)]
pub struct GameConfig {
    max_dice: NonZeroUsize,
    max_roll: NonZeroUsize,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            max_dice: NonZeroUsize::new(5).unwrap(),
            max_roll: NonZeroUsize::new(6).unwrap(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct Game<State: GameState = InRound<NewRound>> {
    player_dice_counts: IndexMap<PlayerRef, usize>,
    config: GameConfig,
    round_history: Vec<Round<Called>>,
    state_data: State,
}

impl Game {
    pub fn new(
        players: impl IntoIterator<Item = Player>,
        config: GameConfig,
    ) -> Game<InRound<NewRound>> {
        let player_dice_counts = players
            .into_iter()
            .map(|x| (PlayerRef::from(x), config.max_dice.get()))
            .collect();
        let curr_round = Round::new(&player_dice_counts, config.max_roll);
        Game {
            player_dice_counts,
            config,
            round_history: Vec::new(),
            state_data: InRound { curr_round },
        }
    }
}

impl<T: GameState> Game<T> {
    pub fn round_history(&self) -> &Vec<Round<Called>> {
        &self.round_history
    }

    pub fn player_dice_counts(&self) -> &IndexMap<PlayerRef, usize> {
        &self.player_dice_counts
    }
}

impl<T: RoundState> Game<InRound<T>> {
    #[must_use]
    pub fn curr_round(&self) -> &Round<T> {
        &self.state_data.curr_round
    }
}

impl Game<InRound<NewRound>> {
    #[must_use]
    pub fn raise_bet(self, bet: Bet) -> Game<InRound<Betting>> {
        Game {
            player_dice_counts: self.player_dice_counts,
            config: self.config,
            round_history: self.round_history,
            state_data: InRound {
                curr_round: self.state_data.curr_round.raise_bet(bet),
            },
        }
    }
}

pub enum FluffCallTransition {
    NextRound(Game<InRound<NewRound>>),
    GameOver(Game<GameOver>),
}

impl From<Game<InRound<NewRound>>> for FluffCallTransition {
    fn from(value: Game<InRound<NewRound>>) -> Self {
        Self::NextRound(value)
    }
}

impl From<Game<GameOver>> for FluffCallTransition {
    fn from(value: Game<GameOver>) -> Self {
        Self::GameOver(value)
    }
}

impl Game<InRound<Betting>> {
    pub fn raise_bet(&mut self, bet: Bet) -> Result<(), bet::RaiseError> {
        self.state_data.curr_round.raise_bet(bet)?;
        Ok(())
    }

    #[must_use]
    pub fn call_fluff(self) -> FluffCallTransition {
        let finished_round = self.state_data.curr_round.call_fluff();
        let winner = finished_round.state_data().winner().clone();
        let (player_is_out, player_dice_counts) = {
            let mut player_dice_counts = self.player_dice_counts;
            let round_loser_dice_count: &mut _ = player_dice_counts
                .get_mut(finished_round.state_data().loser())
                .expect("This should be getting player dice counts at the loser of the finished round, which should exist");
            *round_loser_dice_count -= 1;
            (*round_loser_dice_count == 0, player_dice_counts)
        };
        let config = self.config;
        let round_history = {
            let mut round_history = self.round_history;
            round_history.push(finished_round);
            round_history
        };
        if player_is_out && player_dice_counts.values().filter(|x| **x != 0).count() == 1 {
            return FluffCallTransition::GameOver(Game {
                player_dice_counts,
                config,
                round_history,
                state_data: GameOver { winner },
            });
        };
        let new_round = Round::new_with_first_player(&player_dice_counts, config.max_roll, &winner)
            .expect("Winner of previous round should be in player dice counts");
        FluffCallTransition::NextRound(Game {
            player_dice_counts,
            config,
            round_history,
            state_data: InRound {
                curr_round: new_round,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{from_str, to_string_pretty};

    use super::*;

    #[test]
    fn test_serde() {
        let g: Game = Game::new(
            [
                Player::new("Unga"),
                Player::new("Bunga"),
                Player::new("Ooga"),
                Player::new("Booga"),
            ],
            GameConfig::default(),
        );
        let g_ser = to_string_pretty(&g).unwrap();
        println!("{g_ser}");
        let g_clone = g.clone();
        drop(g);
        let g_de = from_str::<Game>(g_ser.as_str()).unwrap();
        assert_eq!(g_clone, g_de);
        // println!("\n\n\n{g_de:#?}\n\n\n{g_clone:#?}");
    }
}
