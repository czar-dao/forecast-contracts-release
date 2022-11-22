use crate::{Config, FinishedRound, LiveRound, NextRound};
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

pub const IS_HAULTED: Item<bool> = Item::new("is_haulted");
pub const CONFIG: Item<Config> = Item::new("config");
pub const NEXT_ROUND_ID: Item<u128> = Item::new("next_round_id");
/* The round that's open for betting */
pub const NEXT_ROUND: Item<NextRound> = Item::new("next_round");
/* The live round; not accepting bets */
pub const LIVE_ROUND: Item<LiveRound> = Item::new("live_round");
/* Winnings (per-wallet) that can be claimed from the pool  */
pub const SETTLE_DENOM: Item<String> = Item::new("settle_denom");
/* Bears in a given round */
pub const BEAR_BETS: Map<(u128, Addr), u128> = Map::new("bear_bets");
/* Bulls in a given round */
pub const BULL_BETS: Map<(u128, Addr), u128> = Map::new("bull_bets");
/* Bulls in a given round */
pub const ACCUMULATED_FEE: Item<u128> = Item::new("accumulated_fee");

pub const MY_CLAIMED_ROUNDS: Map<(Addr, u128), bool> =
    Map::new("my_claimed_rounds");

pub const ROUNDS: Map<u128, FinishedRound> = Map::new("rounds");
