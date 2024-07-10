pub mod balance;
pub mod route;
pub mod shards;

use crate::router::route::BlockCriteria;

pub trait Routed {
    fn contains(&self, chain: &i32, criteria: &BlockCriteria) -> bool;
    fn contains_not_available(&self, chain: &i32, criteria: &BlockCriteria) -> bool;
    fn last_seqno(&self) -> Option<i32>;
}
