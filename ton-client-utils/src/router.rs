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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct MyRouted {
        contains: bool,
        contains_not_available: bool,
        last_seqno: Option<i32>
    }

    impl Routed for MyRouted {
        fn contains(&self, chain: &i32, criteria: &BlockCriteria) -> bool { self.contains }
        fn contains_not_available(&self, chain: &i32, criteria: &BlockCriteria) -> bool { self.contains_not_available }
        fn last_seqno(&self) -> Option<i32> { self.last_seqno }
    }

    #[test]
    fn route_latest_empty() {
        let route = Route::Latest;
        let from: HashMap<String, MyRouted> = HashMap::new();

        let result = route.choose(&from).unwrap_err();

        assert!(matches!(result, RouterError::RouteUnknown));
    }

    #[test]
    fn route_block_available() {
        let route = Route::Block { chain: 1, criteria: BlockCriteria::LogicalTime(100)  };
        let mut from: HashMap<String, MyRouted> = HashMap::new();
        let routed = MyRouted {
            contains: true,
            contains_not_available: true,
            last_seqno: None,
        };
        from.insert("1".to_string(), routed.clone());

        let result = route.choose(&from).unwrap();

        assert_eq!(result, vec![routed]);
    }
}
