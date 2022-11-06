use std::borrow::BorrowMut;

use cosmwasm_std::{to_binary, Addr, CosmosMsg, Empty, Uint128, WasmMsg};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use forecast_deliverdao::fast_oracle::msg::InstantiateMsg;

fn mock_app() -> App {
    App::default()
}

pub fn contract_fast_oracle() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    Box::new(contract)
}

fn create_fast_oracle(router: &mut App, owner: &Addr) -> Addr {
    let fast_oracl_code_id = router.store_code(contract_fast_oracle());

    let msg = InstantiateMsg {};

    router
        .instantiate_contract(
            fast_oracl_code_id,
            owner.clone(),
            &msg,
            &[],
            "fast_oracle",
            Some(owner.to_string()),
        )
        .unwrap()
}

#[test]
fn proper_initialization() {
    let mut router = mock_app();
    let owner = Addr::unchecked("owner");

    let oracle_addr = create_fast_oracle(router.borrow_mut(), &owner);

    let price: Uint128 = router
        .wrap()
        .query_wasm_smart(
            &oracle_addr,
            &forecast_deliverdao::fast_oracle::msg::QueryMsg::Price {},
        )
        .unwrap();

    let info = router.wrap().query_wasm_contract_info(oracle_addr).unwrap();

    assert_eq!(info.admin, Some(owner.to_string()));
    assert_eq!(price, Uint128::zero());
}

#[test]
fn proper_update_price() {
    let mut router = mock_app();
    let owner = Addr::unchecked("owner");
    let oracle_addr = create_fast_oracle(router.borrow_mut(), &owner);
    let new_price = Uint128::from(100u128);

    let update_price_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: oracle_addr.to_string(),
        msg: to_binary(
            &forecast_deliverdao::fast_oracle::msg::ExecuteMsg::Update {
                price: new_price.clone(),
            },
        )
        .unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(owner, [update_price_msg].to_vec())
        .unwrap();

    let price: Uint128 = router
        .wrap()
        .query_wasm_smart(
            &oracle_addr,
            &forecast_deliverdao::fast_oracle::msg::QueryMsg::Price {},
        )
        .unwrap();

    assert_eq!(price, new_price);
}

#[test]
fn proper_deny_random_user_update() {
    let mut router = mock_app();
    let owner = Addr::unchecked("owner");
    let faker = Addr::unchecked("faker");

    let oracle_addr = create_fast_oracle(router.borrow_mut(), &owner);
    let new_price = Uint128::from(100u128);

    let update_price_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: oracle_addr.to_string(),
        msg: to_binary(
            &forecast_deliverdao::fast_oracle::msg::ExecuteMsg::Update {
                price: new_price.clone(),
            },
        )
        .unwrap(),
        funds: vec![],
    });

    router
        .execute_multi(faker, [update_price_msg].to_vec())
        .expect_err("Faker should fail");
}
