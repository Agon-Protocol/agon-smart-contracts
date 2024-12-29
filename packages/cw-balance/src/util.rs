use cosmwasm_std::Deps;

pub fn is_contract(deps: Deps, addr: impl Into<String>) -> bool {
    deps.querier.query_wasm_contract_info(addr).is_ok()
}
