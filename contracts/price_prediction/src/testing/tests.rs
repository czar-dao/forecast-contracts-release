use cosmwasm_std::{
    coins, to_binary, Addr, Binary, BlockInfo, Coin, CosmosMsg, Empty,
    Response, StdResult, Timestamp, Uint128, WasmMsg,
};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use forecast_deliverdao::fast_oracle::{
    msg::ExecuteMsg as FastOracleExecuteMsg,
    msg::InstantiateMsg as FastOracleInstantiateMsg,
    msg::QueryMsg as FastOracleQueryMsg,
};
use forecast_deliverdao::price_prediction::{
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    response::{ConfigResponse, StatusResponse},
    Config, PartialConfig,
};
use stake_cw20::msg::ReceiveMsg as StakeCw20ReceiveMsg;
use std::borrow::BorrowMut;
use std::convert::TryInto;

const SETTLE_DENOM: &str = "earth";

fn mock_app() -> App {
    App::default()
}

pub fn contract_price_prediction() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    Box::new(contract)
}

pub fn contract_external_rewards() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        |_deps, _, _info, msg: StakeCw20ReceiveMsg| -> StdResult<Response> {
            match msg {
                StakeCw20ReceiveMsg::Fund {} => Ok(Response::default()),
                StakeCw20ReceiveMsg::Stake {} => Ok(Response::default()),
            }
        },
        |_deps, _, _, _: FastOracleInstantiateMsg| -> StdResult<Response> {
            Ok(Response::default())
        },
        |_deps, _, _msg: FastOracleQueryMsg| -> StdResult<Binary> {
            to_binary(&{})
        },
    );
    Box::new(contract)
}

pub fn contract_fast_oracle() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        |deps, _, _info, msg: FastOracleExecuteMsg| -> StdResult<Response> {
            match msg {
                FastOracleExecuteMsg::Update { price } => {
                    deps.storage.set(b"price", &price.to_be_bytes());
                    Ok(Response::default())
                }
                FastOracleExecuteMsg::Owner { owner: _ } => todo!(),
            }
        },
        |deps, _, _, _: FastOracleInstantiateMsg| -> StdResult<Response> {
            deps.storage
                .set(b"price", &Uint128::new(1_000_000u128).to_be_bytes());
            Ok(Response::default())
        },
        |deps, _, msg: FastOracleQueryMsg| -> StdResult<Binary> {
            match msg {
                FastOracleQueryMsg::Price {} => {
                    let res = deps.storage.get(b"price").unwrap_or_default();
                    let price = Uint128::from(u128::from_be_bytes(
                        res.as_slice().try_into().unwrap(),
                    ));

                    to_binary(&price)
                }
            }
        },
    );
    Box::new(contract)
}

fn update_price(
    router: &mut App,
    config: ConfigResponse,
    price: Uint128,
    sender: &Addr,
) {
    let update_price_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.fast_oracle_addr.to_string(),
        msg: to_binary(&FastOracleExecuteMsg::Update { price }).unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(sender.clone(), [update_price_msg].to_vec())
        .unwrap();
}

fn start_next_round(
    router: &mut App,
    prediction_market_addr: &Addr,
    sender: &Addr,
) {
    router.update_block(|block| {
        block.time = block.time.plus_seconds(600);
        block.height += 1;
    });

    let start_live_round_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::CloseRound {}).unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(sender.clone(), [start_live_round_msg].to_vec())
        .unwrap();
}

fn create_prediction_market(
    router: &mut App,
    owner: &Addr,
    config: Config,
) -> Addr {
    let prediction_market_code_id =
        router.store_code(contract_price_prediction());

    router.set_block(BlockInfo {
        height: 0,
        time: Timestamp::from_seconds(0),
        chain_id: "testing".to_string(),
    });

    let mut msg = InstantiateMsg {
        config: config.clone(),
        settle_denom: SETTLE_DENOM.to_string(),
    };

    let fast_oracl_code_id = router.store_code(contract_fast_oracle());
    let external_rewards_code_id =
        router.store_code(contract_external_rewards());

    let fast_oracle_addr: Addr = router
        .instantiate_contract(
            fast_oracl_code_id,
            Addr::unchecked("fast_oracle"),
            &msg,
            &[],
            "fast_oracle",
            Some(owner.to_string()),
        )
        .unwrap();

    let external_rewards_addr: Addr = router
        .instantiate_contract(
            external_rewards_code_id,
            Addr::unchecked("external_rewards"),
            &msg,
            &[],
            "external_rewards",
            Some(owner.to_string()),
        )
        .unwrap();

    msg.config.fast_oracle_addr = fast_oracle_addr;
    msg.config.cw20_stake_external_rewards_addr = external_rewards_addr;

    router
        .instantiate_contract(
            prediction_market_code_id,
            owner.clone(),
            &msg,
            &[],
            "prediction_market",
            Some(owner.to_string()),
        )
        .unwrap()
}

