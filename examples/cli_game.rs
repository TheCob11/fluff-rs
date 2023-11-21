use std::fmt::{Display, Formatter};

use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

use fluff::{
    bet::Bet,
    game::{
        self, round,
        state::{self, Betting, InRound, NewRound},
        Game,
    },
    player::Player,
};

#[derive(Debug, Copy, Clone)]
struct BetInput(pub Bet);

impl Display for BetInput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::str::FromStr for BetInput {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim().to_lowercase();
        let v = trimmed.split_whitespace().collect::<Vec<_>>();
        if v.len() != 2 {
            return Err(Self::Err::msg(format!(
                "{} arg(s) given instead of 2",
                v.len()
            )));
        };
        Ok(Self(Bet::new(v[0].parse()?, v[1].parse()?)))
    }
}

static THEME: std::sync::OnceLock<ColorfulTheme> = std::sync::OnceLock::new();

fn theme() -> &'static ColorfulTheme {
    THEME.get_or_init(ColorfulTheme::default)
}

impl BetInput {
    pub fn input_with_confirm(curr_bet: Option<Bet>) -> dialoguer::Result<Option<Bet>> {
        let bet = Input::<Self>::with_theme(theme())
            .with_prompt("Input your bet as \"<count> <roll>\"")
            .validate_with(|input: &BetInput| {
                curr_bet.map_or_else(
                    || Ok(()),
                    |ref actual_bet| input.0.is_raised_from(actual_bet),
                )
            })
            .interact_text()?
            .0;
        Ok(Confirm::with_theme(theme())
            .with_prompt(format!("Confirm bet of {bet}?"))
            .default(true)
            .interact()?
            .then_some(bet))
    }
}

pub fn explain_fluff_result(transition: &game::FluffCallTransition) {
    let round = match transition {
        game::FluffCallTransition::NextRound(g) => g
            .round_history()
            .last()
            .expect("transitioned game should not have empty round history"),
        game::FluffCallTransition::GameOver(g) => g
            .round_history()
            .last()
            .expect("finished game should not have empty round history"),
    };
    let round::Turn {
        player: _,
        bet: final_bet @ Bet {
            count: bet_count,
            roll: bet_roll,
        },
    } = round
        .turns()
        .last()
        .expect("A called round should not have an empty turns");
    let call_data @ state::Called {
        caller,
        better,
        was_fluff,
    } = round.state_data();
    let (winner, loser) = (round.state_data().winner(), round.state_data().loser());
    println!("{caller} called fluff on the bet of {final_bet} made by {better}!\nRolls:");
    let (total_count, loser_dice_count) = {
        let mut running_total_count = 0;
        let mut loser_dice_count: Option<usize> = None;
        for (player, rolls) in round.players_rolls() {
            if player == call_data.loser() {
                loser_dice_count = Some(rolls.len());
            }
            let match_count = final_bet.count_matches(rolls.iter().copied());
            running_total_count += match_count;
            println!("{player} had {rolls:?}: {match_count} effective {bet_roll}(s) => current total {running_total_count}");
        }
        (running_total_count, loser_dice_count.expect("Loser should have been iterated through(where loser dice count should have been assigned) with the rest of the players rolls"))
    };
    if (total_count < bet_count.get()).ne(was_fluff) {
        let error = anyhow::anyhow!(
            "ERROR: Discrepancy in determined was_fluff values: The round data has a stored value of {was_fluff}, but I disagree based on a total of {total_count} effective {bet_roll}(s). The stored round data is probably more trustworthy"
        );
        eprintln!("{error}");
    }
    // println!("was fluff: {was_fluff}");
    println!(
        "{total_count} {bet_roll}(s) is {relationship} {bet_count}, so {winner} is correct and {loser} loses the round",
        relationship = match total_count.cmp(&bet_count.get()) {
            std::cmp::Ordering::Greater => "greater than",
            std::cmp::Ordering::Equal => "equal to",
            std::cmp::Ordering::Less => "less than",
        }
    );
    println!(
        "Since {loser} lost this round, their dice count goes from {loser_dice_count} to {}",
        loser_dice_count - 1
    );
}

