use crate::{
    bet::Bet,
    game::{
        round::{PlayerRolls, Round},
        PlayerRef,
    },
};

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct InRound<State: RoundState> {
    pub curr_round: Round<State>,
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct GameOver {
    pub winner: PlayerRef,
}

pub trait GameState {}

impl<State: RoundState> GameState for InRound<State> {}

impl GameState for GameOver {}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct NewRound {
    pub first_player_rolls: PlayerRolls,
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct Betting {
    pub curr_player_rolls: PlayerRolls,
    pub prev_bet: Bet,
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct Called {
    pub caller: PlayerRef,
    pub better: PlayerRef,
    pub was_fluff: bool,
}

impl Called {
    #[inline]
    #[must_use]
    pub const fn loser(&self) -> &PlayerRef {
        if self.was_fluff {
            &self.better
        } else {
            &self.caller
        }
    }

    #[inline]
    #[must_use]
    pub const fn winner(&self) -> &PlayerRef {
        if self.was_fluff {
            &self.caller
        } else {
            &self.better
        }
    }
}

pub trait RoundState {}

impl RoundState for NewRound {}

impl RoundState for Betting {}

impl RoundState for Called {}

pub trait UnfinishedRound: RoundState {}

impl UnfinishedRound for NewRound {}

impl UnfinishedRound for Betting {}