#[test]
fn proper_initialization() {
    let mut router = mock_app();

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, SETTLE_DENOM);

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let default_config: Config = Config {
        next_round_seconds: Uint128::new(600u128),
        fast_oracle_addr: Addr::unchecked("fast_oracle"),
        cw20_stake_external_rewards_addr: Addr::unchecked("external_rewards"),
        minimum_bet: Uint128::new(1u128),
        burn_addr: Addr::unchecked("burn"),
        burn_fee: Uint128::new(100u128),
        staker_fee: Uint128::new(200u128),
    };

    let prediction_market_addr =
        create_prediction_market(&mut router, &owner, default_config.clone());

    assert_ne!("", prediction_market_addr);

    let config: ConfigResponse = router
        .wrap()
        .query_wasm_smart(prediction_market_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    assert_eq!(config.minimum_bet, default_config.minimum_bet);
    assert_eq!(config.staker_fee, default_config.staker_fee);
    assert_eq!(config.next_round_seconds, default_config.next_round_seconds);
}

fn create_market_and_start(
    router: &mut App,
    config: Option<Config>,
    owner: Addr,
    funds: Vec<Coin>,
) -> Addr {
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let prediction_market_addr: Addr;
    match config {
        Some(config) => {
            prediction_market_addr =
                create_prediction_market(router, &owner, config.clone());
        }
        None => {
            let default_config: Config = Config {
                next_round_seconds: Uint128::new(600u128),
                fast_oracle_addr: Addr::unchecked("fast_oracle"),
                cw20_stake_external_rewards_addr: Addr::unchecked(
                    "external_rewards",
                ),
                minimum_bet: Uint128::new(1u128),
                staker_fee: Uint128::new(200u128),
                burn_addr: Addr::unchecked("burn"),
                burn_fee: Uint128::new(100u128),
            };

            prediction_market_addr = create_prediction_market(
                router,
                &owner,
                default_config.clone(),
            );
        }
    }

    start_next_round(router, &prediction_market_addr, &owner);

    return prediction_market_addr;
}