#[inline]
fn clear_term() -> std::io::Result<()> {
    dialoguer::console::Term::stdout().clear_screen()
}

fn wait_player_ready(player: &game::PlayerRef) -> dialoguer::Result<()> {
    loop {
        if Confirm::with_theme(theme())
            .with_prompt(format!("Player {player} ready?"))
            .report(false)
            .interact()?
        {
            break;
        }
    }
    Ok(())
}

pub fn run_round(game: Game<InRound<NewRound>>) -> dialoguer::Result<game::FluffCallTransition> {
    println!("Current dice counts: ");
    for (player, dice_count) in game.player_dice_counts() {
        println!("{player} has {dice_count}");
    }
    wait_player_ready(&game.curr_round().state_data().first_player_rolls.player)?;
    let mut g: Game<InRound<Betting>> = loop {
        let NewRound {
            first_player_rolls: round::PlayerRolls { player, rolls },
        } = game.curr_round().state_data();
        println!("Turn of player {player} with rolls {rolls:?}");
        if let Some(bet) = BetInput::input_with_confirm(None)? {
            break game.raise_bet(bet);
        }
    };
    loop {
        clear_term()?;
        println!("Current dice counts: ");
        for (player, dice_count) in g.player_dice_counts() {
            println!("{player} has {dice_count}");
        }
        let Betting {
            curr_player_rolls: round::PlayerRolls { player, rolls },
            prev_bet,
        } = g.curr_round().state_data();
        println!("Current bet: {prev_bet}");
        wait_player_ready(&g.curr_round().state_data().curr_player_rolls.player)?;
        println!("Turn of player {player}, with rolls {rolls:?}");
        if Select::with_theme(theme())
            .with_prompt("Do you want to raise the bet or call Fluff?")
            .items(&["Raise", "Call"])
            .default(0)
            .interact()?
            == 1
            && Confirm::with_theme(theme())
                .with_prompt(format!(
                    "Are you sure you want to call Fluff on {prev_bet}?"
                ))
                .interact()?
        {
            return Ok(g.call_fluff());
        }
        if let Some(bet) = BetInput::input_with_confirm(Some(*prev_bet))? {
            if let Err(err) = g.raise_bet(bet) {
                eprintln!("{err:#?}");
                continue;
            };
        }
    }
}

pub fn run_game(mut g: Game<InRound<NewRound>>) -> dialoguer::Result<Game<state::GameOver>> {
    // println!("{g:#?}");
    // println!("{:#?}", g.curr_round().curr_bet());
    loop {
        use game::FluffCallTransition as T;
        let transition = run_round(g)?;
        clear_term()?;
        explain_fluff_result(&transition);
        println!();
        match transition {
            T::NextRound(new_game) => g = new_game,
            T::GameOver(finished_game) => return Ok(finished_game),
        };
    }
}

pub fn prompt_players() -> dialoguer::Result<std::collections::HashSet<Player>> {
    let mut players = std::collections::HashSet::new();
    loop {
        if !players.is_empty() {
            println!(
                "Current players: [{}]",
                players
                    .iter()
                    .map(Player::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            if players.len() > 1
                && Confirm::with_theme(theme())
                    .with_prompt("Use these players?")
                    .interact()?
            {
                return Ok(players);
            }
        }
        let player = Input::with_theme(theme())
            .with_prompt("Player name?")
            .validate_with(|x: &_| {
                if players.contains(x) {
                    Err("Player already in game")
                } else {
                    Ok(())
                }
            })
            .interact_text()?;
        if Confirm::with_theme(theme())
            .with_prompt(format!("Add player {player}?"))
            .default(true)
            .interact()?
        {
            players.insert(player);
        }
        clear_term()?;
    }
}

pub fn main() -> dialoguer::Result<()> {
    Ok(println!(
        "{:#?}",
        run_game(Game::new(prompt_players()?, game::GameConfig::default()))?
    ))
}
