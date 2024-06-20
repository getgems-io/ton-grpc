use itertools::Itertools;
use crate::router::{Routed, RouterError};

pub trait ToRoute {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockCriteria {
    Seqno { shard: i64, seqno: i32 },
    LogicalTime { address: [u8; 32], lt: i64 },
}

#[derive(Debug, Clone, Copy)]
pub enum Route {
    Block { chain: i32, criteria: BlockCriteria },
    Latest,
}

impl Route {
    pub fn choose<'a, S: Routed, I: IntoIterator<Item=&'a S>>(&self, from: I) -> Result<Vec<&'a S>, RouterError> {
        match self {
            Route::Block { chain, criteria } => {
                let mut known = false;
                let clients: Vec<&S> = from
                    .into_iter()
                    .filter(|s| {
                        if s.contains(chain, criteria) {
                            true
                        } else {
                            if s.contains_not_available(chain, criteria) {
                                known = true;
                            }

                            false
                        }
                    })
                    .collect();

                if clients.is_empty() {
                    if known {
                        Err(RouterError::RouteNotAvailable)
                    } else {
                        Err(RouterError::RouteUnknown)
                    }
                } else {
                    Ok(clients)
                }
            }
            Route::Latest => {
                let groups = from
                    .into_iter()
                    .filter_map(|s| s.last_seqno().map(|seqno| (s, seqno)))
                    .sorted_unstable_by_key(|(_, seqno)| -seqno)
                    .chunk_by(|(_, seqno)| *seqno);

                if let Some((_, group)) = groups.into_iter().next() {
                    return Ok(group
                        .into_iter()
                        .map(|(s, _)| s)
                        .collect());
                }

                Err(RouterError::RouteUnknown)
            }
        }
    }
}