#[test]
fn proper_prediction_market_start() {
    let mut router = mock_app();
    let owner = Addr::unchecked("owner");
    let funds = coins(2000, SETTLE_DENOM);

    let prediction_market_addr =
        create_market_and_start(router.borrow_mut(), None, owner, funds);

    let status: StatusResponse = router
        .wrap()
        .query_wasm_smart(prediction_market_addr.clone(), &QueryMsg::Status {})
        .unwrap();

    let config: ConfigResponse = router
        .wrap()
        .query_wasm_smart(prediction_market_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    assert!(status.bidding_round.is_some());
    assert!(status.live_round.is_none());

    let bidding_round = status.bidding_round.unwrap();

    assert_eq!(bidding_round.id, Uint128::zero());
    assert_eq!(bidding_round.bear_amount, Uint128::zero());
    assert_eq!(bidding_round.bull_amount, Uint128::zero());

    let time_difference: Uint128 = (bidding_round.close_time.seconds()
        - bidding_round.open_time.seconds())
    .into();

    assert_eq!(time_difference, config.next_round_seconds);
}

#[test]
fn proper_betting() {
    let mut router = mock_app();

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, SETTLE_DENOM);

    let prediction_market_addr = create_market_and_start(
        router.borrow_mut(),
        None,
        owner.clone(),
        funds,
    );

    let bet_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBear {
            round_id: Uint128::zero(),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(owner.clone(), [bet_msg].to_vec())
        .unwrap();

    let status: StatusResponse = router
        .wrap()
        .query_wasm_smart(prediction_market_addr.clone(), &QueryMsg::Status {})
        .unwrap();

    assert_eq!(
        status.bidding_round.unwrap().bear_amount,
        Uint128::new(97u128) // 3% fee
    );

    // // hmm something is removing all stored data in router when updating block time
    start_next_round(&mut router, &prediction_market_addr, &owner);

    let status: StatusResponse = router
        .wrap()
        .query_wasm_smart(prediction_market_addr.clone(), &QueryMsg::Status {})
        .unwrap();

    let live_round = status.live_round.unwrap();

    assert_eq!(live_round.id, Uint128::zero());
    assert_eq!(live_round.open_price, Uint128::new(1_000_000u128));
}

#[test]
fn proper_close_round_up_win() {
    let mut router = mock_app();

    let winner = Addr::unchecked("owner");
    let funds = coins(2000, SETTLE_DENOM);

    let loser = Addr::unchecked("loser");
    let loser_funds = coins(2000, SETTLE_DENOM);

    router.borrow_mut().init_modules(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &loser, loser_funds)
            .unwrap()
    });

    let prediction_market_addr = create_market_and_start(
        router.borrow_mut(),
        None,
        winner.clone(),
        funds,
    );

    let config: ConfigResponse = router
        .wrap()
        .query_wasm_smart(prediction_market_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    let loser_bet_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBear {
            round_id: Uint128::zero(),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(loser.clone(), [loser_bet_msg].to_vec())
        .unwrap();

    let bet_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBull {
            round_id: Uint128::zero(),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(winner.clone(), [bet_msg].to_vec())
        .unwrap();

    // Betting over, start live round from bidding round
    start_next_round(&mut router, &prediction_market_addr, &winner);

    // Update price and close live round
    update_price(
        &mut router,
        config.clone(),
        Uint128::new(1_000_001u128),
        &winner,
    );
    start_next_round(&mut router, &prediction_market_addr, &winner);

    let starting_balance = router
        .wrap()
        .query_balance(winner.clone(), SETTLE_DENOM.to_string())
        .unwrap();
    // Claim rewards for winner
    let claim_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::CollectWinnings {
            rounds: vec![Uint128::from(0u128)],
        })
        .unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(winner.clone(), [claim_msg].to_vec())
        .unwrap();

    let ending_balance = router
        .wrap()
        .query_balance(winner.clone(), SETTLE_DENOM.to_string())
        .unwrap();

    let diff = ending_balance.amount - starting_balance.amount;
    assert_eq!(diff, Uint128::new(194u128));

    let claim_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::CollectWinnings {
            rounds: vec![Uint128::from(0u128)],
        })
        .unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(loser.clone(), [claim_msg].to_vec())
        .expect_err("Should not be able to claim winnings after losing");

    let claim_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::CollectWinnings {
            rounds: vec![Uint128::from(0u128)],
        })
        .unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(winner.clone(), [claim_msg].to_vec())
        .expect_err("Should not be able to claim winnings twice");

    let burned_amount = router
        .wrap()
        .query_balance("burn", SETTLE_DENOM)
        .unwrap()
        .amount;

    assert_eq!(burned_amount, Uint128::new(2u128));

    let external_rewards_amount = router
        .wrap()
        .query_balance(
            config.cw20_stake_external_rewards_addr.clone(),
            SETTLE_DENOM,
        )
        .unwrap()
        .amount;

    assert_eq!(external_rewards_amount, Uint128::zero());

    let fund_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::FundStakers {}).unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(winner.clone(), [fund_msg].to_vec())
        .unwrap();

    let external_rewards_amount_after = router
        .wrap()
        .query_balance(
            config.cw20_stake_external_rewards_addr.clone(),
            SETTLE_DENOM,
        )
        .unwrap()
        .amount;

    assert_eq!(external_rewards_amount_after, Uint128::new(4u128));
}

