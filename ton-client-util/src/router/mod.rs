pub mod shards;
pub mod route;

use itertools::Itertools;
use crate::router::route::BlockCriteria;

#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    #[error("route is not available at this moment")]
    RouteNotAvailable,
    #[error("route is unknown")]
    RouteUnknown,
}

pub trait Routed {
    fn contains(&self, chain: &i32, criteria: &BlockCriteria) -> bool;
    fn contains_not_available(&self, chain: &i32, criteria: &BlockCriteria) -> bool;
    fn last_seqno(&self) -> Option<i32>;
}

#[cfg(test)]
mod tests {
    use crate::router::route::Route;
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct MyRouted {
        contains: bool,
        contains_not_available: bool,
        last_seqno: Option<i32>,
    }

    impl Routed for MyRouted {
        fn contains(&self, _: &i32, _: &BlockCriteria) -> bool { self.contains }
        fn contains_not_available(&self, _: &i32, _: &BlockCriteria) -> bool { self.contains_not_available }
        fn last_seqno(&self) -> Option<i32> { self.last_seqno }
    }

    #[test]
    fn given_routed_is_empty() {
        let route = Route::Latest;
        let from: Vec<MyRouted> = Vec::new();

        let result = route.choose(&from).unwrap_err();

        assert!(matches!(result, RouterError::RouteUnknown));
    }

    #[test]
    fn given_block_available() {
        let route = Route::Block { chain: 1, criteria: BlockCriteria::LogicalTime { address: [0; 32], lt: 100 } };
        let routed = MyRouted {
            contains: true,
            contains_not_available: true,
            last_seqno: None,
        };
        let from = vec![routed.clone()];

        let result = route.choose(&from).unwrap();

        assert_eq!(result, vec![&routed]);
    }

    #[test]
    fn given_block_unknown() {
        let route = Route::Block { chain: 1, criteria: BlockCriteria::LogicalTime { address: [0; 32], lt: 100 } };
        let from = vec![MyRouted {
            contains: false,
            contains_not_available: false,
            last_seqno: None,
        }];

        let result = route.choose(&from).unwrap_err();

        assert!(matches!(result, RouterError::RouteUnknown));
    }

    #[test]
    fn given_block_not_available() {
        let route = Route::Block { chain: 1, criteria: BlockCriteria::LogicalTime { address: [0; 32], lt: 100 } };
        let from = vec![MyRouted {
            contains: false,
            contains_not_available: true,
            last_seqno: None,
        }, MyRouted {
            contains: false,
            contains_not_available: false,
            last_seqno: None,
        }];

        let result = route.choose(&from).unwrap_err();

        assert!(matches!(result, RouterError::RouteNotAvailable));
    }

    #[test]
    fn route_latest_to_max_seqno() {
        let route = Route::Latest;
        let from = vec![MyRouted {
            contains: false,
            contains_not_available: true,
            last_seqno: Some(70),
        }, MyRouted {
            contains: false,
            contains_not_available: true,
            last_seqno: Some(100),
        }, MyRouted {
            contains: false,
            contains_not_available: true,
            last_seqno: Some(50),
        }];

        let result = route.choose(&from).unwrap();

        assert_eq!(result, vec![&MyRouted {
            contains: false,
            contains_not_available: true,
            last_seqno: Some(100),
        }]);
    }
}
