pub mod client;
mod discover;
mod registry;
mod request;
mod router;
mod shard_bounds;
mod shard_prefix;

use itertools::Itertools;
use ton_tower::response::BlockIdExt;

pub use client::RoutedClient;
pub use router::Router;

pub type Seqno = i32;
pub type ChainId = i32;
pub type ShardId = (i32, i64);

pub fn shard_id_of(block_id: &BlockIdExt) -> ShardId {
    (block_id.workchain, block_id.shard)
}

pub trait ToRoute {
    fn to_route(&self) -> Route;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Block { chain: i32, criteria: BlockCriteria },
    Latest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockCriteria {
    Seqno { shard: i64, seqno: i32 },
    LogicalTime { address: [u8; 32], lt: i64 },
}

pub trait Routed {
    fn contains(&self, chain: &i32, criteria: &BlockCriteria) -> bool;
    fn contains_not_available(&self, chain: &i32, criteria: &BlockCriteria) -> bool;
    fn last_seqno(&self) -> Option<i32>;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("route is not available at this moment")]
    RouteNotAvailable,
    #[error("route is unknown")]
    RouteUnknown,
}

pub fn choose<'a, S, I>(route: &Route, from: I) -> Result<Vec<S>, Error>
where
    S: Routed + Clone + 'a,
    I: IntoIterator<Item = &'a S>,
{
    match route {
        Route::Block { chain, criteria } => {
            let mut known = false;
            let clients: Vec<_> = from
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
                .cloned()
                .collect();

            if clients.is_empty() {
                if known {
                    Err(Error::RouteNotAvailable)
                } else {
                    Err(Error::RouteUnknown)
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
                return Ok(group.into_iter().map(|(s, _)| s).cloned().collect());
            }

            Err(Error::RouteUnknown)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct StubRouted {
        contains: bool,
        contains_not_available: bool,
        last_seqno: Option<i32>,
    }

    impl Routed for StubRouted {
        fn contains(&self, _: &i32, _: &BlockCriteria) -> bool {
            self.contains
        }
        fn contains_not_available(&self, _: &i32, _: &BlockCriteria) -> bool {
            self.contains_not_available
        }
        fn last_seqno(&self) -> Option<i32> {
            self.last_seqno
        }
    }

    #[test]
    fn given_routed_is_empty() {
        let route = Route::Latest;
        let from: Vec<StubRouted> = Vec::new();

        let result = choose(&route, &from).unwrap_err();

        assert!(matches!(result, Error::RouteUnknown));
    }

    #[test]
    fn given_block_available() {
        let route = Route::Block {
            chain: 1,
            criteria: BlockCriteria::LogicalTime {
                address: [0; 32],
                lt: 100,
            },
        };
        let routed = StubRouted {
            contains: true,
            contains_not_available: true,
            last_seqno: None,
        };
        let from = vec![routed.clone()];

        let result = choose(&route, &from).unwrap();

        assert_eq!(result, vec![routed]);
    }

    #[test]
    fn given_block_unknown() {
        let route = Route::Block {
            chain: 1,
            criteria: BlockCriteria::LogicalTime {
                address: [0; 32],
                lt: 100,
            },
        };
        let from = vec![StubRouted {
            contains: false,
            contains_not_available: false,
            last_seqno: None,
        }];

        let result = choose(&route, &from).unwrap_err();

        assert!(matches!(result, Error::RouteUnknown));
    }

    #[test]
    fn given_block_not_available() {
        let route = Route::Block {
            chain: 1,
            criteria: BlockCriteria::LogicalTime {
                address: [0; 32],
                lt: 100,
            },
        };
        let from = vec![
            StubRouted {
                contains: false,
                contains_not_available: true,
                last_seqno: None,
            },
            StubRouted {
                contains: false,
                contains_not_available: false,
                last_seqno: None,
            },
        ];

        let result = choose(&route, &from).unwrap_err();

        assert!(matches!(result, Error::RouteNotAvailable));
    }

    #[test]
    fn route_latest_picks_max_seqno() {
        let route = Route::Latest;
        let from = vec![
            StubRouted {
                contains: false,
                contains_not_available: true,
                last_seqno: Some(70),
            },
            StubRouted {
                contains: false,
                contains_not_available: true,
                last_seqno: Some(100),
            },
            StubRouted {
                contains: false,
                contains_not_available: true,
                last_seqno: Some(50),
            },
        ];

        let result = choose(&route, &from).unwrap();

        assert_eq!(
            result,
            vec![StubRouted {
                contains: false,
                contains_not_available: true,
                last_seqno: Some(100),
            }]
        );
    }
}