#[test]
fn proper_close_round_down_win() {
    let mut router = mock_app();

    let winner = Addr::unchecked("owner");
    let funds = coins(2000, SETTLE_DENOM);

    let loser = Addr::unchecked("loser");
    let loser_funds = coins(2000, SETTLE_DENOM);

    router.borrow_mut().init_modules(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &loser, loser_funds)
            .unwrap()
    });

    let prediction_market_addr = create_market_and_start(
        router.borrow_mut(),
        None,
        winner.clone(),
        funds,
    );

    let config: ConfigResponse = router
        .wrap()
        .query_wasm_smart(prediction_market_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    let loser_bet_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBull {
            round_id: Uint128::zero(),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(loser.clone(), [loser_bet_msg].to_vec())
        .unwrap();

    let winner_bet_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBear {
            round_id: Uint128::zero(),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(winner.clone(), [winner_bet_msg].to_vec())
        .unwrap();

    // Betting over, start live round from bidding round
    start_next_round(&mut router, &prediction_market_addr, &winner);

    // Update price and close live round
    update_price(&mut router, config, Uint128::new(999_999u128), &winner);
    start_next_round(&mut router, &prediction_market_addr, &winner);

    let starting_balance = router
        .wrap()
        .query_balance(winner.clone(), SETTLE_DENOM.to_string())
        .unwrap();
    // Claim rewards for winner
    let claim_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::CollectWinnings {
            rounds: vec![Uint128::from(0u128)],
        })
        .unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(winner.clone(), [claim_msg].to_vec())
        .unwrap();

    let ending_balance = router
        .wrap()
        .query_balance(winner.clone(), SETTLE_DENOM.to_string())
        .unwrap();

    let diff = ending_balance.amount - starting_balance.amount;
    assert_eq!(diff, Uint128::new(194u128));

    let claim_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::CollectWinnings {
            rounds: vec![Uint128::from(0u128)],
        })
        .unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(loser.clone(), [claim_msg].to_vec())
        .expect_err("Should not be able to claim winnings after losing");

    let claim_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::CollectWinnings {
            rounds: vec![Uint128::from(0u128)],
        })
        .unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(winner.clone(), [claim_msg].to_vec())
        .expect_err("Should not be able to claim winnings twice");
}

