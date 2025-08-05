use crate::router::{BlockAvailability, Routed};
use itertools::Itertools;

pub trait ToRoute {
    fn to_route(&self) -> Route;
}

#[derive(Debug, Clone, Copy)]
pub enum Route {
    Block { chain: i32, criteria: BlockCriteria },
    Latest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockCriteria {
    Seqno { shard: i64, seqno: i32 },
    LogicalTime { address: [u8; 32], lt: i64 },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("route is not available at this moment")]
    RouteNotAvailable,
    #[error("route is unknown")]
    RouteUnknown,
}

impl Route {
    pub fn choose<'a, S, I>(&self, from: I) -> Result<Vec<S>, Error>
    where
        S: Routed + Clone + 'a,
        I: IntoIterator<Item = &'a S>,
    {
        match self {
            Route::Block { chain, criteria } => {
                let mut known = false;
                let clients: Vec<_> = from
                    .into_iter()
                    .filter(|s| match s.available(chain, criteria) {
                        BlockAvailability::Available => true,
                        BlockAvailability::NotAvailable => {
                            known = true;
                            false
                        }
                        BlockAvailability::NotPresent | BlockAvailability::Unknown => false,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::route::Route;
    use crate::router::BlockAvailability;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct MyRouted {
        contains: bool,
        contains_not_available: bool,
        last_seqno: Option<i32>,
    }

    impl Routed for MyRouted {
        fn available(&self, _: &i32, _: &BlockCriteria) -> BlockAvailability {
            if self.contains {
                BlockAvailability::Available
            } else if self.contains_not_available {
                BlockAvailability::NotAvailable
            } else {
                BlockAvailability::NotPresent
            }
        }

        fn last_seqno(&self) -> Option<i32> {
            self.last_seqno
        }
    }

    #[test]
    fn given_routed_is_empty() {
        let route = Route::Latest;
        let from: Vec<MyRouted> = Vec::new();

        let result = route.choose(&from).unwrap_err();

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
        let routed = MyRouted {
            contains: true,
            contains_not_available: true,
            last_seqno: None,
        };
        let from = vec![routed.clone()];

        let result = route.choose(&from).unwrap();

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
        let from = vec![MyRouted {
            contains: false,
            contains_not_available: false,
            last_seqno: None,
        }];

        let result = route.choose(&from).unwrap_err();

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
            MyRouted {
                contains: false,
                contains_not_available: true,
                last_seqno: None,
            },
            MyRouted {
                contains: false,
                contains_not_available: false,
                last_seqno: None,
            },
        ];

        let result = route.choose(&from).unwrap_err();

        assert!(matches!(result, Error::RouteNotAvailable));
    }

    #[test]
    fn route_latest_to_max_seqno() {
        let route = Route::Latest;
        let from = vec![
            MyRouted {
                contains: false,
                contains_not_available: true,
                last_seqno: Some(70),
            },
            MyRouted {
                contains: false,
                contains_not_available: true,
                last_seqno: Some(100),
            },
            MyRouted {
                contains: false,
                contains_not_available: true,
                last_seqno: Some(50),
            },
        ];

        let result = route.choose(&from).unwrap();

        assert_eq!(
            result,
            vec![MyRouted {
                contains: false,
                contains_not_available: true,
                last_seqno: Some(100),
            }]
        );
    }
}
