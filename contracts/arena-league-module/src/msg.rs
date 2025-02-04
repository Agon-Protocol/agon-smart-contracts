use crate::state::{LeagueExt, Match, MatchResult, PointAdjustment};
use arena_interface::{
    competition::{
        msg::{ExecuteBase, InstantiateBase, MigrateBase, QueryBase, ToCompetitionExt},
        state::{Competition, CompetitionResponse},
    },
    group,
};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Empty, Int128, StdError, StdResult, Uint128, Uint64};

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteExt {
    /// Callable only by the module to instantiate the rounds when creating a competition
    InstantiateRounds {},
    ProcessMatch {
        league_id: Uint128,
        round_number: Uint64,
        match_results: Vec<MatchResultMsg>,
    },
    UpdateDistribution {
        league_id: Uint128,
        distribution: Vec<Decimal>,
    },
    AddPointAdjustments {
        league_id: Uint128,
        addr: String,
        point_adjustments: Vec<PointAdjustment>,
    },
}

impl From<ExecuteExt> for ExecuteMsg {
    fn from(msg: ExecuteExt) -> Self {
        ExecuteMsg::Extension { msg }
    }
}

#[cw_serde]
pub struct MatchResultMsg {
    pub match_number: Uint128,
    pub match_result: MatchResult,
}

#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum LeagueQueryExt {
    #[returns(Vec<MemberPoints>)]
    Leaderboard {
        league_id: Uint128,
        round: Option<Uint64>,
    },
    #[returns(RoundResponse)]
    Round {
        league_id: Uint128,
        round_number: Uint64,
    },
    #[returns(Vec<PointAdjustmentResponse>)]
    PointAdjustments {
        league_id: Uint128,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(DumpStateResponse)]
    DumpState {
        league_id: Uint128,
        round_number: Uint64,
    },
}

impl From<LeagueQueryExt> for QueryMsg {
    fn from(msg: LeagueQueryExt) -> Self {
        QueryMsg::QueryExtension { msg }
    }
}

#[cw_serde]
#[serde(untagged)]
pub enum MigrateMsg {
    Base(MigrateBase),
}
/// This is used to completely generate schema types
/// QueryExt response types are hidden by the QueryBase mapping to Binary output
#[cw_serde]
pub struct SudoMsg {
    pub member_points: MemberPoints,
    pub round_response: RoundResponse,
}

#[cw_serde]
pub struct LeagueInstantiateExt {
    pub match_win_points: Uint64,
    pub match_draw_points: Uint64,
    pub match_lose_points: Uint64,
    pub distribution: Vec<Decimal>,
}

impl ToCompetitionExt<LeagueExt> for LeagueInstantiateExt {
    fn to_competition_ext(
        &self,
        deps: cosmwasm_std::Deps,
        group_contract: &Addr,
    ) -> StdResult<LeagueExt> {
        let team_count: Uint64 = deps.querier.query_wasm_smart(
            group_contract.to_string(),
            &group::QueryMsg::MembersCount {},
        )?;
        if team_count < Uint64::new(2) {
            return Err(StdError::GenericErr {
                msg: "At least 2 teams should be provided".to_string(),
            });
        }
        if Uint64::new(self.distribution.len() as u64) > team_count {
            return Err(StdError::GenericErr {
                msg: "Cannot have a distribution size bigger than the teams size".to_string(),
            });
        }
        if self.distribution.iter().sum::<Decimal>() != Decimal::one() {
            return Err(StdError::generic_err("The distribution must sum up to 1"));
        }

        let matches = team_count * (team_count - Uint64::one()) / Uint64::new(2);
        let rounds = if team_count.u64() % 2 == 0 {
            team_count - Uint64::one()
        } else {
            team_count
        };

        Ok(LeagueExt {
            match_win_points: self.match_win_points,
            match_draw_points: self.match_draw_points,
            match_lose_points: self.match_lose_points,
            teams: team_count,
            rounds,
            matches: matches.into(),
            processed_matches: Uint128::zero(),
            distribution: self.distribution.clone(),
        })
    }
}

#[cw_serde]
pub struct MemberPoints {
    pub member: Addr,
    pub points: Int128,
    pub matches_played: Uint64,
}

#[cw_serde]
pub struct RoundResponse {
    pub round_number: Uint64,
    pub matches: Vec<Match>,
}

#[cw_serde]
pub struct PointAdjustmentResponse {
    pub addr: Addr,
    pub point_adjustments: Vec<PointAdjustment>,
}

#[cw_serde]
pub struct DumpStateResponse {
    pub leaderboard: Vec<MemberPoints>,
    pub round: RoundResponse,
    pub point_adjustments: Vec<PointAdjustmentResponse>,
}

pub type InstantiateMsg = InstantiateBase<Empty>;
pub type ExecuteMsg = ExecuteBase<ExecuteExt, LeagueInstantiateExt>;
pub type QueryMsg = QueryBase<Empty, LeagueQueryExt, LeagueExt>;
pub type League = Competition<LeagueExt>;
pub type LeagueResponse = CompetitionResponse<LeagueExt>;