#[test]
fn proper_deny_betting_on_closed_round() {
    let mut router = mock_app();

    let sender = Addr::unchecked("owner");
    let funds = coins(2000, SETTLE_DENOM);

    let prediction_market_addr = create_market_and_start(
        router.borrow_mut(),
        None,
        sender.clone(),
        funds,
    );

    start_next_round(&mut router, &prediction_market_addr, &sender);

    start_next_round(&mut router, &prediction_market_addr, &sender);

    let winner_bet_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBear {
            round_id: Uint128::zero(),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(sender.clone(), [winner_bet_msg].to_vec())
        .expect_err("Should not be able to bet on closed round");
}

#[test]
fn proper_deny_betting_on_live_round() {
    let mut router = mock_app();

    let sender = Addr::unchecked("owner");
    let funds = coins(2000, SETTLE_DENOM);

    let prediction_market_addr = create_market_and_start(
        router.borrow_mut(),
        None,
        sender.clone(),
        funds,
    );

    start_next_round(&mut router, &prediction_market_addr, &sender);

    start_next_round(&mut router, &prediction_market_addr, &sender);

    let winner_bet_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBear {
            round_id: Uint128::new(1u128),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(sender.clone(), [winner_bet_msg].to_vec())
        .expect_err("Should not be able to bet on closed round");

    let winner_bet_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBear {
            round_id: Uint128::new(3u128),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(sender.clone(), [winner_bet_msg].to_vec())
        .expect_err("Should not be able to bet on future rounds");
}

#[test]
fn proper_deny_closing_round_early() {
    let mut router = mock_app();

    let sender = Addr::unchecked("owner");
    let funds = coins(2000, SETTLE_DENOM);

    let prediction_market_addr = create_market_and_start(
        router.borrow_mut(),
        None,
        sender.clone(),
        funds,
    );

    start_next_round(&mut router, &prediction_market_addr, &sender);

    let start_status: StatusResponse = router
        .wrap()
        .query_wasm_smart(prediction_market_addr.clone(), &QueryMsg::Status {})
        .unwrap();

    router.update_block(|block| {
        block.time = block.time.plus_seconds(100);
        block.height += 1;
    });

    let start_next_round_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::CloseRound {}).unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(sender.clone(), [start_next_round_msg].to_vec())
        .unwrap();

    let end_status: StatusResponse = router
        .wrap()
        .query_wasm_smart(prediction_market_addr.clone(), &QueryMsg::Status {})
        .unwrap();

    assert_eq!(
        start_status.bidding_round.unwrap().id,
        end_status.bidding_round.unwrap().id
    );

    assert_eq!(
        start_status.live_round.unwrap().id,
        end_status.live_round.unwrap().id
    );
}

#[test]
fn proper_deny_betting_twice() {
    let mut router = mock_app();

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, SETTLE_DENOM);

    let prediction_market_addr = create_market_and_start(
        router.borrow_mut(),
        None,
        owner.clone(),
        funds,
    );

    let bet_bear_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBear {
            round_id: Uint128::zero(),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    let bet_bull_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBear {
            round_id: Uint128::zero(),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(owner.clone(), [bet_bear_msg.clone()].to_vec())
        .unwrap();

    router
        .execute_multi(owner.clone(), [bet_bull_msg.clone()].to_vec())
        .expect_err("Should not be able to bet twice");

    router
        .execute_multi(owner.clone(), [bet_bear_msg.clone()].to_vec())
        .expect_err("Should not be able to bet twice");

    start_next_round(&mut router, &prediction_market_addr, &owner);

    let bet_bear_msg_next: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBear {
            round_id: Uint128::new(1),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    let bet_bull_msg_next: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBear {
            round_id: Uint128::new(1),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(owner.clone(), [bet_bull_msg_next.clone()].to_vec())
        .unwrap();

    router
        .execute_multi(owner.clone(), [bet_bull_msg_next.clone()].to_vec())
        .expect_err("Should not be able to bet twice");

    router
        .execute_multi(owner.clone(), [bet_bear_msg_next.clone()].to_vec())
        .expect_err("Should not be able to bet twice");
}

#[test]
fn proper_haulting_and_resume_games() {
    let mut router = mock_app();

    let sender = Addr::unchecked("owner");
    let faker = Addr::unchecked("faker");
    let funds = coins(2000, SETTLE_DENOM);

    let prediction_market_addr = create_market_and_start(
        router.borrow_mut(),
        None,
        sender.clone(),
        funds,
    );

    start_next_round(&mut router, &prediction_market_addr, &sender);

    let halt_games_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::Hault {}).unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(sender.clone(), [halt_games_msg].to_vec())
        .unwrap();

    router.update_block(|block| {
        block.time = block.time.plus_seconds(900);
        block.height += 1;
    });

    let start_live_round_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::CloseRound {}).unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(sender.clone(), [start_live_round_msg].to_vec())
        .expect_err(
            "Should not be able to start a round when games are haulted",
        );

    let bet_bull_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBear {
            round_id: Uint128::new(2),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(sender.clone(), [bet_bull_msg.clone()].to_vec())
        .expect_err("Should not be able to bet when games are haulted");

    let resume_games_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::Resume {}).unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(sender.clone(), [resume_games_msg].to_vec())
        .unwrap();

    start_next_round(&mut router, &prediction_market_addr, &sender);

    // Betting resumed
    let bet_bull_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBear {
            round_id: Uint128::new(2),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(sender.clone(), [bet_bull_msg.clone()].to_vec())
        .unwrap();

    let halt_games_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::Hault {}).unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(faker.clone(), [halt_games_msg].to_vec())
        .expect_err("Should not be able to hault games if not owner");
}

#[test]
fn proper_updating_config() {
    let mut router = mock_app();

    let sender = Addr::unchecked("owner");
    let faker = Addr::unchecked("faker");
    let funds = coins(2000, SETTLE_DENOM);

    let default_config: Config = Config {
        next_round_seconds: Uint128::new(600u128),
        fast_oracle_addr: Addr::unchecked("fast_oracle"),
        cw20_stake_external_rewards_addr: Addr::unchecked("external_rewards"),
        minimum_bet: Uint128::new(1u128),
        staker_fee: Uint128::new(300u128),
        burn_addr: Addr::unchecked("burn"),
        burn_fee: Uint128::new(300u128),
    };

    let prediction_market_addr = create_market_and_start(
        router.borrow_mut(),
        Some(default_config.clone()),
        sender.clone(),
        funds,
    );

    start_next_round(&mut router, &prediction_market_addr, &sender);

    let new_minimum_bet = Uint128::new(999u128);
    let new_staker_fee = Uint128::new(500u128);
    let new_next_round_seconds = Uint128::new(900u128);
    let new_fast_oracle_addr = Addr::unchecked("new_fast_oracle");
    let new_cw20_stake_external_rewards_addr =
        Addr::unchecked("new_external_rewards");

    let update_config: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::UpdateConfig {
            config: PartialConfig {
                minimum_bet: Some(new_minimum_bet),
                fast_oracle_addr: Some(new_fast_oracle_addr.clone()),
                next_round_seconds: Some(new_next_round_seconds),
                cw20_stake_external_rewards_addr: Some(
                    new_cw20_stake_external_rewards_addr.clone(),
                ),
                staker_fee: Some(new_staker_fee),
                burn_addr: None,
                burn_fee: None,
            },
        })
        .unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(sender.clone(), [update_config.clone()].to_vec())
        .unwrap();

    let config: ConfigResponse = router
        .wrap()
        .query_wasm_smart(prediction_market_addr, &QueryMsg::Config {})
        .unwrap();

    assert!(config.minimum_bet == new_minimum_bet);
    assert!(config.staker_fee == new_staker_fee);
    assert!(config.next_round_seconds == new_next_round_seconds);
    assert!(config.fast_oracle_addr == new_fast_oracle_addr);
    assert!(
        new_cw20_stake_external_rewards_addr
            == config.cw20_stake_external_rewards_addr
    );

    router
        .execute_multi(faker.clone(), [update_config].to_vec())
        .expect_err("Should not be able to update config if not owner");
}

#[test]
fn proper_close_round_tie() {
    let mut router = mock_app();

    let bull_tie = Addr::unchecked("owner");
    let funds = coins(2000, SETTLE_DENOM);

    let bear_tie = Addr::unchecked("loser");
    let bear_funds = coins(2000, SETTLE_DENOM);

    router.borrow_mut().init_modules(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &bear_tie, bear_funds)
            .unwrap()
    });

    let prediction_market_addr = create_market_and_start(
        router.borrow_mut(),
        None,
        bull_tie.clone(),
        funds,
    );

    let bear_bet_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBull {
            round_id: Uint128::zero(),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(bear_tie.clone(), [bear_bet_msg].to_vec())
        .unwrap();

    let bull_bet_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBear {
            round_id: Uint128::zero(),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(bull_tie.clone(), [bull_bet_msg].to_vec())
        .unwrap();

    // Betting over, start live round from bidding round
    start_next_round(&mut router, &prediction_market_addr, &bull_tie);
    // Price stays the same, so it's a tie
    start_next_round(&mut router, &prediction_market_addr, &bull_tie);

    let bull_starting_balance = router
        .wrap()
        .query_balance(bull_tie.clone(), SETTLE_DENOM.to_string())
        .unwrap();
    let bear_starting_balance = router
        .wrap()
        .query_balance(bear_tie.clone(), SETTLE_DENOM.to_string())
        .unwrap();
    // Claim rewards for winner
    let claim_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::CollectWinnings {
            rounds: vec![Uint128::from(0u128)],
        })
        .unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(bull_tie.clone(), [claim_msg.clone()].to_vec())
        .unwrap();

    router
        .execute_multi(bear_tie.clone(), [claim_msg].to_vec())
        .unwrap();

    let bull_ending_balance = router
        .wrap()
        .query_balance(bull_tie.clone(), SETTLE_DENOM.to_string())
        .unwrap();
    let bear_ending_balance = router
        .wrap()
        .query_balance(bear_tie.clone(), SETTLE_DENOM.to_string())
        .unwrap();

    let diff = bull_ending_balance.amount - bull_starting_balance.amount;
    assert_eq!(diff, Uint128::new(97u128));

    let bear_diff = bear_ending_balance.amount - bear_starting_balance.amount;
    assert_eq!(bear_diff, Uint128::new(97u128));

    let claim_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::CollectWinnings {
            rounds: vec![Uint128::from(0u128)],
        })
        .unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(bear_tie.clone(), [claim_msg.clone()].to_vec())
        .expect_err("Should not be able to claim winnings twice");
    router
        .execute_multi(bull_tie.clone(), [claim_msg.clone()].to_vec())
        .expect_err("Should not be able to claim winnings twice");
}

