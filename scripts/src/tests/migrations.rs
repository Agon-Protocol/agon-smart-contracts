use arena_competition_enrollment::msg::QueryMsgFns as _;
use arena_interface::competition::msg::MigrateBase;
use cw_orch::{anyhow, prelude::*};
use cw_orch_clone_testing::CloneTesting;
use networks::PION_1;

use crate::Arena;

const ARENA_DAO: &str = "neutron1ehkcl0n6s2jtdw75xsvfxm304mz4hs5z7jt6wn5mk0celpj0epqql4ulxk";

#[test]
#[ignore = "RPC blocks"]
fn test_migration_v2_2_v2_3() -> anyhow::Result<()> {
    let app = CloneTesting::new(PION_1)?;
    let mut arena = Arena::load_from(app.clone())?;
    arena.set_contracts_state(None);

    let arena_dao = Addr::unchecked(ARENA_DAO);

    arena.upload(false)?;

    let escrow_id = arena.arena_escrow.code_id()?;

    arena.arena_wager_module.call_as(&arena_dao).migrate(
        &arena_wager_module::msg::MigrateMsg::Base(MigrateBase::FromV2_2 { escrow_id }),
        arena.arena_wager_module.code_id()?,
    )?;
    arena.arena_league_module.call_as(&arena_dao).migrate(
        &arena_league_module::msg::MigrateMsg::Base(MigrateBase::FromV2_2 { escrow_id }),
        arena.arena_league_module.code_id()?,
    )?;
    arena.arena_tournament_module.call_as(&arena_dao).migrate(
        &arena_tournament_module::msg::MigrateMsg::Base(MigrateBase::FromV2_2 { escrow_id }),
        arena.arena_tournament_module.code_id()?,
    )?;
    arena
        .arena_competition_enrollment
        .call_as(&arena_dao)
        .migrate(
            &arena_competition_enrollment::msg::MigrateMsg::FromV2_2 { escrow_id },
            arena.arena_competition_enrollment.code_id()?,
        )?;

    let enrollments = arena
        .arena_competition_enrollment
        .enrollments(None, None, None)?;
    dbg!(enrollments);

    Ok(())
}
