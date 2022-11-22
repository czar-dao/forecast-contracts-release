use cosmwasm_std::{Addr, Timestamp, Uint128};
use partial_derive::Partial;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const FEE_PRECISION: u128 = 100u128;

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Bull,
    Bear,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct MigrateMsg {}

impl ToString for Direction {
    fn to_string(&self) -> String {
        match self {
            Direction::Bull => "bull",
            Direction::Bear => "bear",
        }
        .to_string()
    }
}

#[derive(Partial)]
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
/**
 * Parameters which are mutable by a governance vote
 */
pub struct Config {
    /* After a round ends this is the duration of the next */
    pub next_round_seconds: Uint128,
    pub fast_oracle_addr: Addr,
    pub cw20_stake_external_rewards_addr: Addr,
    pub minimum_bet: Uint128,
    pub burn_fee: Uint128,
    pub burn_addr: Addr,
    pub staker_fee: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct NextRound {
    pub id: Uint128,
    pub bid_time: Timestamp,
    pub open_time: Timestamp,
    pub close_time: Timestamp,
    pub bull_amount: Uint128,
    pub bear_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct LiveRound {
    pub id: Uint128,
    pub bid_time: Timestamp,
    pub open_time: Timestamp,
    pub close_time: Timestamp,
    pub open_price: Uint128,
    pub bull_amount: Uint128,
    pub bear_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct FinishedRound {
    pub id: Uint128,
    pub bid_time: Timestamp,
    pub open_time: Timestamp,
    pub close_time: Timestamp,
    pub open_price: Uint128,
    pub close_price: Uint128,
    pub winner: Option<Direction>,
    pub bull_amount: Uint128,
    pub bear_amount: Uint128,
}

pub mod msg {
    use super::*;

    #[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub struct InstantiateMsg {
        /* Mutable params */
        pub config: Config,
        /* Immutable params */
        pub settle_denom: String,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum ExecuteMsg {
        /**
         * Update part of or all of the mutable config params
         */
        UpdateConfig {
            config: PartialConfig,
        },
        /**
         * Price go up
         */
        BetBull {
            /* In case the TX is delayed */
            round_id: Uint128,
        },
        /**
         * Price go down
         */
        BetBear {
            /* In case the TX is delayed */
            round_id: Uint128,
        },
        /**
         * Permissionless msg to close the current round and open the next
         * NOTE It is permissionless because we can check timestamps :)
         */
        CloseRound {},
        /**
         * Settle winnings for an account
         */
        CollectWinnings {
            rounds: Vec<Uint128>,
        },
        FundStakers {},
        Hault {},
        Resume {},
    }

    #[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum QueryMsg {
        Config {},
        Status {},
        MyCurrentPosition { address: String },
        FinishedRound { round_id: Uint128 },
    }
}

pub mod response {
    use super::*;

    pub type ConfigResponse = Config;

    pub type RoundResponse = FinishedRound;

    #[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub struct StatusResponse {
        pub bidding_round: Option<NextRound>,
        pub live_round: Option<LiveRound>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub struct MyCurrentPositionResponse {
        pub live_bear_amount: Uint128,
        pub live_bull_amount: Uint128,
        pub next_bear_amount: Uint128,
        pub next_bull_amount: Uint128,
    }
}
