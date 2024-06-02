use std::collections::HashMap;
use itertools::Itertools;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockCriteria {
    Seqno { shard: i64, seqno: i32 },
    LogicalTime(i64)
}

#[derive(Debug, Clone, Copy)]
pub enum Route {
    Block { chain: i32, criteria: BlockCriteria },
    Latest
}

#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    #[error("route is not available at this moment")]
    RouteNotAvailable,
    #[error("route is unknown")]
    RouteUnknown
}

pub trait Routed {
    fn contains(&self, chain: &i32, criteria: &BlockCriteria) -> bool;
    fn contains_not_available(&self, chain: &i32, criteria: &BlockCriteria) -> bool;
    fn last_seqno(&self) -> Option<i32>;
}

impl Route {
    pub fn choose<S: Routed + Clone>(&self, from: &HashMap<String, S>) -> Result<Vec<S>, RouterError> {
        match self {
            Route::Block { chain, criteria } => {
                let clients: Vec<S> = from
                    .values()
                    .filter(|s| s.contains(chain, criteria))
                    .map(|s| s.clone())
                    .collect();

                if clients.is_empty() {
                    if from.values().any(|s| s.contains_not_available(chain, criteria)) {
                        Err(RouterError::RouteNotAvailable)
                    } else {
                        Err(RouterError::RouteUnknown)
                    }
                } else {
                    Ok(clients)
                }
            },
            Route::Latest => {
                let groups = from
                    .values()
                    .filter_map(|s| s.last_seqno().map(|seqno| (s, seqno)))
                    .sorted_unstable_by_key(|(_, seqno)| -seqno)
                    .chunk_by(|(_, seqno)| *seqno);

                if let Some((_, group)) = (&groups).into_iter().next() {
                    return Ok(group
                        .into_iter()
                        .map(|(s, _)| s.clone())
                        .collect());
                }

                Err(RouterError::RouteUnknown)
            }
        }
    }
}