#[test]
fn proper_return_funds_if_no_counter_party() {
    let mut router = mock_app();

    let winner = Addr::unchecked("owner");
    let funds = coins(2000, SETTLE_DENOM);

    let prediction_market_addr = create_market_and_start(
        router.borrow_mut(),
        None,
        winner.clone(),
        funds,
    );

    let config: ConfigResponse = router
        .wrap()
        .query_wasm_smart(prediction_market_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    let bet_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::BetBull {
            round_id: Uint128::zero(),
        })
        .unwrap(),
        funds: vec![Coin {
            denom: SETTLE_DENOM.to_string(),
            amount: Uint128::new(100u128),
        }],
    });

    router
        .execute_multi(winner.clone(), [bet_msg].to_vec())
        .unwrap();

    // Betting over, start live round from bidding round
    start_next_round(&mut router, &prediction_market_addr, &winner);

    // Update price and close live round
    update_price(&mut router, config, Uint128::new(999_999u128), &winner);
    start_next_round(&mut router, &prediction_market_addr, &winner);

    let starting_balance = router
        .wrap()
        .query_balance(winner.clone(), SETTLE_DENOM.to_string())
        .unwrap();
    // Claim rewards for winner
    let claim_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::CollectWinnings {
            rounds: vec![Uint128::from(0u128)],
        })
        .unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(winner.clone(), [claim_msg].to_vec())
        .unwrap();

    let ending_balance = router
        .wrap()
        .query_balance(winner.clone(), SETTLE_DENOM.to_string())
        .unwrap();

    let diff = ending_balance.amount - starting_balance.amount;
    assert_eq!(diff, Uint128::new(97u128));

    let claim_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::CollectWinnings {
            rounds: vec![Uint128::from(0u128)],
        })
        .unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(winner.clone(), [claim_msg].to_vec())
        .expect_err("Should not be able to claim winnings twice");
}

