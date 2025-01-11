use cosmwasm_std::{BlockInfo, Timestamp};

pub fn is_expired(current: &BlockInfo, date: &Timestamp, duration: u64) -> bool {
    current.time > date.plus_seconds(duration)
}
