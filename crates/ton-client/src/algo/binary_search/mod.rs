mod account;
mod block;

use futures::TryFutureExt;
use std::future::Future;
use ton_tower::response::BlockId;

pub use account::AccountTxAvailability;
pub use block::BlockAvailability;

#[derive(Debug, thiserror::Error)]
pub enum BinarySearchError {
    #[error("empty search range")]
    EmptyRange,
    #[error("first point not found")]
    NotFound(#[source] anyhow::Error),
    #[error("failed to resolve search bounds")]
    Bounds(#[source] anyhow::Error),
}

/// Binary search for the first chain block where an async probe succeeds.
///
/// The workchain and shard are taken from the upper
/// bound ([`upper_bound`](BinarySearch::upper_bound),
/// a full [`BlockId`]), while the lower bound is a bare seqno
/// ([`lower_bound`](BinarySearch::lower_bound)).
///
/// Assumes the probe is monotonic: once it succeeds at some seqno, it succeeds for
/// every greater seqno. A non-zero [`tolerance`](BinarySearch::tolerance) allows
/// returning the last successful result when the final probe fails but the success
/// is within `tolerance` of the convergence point.
pub trait BinarySearch {
    type Item;

    fn probe(&mut self, point: BlockId) -> impl Future<Output = anyhow::Result<Self::Item>> + Send;

    fn lower_bound(&mut self) -> impl Future<Output = anyhow::Result<i32>> + Send {
        async { Ok(1) }
    }

    fn upper_bound(&mut self) -> impl Future<Output = anyhow::Result<BlockId>> + Send;

    fn starting_point(&self, lhs: i32, rhs: i32) -> i32 {
        lhs.midpoint(rhs)
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

    fn from(self, seqno: Option<i32>) -> From<Self>
    where
        Self: Sized,
    {
        From { inner: self, seqno }
    }

    fn to(self, block: Option<BlockId>) -> To<Self>
    where
        Self: Sized,
    {
        To { inner: self, block }
    }

    async fn find<'a>(mut self) -> Result<Self::Item, BinarySearchError>
    where
        Self: Sized + Send + 'a,
        Self::Item: Send,
    {
        let upper = self
            .upper_bound()
            .map_err(BinarySearchError::Bounds)
            .await?;
        let mut lhs = self
            .lower_bound()
            .map_err(BinarySearchError::Bounds)
            .await?;
        let mut rhs = upper.seqno;

        if lhs > rhs {
            return Err(BinarySearchError::EmptyRange);
        }

        let workchain = upper.workchain;
        let shard = upper.shard;

        let tolerance = self.tolerance();
        let mut cur = self.starting_point(lhs, rhs).clamp(lhs, rhs);

        let mut best: Option<(i32, Self::Item)> = None;
        let mut hops = 0;

        let last = loop {
            let point = BlockId {
                workchain,
                shard,
                seqno: cur,
            };
            let last = self.probe(point).map_ok(|value| (cur, value)).await;
            hops += 1;

            if lhs >= rhs {
                break last;
            }

            if last.is_ok() {
                rhs = cur;
            } else {
                lhs = cur + 1;
            }

            cur = lhs.midpoint(rhs);
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

    fn probe(&mut self, point: BlockId) -> impl Future<Output = anyhow::Result<Self::Item>> + Send {
        self.inner.probe(point)
    }

    fn lower_bound(&mut self) -> impl Future<Output = anyhow::Result<i32>> + Send {
        self.inner.lower_bound()
    }

    fn upper_bound(&mut self) -> impl Future<Output = anyhow::Result<BlockId>> + Send {
        self.inner.upper_bound()
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

    fn probe(&mut self, point: BlockId) -> impl Future<Output = anyhow::Result<Self::Item>> + Send {
        self.inner.probe(point)
    }

    fn lower_bound(&mut self) -> impl Future<Output = anyhow::Result<i32>> + Send {
        self.inner.lower_bound()
    }

    fn upper_bound(&mut self) -> impl Future<Output = anyhow::Result<BlockId>> + Send {
        self.inner.upper_bound()
    }

    fn starting_point(&self, lhs: i32, rhs: i32) -> i32 {
        self.inner.starting_point(lhs, rhs)
    }

    fn tolerance(&self) -> i32 {
        self.tolerance
    }
}

pub struct From<B> {
    inner: B,
    seqno: Option<i32>,
}

impl<B: BinarySearch + Send> BinarySearch for From<B> {
    type Item = B::Item;

    fn probe(&mut self, point: BlockId) -> impl Future<Output = anyhow::Result<Self::Item>> + Send {
        self.inner.probe(point)
    }

    async fn lower_bound(&mut self) -> anyhow::Result<i32> {
        match self.seqno {
            Some(seqno) => Ok(seqno),
            None => self.inner.lower_bound().await,
        }
    }

    fn upper_bound(&mut self) -> impl Future<Output = anyhow::Result<BlockId>> + Send {
        self.inner.upper_bound()
    }

    fn starting_point(&self, lhs: i32, rhs: i32) -> i32 {
        self.inner.starting_point(lhs, rhs)
    }

    fn tolerance(&self) -> i32 {
        self.inner.tolerance()
    }
}

pub struct To<B> {
    inner: B,
    block: Option<BlockId>,
}

impl<B: BinarySearch + Send> BinarySearch for To<B> {
    type Item = B::Item;

    fn probe(&mut self, point: BlockId) -> impl Future<Output = anyhow::Result<Self::Item>> + Send {
        self.inner.probe(point)
    }

    fn lower_bound(&mut self) -> impl Future<Output = anyhow::Result<i32>> + Send {
        self.inner.lower_bound()
    }

    async fn upper_bound(&mut self) -> anyhow::Result<BlockId> {
        match &self.block {
            Some(block) => Ok(block.clone()),
            None => self.inner.upper_bound().await,
        }
    }

    fn starting_point(&self, lhs: i32, rhs: i32) -> i32 {
        self.inner.starting_point(lhs, rhs)
    }

    fn tolerance(&self) -> i32 {
        self.inner.tolerance()
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
            .to(Some(masterchain(1000)))
            .find()
            .await;

        assert_eq!(result.unwrap(), 700);
    }

    #[tokio::test]
    async fn should_find_lower_bound_when_probe_always_succeeds() {
        let result = from_fn(|point| async move { anyhow::Ok(point) })
            .to(Some(masterchain(1000)))
            .find()
            .await;

        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn should_return_error_when_probe_never_succeeds() {
        let result: Result<i32, _> = from_fn(|_| async { Err(anyhow!("unavailable")) })
            .to(Some(masterchain(1000)))
            .find()
            .await;

        let err = result.unwrap_err();
        assert!(matches!(err, BinarySearchError::NotFound(_)));
        assert_eq!(err.source().unwrap().to_string(), "unavailable");
    }

    #[tokio::test]
    async fn should_fail_on_empty_range() {
        let result = from_fn(|point| async move { anyhow::Ok(point) })
            .from(Some(1000))
            .to(Some(masterchain(1)))
            .find()
            .await;

        assert!(matches!(result.unwrap_err(), BinarySearchError::EmptyRange));
    }

    #[tokio::test]
    async fn should_clamp_starting_point_into_bounds() {
        let result = from_fn(|point| async move { available_from(700, point) })
            .starting_at(-200000)
            .to(Some(masterchain(1000)))
            .find()
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
        .to(Some(masterchain(1000)))
        .find()
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
        .to(Some(masterchain(1000)))
        .find()
        .await;

        assert_eq!(result.unwrap(), 500);
    }

    fn masterchain(seqno: i32) -> BlockId {
        BlockId {
            workchain: -1,
            shard: i64::MIN,
            seqno,
        }
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

        fn probe(&mut self, point: BlockId) -> impl Future<Output = anyhow::Result<T>> + Send {
            (self.0)(point.seqno)
        }

        fn upper_bound(&mut self) -> impl Future<Output = anyhow::Result<BlockId>> + Send {
            async { Err(anyhow!("upper bound must be provided via .to(..)")) }
        }
    }
}