#[test]
fn proper_change_round_duration() {
    let mut router = mock_app();

    let sender = Addr::unchecked("owner");
    let funds = coins(2000, SETTLE_DENOM);

    let default_config: Config = Config {
        next_round_seconds: Uint128::new(600u128),
        fast_oracle_addr: Addr::unchecked("fast_oracle"),
        cw20_stake_external_rewards_addr: Addr::unchecked("treasury"),
        minimum_bet: Uint128::new(1u128),
        staker_fee: Uint128::new(300u128),
        burn_addr: Addr::unchecked("burn"),
        burn_fee: Uint128::new(300u128),
    };

    let prediction_market_addr = create_market_and_start(
        router.borrow_mut(),
        Some(default_config.clone()),
        sender.clone(),
        funds,
    );

    start_next_round(&mut router, &prediction_market_addr, &sender);

    let new_next_round_seconds = Uint128::new(900u128);

    let update_config: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: prediction_market_addr.to_string(),
        msg: to_binary(&ExecuteMsg::UpdateConfig {
            config: PartialConfig {
                minimum_bet: None,
                fast_oracle_addr: None,
                next_round_seconds: Some(new_next_round_seconds),
                cw20_stake_external_rewards_addr: None,
                staker_fee: None,
                burn_addr: None,
                burn_fee: None,
            },
        })
        .unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(sender.clone(), [update_config.clone()].to_vec())
        .unwrap();

    start_next_round(&mut router, &prediction_market_addr, &sender);

    let next_round_status: StatusResponse = router
        .wrap()
        .query_wasm_smart(prediction_market_addr.clone(), &QueryMsg::Status {})
        .unwrap();

    assert_eq!(
        next_round_status.live_round.unwrap().close_time,
        next_round_status.bidding_round.unwrap().open_time
    );
}
