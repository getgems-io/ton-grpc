mod account;
mod block;

use core::range::RangeInclusive;
use std::future::Future;

pub use account::AccountTxAvailability;
pub use block::BlockAvailability;

#[derive(Debug, thiserror::Error)]
pub enum BinarySearchError {
    #[error("empty search range")]
    EmptyRange,
    #[error("first point not found")]
    NotFound(#[source] anyhow::Error),
}

/// Binary search for the first point in a range where an async probe succeeds.
///
/// Assumes the probe is monotonic: once it succeeds at some point, it succeeds for
/// every greater point. A non-zero [`tolerance`](BinarySearch::tolerance) allows
/// returning the last successful result when the final probe fails but the success
/// is within `tolerance` of the convergence point.
#[async_trait::async_trait]
pub trait BinarySearch {
    type Item;

    fn probe(&mut self, point: i32) -> impl Future<Output = anyhow::Result<Self::Item>> + Send;

    fn starting_point(&self, lhs: i32, rhs: i32) -> i32 {
        (lhs + rhs) / 2
    }

    fn tolerance(&self) -> i32 {
        0
    }

    fn starting_at(self, point: i32) -> StartingAt<Self>
    where
        Self: Sized,
    {
        StartingAt { inner: self, point }
    }

    fn with_tolerance(self, tolerance: i32) -> WithTolerance<Self>
    where
        Self: Sized,
    {
        WithTolerance {
            inner: self,
            tolerance,
        }
    }

    async fn find_first<'a>(
        mut self,
        range: impl Into<RangeInclusive<i32>> + Send,
    ) -> Result<Self::Item, BinarySearchError>
    where
        Self: Sized + Send + 'a,
        Self::Item: Send,
    {
        let RangeInclusive {
            start: mut lhs,
            last: mut rhs,
        } = range.into();

        if lhs > rhs {
            return Err(BinarySearchError::EmptyRange);
        }

        let tolerance = self.tolerance();
        let mut cur = self.starting_point(lhs, rhs).clamp(lhs, rhs);

        let mut best: Option<(i32, Self::Item)> = None;
        let mut hops = 0;

        let last = loop {
            let last = self.probe(cur).await.map(|value| (cur, value));
            hops += 1;

            if lhs >= rhs {
                break last;
            }

            if last.is_ok() {
                rhs = cur;
            } else {
                lhs = cur + 1;
            }

            cur = (lhs + rhs) / 2;
            if cur == 0 {
                break last;
            }

            if let Ok(found) = last {
                best = Some(found);
            }
        };

        match last {
            Ok((point, value)) => {
                tracing::trace!(hops, point, "found first point");

                Ok(value)
            }
            Err(e) => match best {
                Some((point, value)) if point - cur <= tolerance => {
                    tracing::trace!(hops, point, "found first point");

                    Ok(value)
                }
                _ => Err(BinarySearchError::NotFound(e)),
            },
        }
    }
}

pub struct StartingAt<B> {
    inner: B,
    point: i32,
}

impl<B: BinarySearch> BinarySearch for StartingAt<B> {
    type Item = B::Item;

    fn probe(&mut self, point: i32) -> impl Future<Output = anyhow::Result<Self::Item>> + Send {
        self.inner.probe(point)
    }

    fn starting_point(&self, _lhs: i32, _rhs: i32) -> i32 {
        self.point
    }

    fn tolerance(&self) -> i32 {
        self.inner.tolerance()
    }
}

pub struct WithTolerance<B> {
    inner: B,
    tolerance: i32,
}

impl<B: BinarySearch> BinarySearch for WithTolerance<B> {
    type Item = B::Item;

    fn probe(&mut self, point: i32) -> impl Future<Output = anyhow::Result<Self::Item>> + Send {
        self.inner.probe(point)
    }

    fn starting_point(&self, lhs: i32, rhs: i32) -> i32 {
        self.inner.starting_point(lhs, rhs)
    }

    fn tolerance(&self) -> i32 {
        self.tolerance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use std::error::Error;

    #[tokio::test]
    async fn should_find_first_successful_point() {
        let result = from_fn(|point| async move { available_from(700, point) })
            .find_first(1..=1000)
            .await;

        assert_eq!(result.unwrap(), 700);
    }

    #[tokio::test]
    async fn should_find_lower_bound_when_probe_always_succeeds() {
        let result = from_fn(|point| async move { anyhow::Ok(point) })
            .find_first(1..=1000)
            .await;

        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn should_return_error_when_probe_never_succeeds() {
        let result: Result<i32, _> = from_fn(|_| async { Err(anyhow!("unavailable")) })
            .find_first(1..=1000)
            .await;

        let err = result.unwrap_err();
        assert!(matches!(err, BinarySearchError::NotFound(_)));
        assert_eq!(err.source().unwrap().to_string(), "unavailable");
    }

    #[tokio::test]
    async fn should_fail_on_empty_range() {
        let result = from_fn(|point| async move { anyhow::Ok(point) })
            .find_first(1000..=1)
            .await;

        assert!(matches!(result.unwrap_err(), BinarySearchError::EmptyRange));
    }

    #[tokio::test]
    async fn should_clamp_starting_point_into_bounds() {
        let result = from_fn(|point| async move { available_from(700, point) })
            .starting_at(-200000)
            .find_first(1..=1000)
            .await;

        assert_eq!(result.unwrap(), 700);
    }

    #[tokio::test]
    async fn should_return_last_success_within_tolerance() {
        let mut calls = 0;

        let result = from_fn(|point| {
            calls += 1;
            let ok = calls == 1;
            async move {
                if ok {
                    Ok(point)
                } else {
                    Err(anyhow!("unavailable"))
                }
            }
        })
        .with_tolerance(4)
        .find_first(1..=1000)
        .await;

        assert_eq!(result.unwrap(), 500);
    }

    #[tokio::test]
    async fn should_return_last_success_when_reprobe_at_same_point_fails() {
        let mut calls = 0;

        let result = from_fn(|point| {
            calls += 1;
            let ok = calls == 1;
            async move {
                if ok {
                    Ok(point)
                } else {
                    Err(anyhow!("unavailable"))
                }
            }
        })
        .find_first(1..=1000)
        .await;

        assert_eq!(result.unwrap(), 500);
    }

    fn available_from(first: i32, point: i32) -> anyhow::Result<i32> {
        if point >= first {
            Ok(point)
        } else {
            Err(anyhow!("unavailable"))
        }
    }

    fn from_fn<F, Fut, T>(f: F) -> FromFn<F>
    where
        F: FnMut(i32) -> Fut,
        Fut: Future<Output = anyhow::Result<T>> + Send,
    {
        FromFn(f)
    }

    struct FromFn<F>(F);

    impl<F, Fut, T> BinarySearch for FromFn<F>
    where
        F: FnMut(i32) -> Fut,
        Fut: Future<Output = anyhow::Result<T>> + Send,
    {
        type Item = T;

        fn probe(&mut self, point: i32) -> impl Future<Output = anyhow::Result<T>> + Send {
            (self.0)(point)
        }
    }
}
