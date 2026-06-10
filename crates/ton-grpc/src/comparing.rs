use anyhow::Error;
use futures::FutureExt;
use futures::future::BoxFuture;
use similar::TextDiff;
use std::any::type_name;
use std::fmt::Debug;
use std::task::{Context, Poll};
use ton_tower::Request;
use tower::Service;
use tower::ServiceExt;

/// Dispatches every request to both `primary` and `secondary`, compares their
/// responses, logs mismatches, and returns the response from `primary`.
#[derive(Clone, Debug)]
pub struct ComparingAdapter<P, S> {
    primary: P,
    secondary: S,
}

impl<P, S> ComparingAdapter<P, S> {
    pub fn new(primary: P, secondary: S) -> Self {
        Self { primary, secondary }
    }
}

impl<R, P, S> Service<R> for ComparingAdapter<P, S>
where
    R: Request + Clone + Debug + Send + 'static,
    R::Response: PartialEq + Debug + Send + 'static,
    P: Service<R, Response = R::Response, Error = Error>,
    P::Future: Send + 'static,
    S: Service<R, Response = R::Response, Error = Error>,
    S::Future: Send + 'static,
{
    type Response = R::Response;
    type Error = Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match <P as Service<R>>::poll_ready(&mut self.primary, cx) {
            Poll::Ready(Ok(())) => <S as Service<R>>::poll_ready(&mut self.secondary, cx),
            other => other,
        }
    }

    fn call(&mut self, req: R) -> Self::Future {
        let req_for_secondary = req.clone();
        let req_for_log = req.clone();

        let primary_fut = self.primary.call(req);
        let secondary_fut = self.secondary.call(req_for_secondary);

        async move {
            let (primary_res, secondary_res) = tokio::join!(primary_fut, secondary_fut);
            compare_and_unwrap::<R>(req_for_log, primary_res, secondary_res)
        }
        .boxed()
    }
}

fn compare_and_unwrap<R: Request + Debug>(
    request: R,
    primary: Result<R::Response, Error>,
    secondary: Result<R::Response, Error>,
) -> Result<R::Response, Error>
where
    R::Response: PartialEq + Debug,
{
    let request_name = type_name::<R>();
    let skip = request_name.ends_with("::Sync");
    match (primary, secondary) {
        (Ok(p), Ok(s)) => {
            if !skip && p != s {
                let diff = pretty_diff(&p, &s);
                let primary_size = format!("{:?}", p).len();
                let secondary_size = format!("{:?}", s).len();
                tracing::warn!(
                    request_type = request_name,
                    request = ?request,
                    primary_size,
                    secondary_size,
                    diff = %diff,
                    "comparing adapter: response mismatch",
                );
            }
            Ok(p)
        }
        (Ok(p), Err(s_err)) => {
            tracing::warn!(
                request_type = request_name,
                request = ?request,
                primary = ?p,
                secondary_error = %s_err,
                "comparing adapter: secondary failed while primary succeeded",
            );
            Ok(p)
        }
        (Err(p_err), Ok(s)) => {
            tracing::warn!(
                request_type = request_name,
                request = ?request,
                primary_error = %p_err,
                secondary = ?s,
                "comparing adapter: primary failed while secondary succeeded",
            );
            Err(p_err)
        }
        (Err(p_err), Err(s_err)) => {
            tracing::debug!(
                request_type = request_name,
                request = ?request,
                primary_error = %p_err,
                secondary_error = %s_err,
                "comparing adapter: both primary and secondary failed",
            );
            Err(p_err)
        }
    }
}

fn pretty_diff<T: Debug>(primary: &T, secondary: &T) -> String {
    let primary_text = format!("{:#?}", primary);
    let secondary_text = format!("{:#?}", secondary);
    let diff = TextDiff::from_lines(&primary_text, &secondary_text);
    format!(
        "{}",
        diff.unified_diff()
            .context_radius(2)
            .header("primary", "secondary")
    )
}

#[derive(Clone, Debug)]
pub struct MakeComparingAdapter<MP, MS> {
    primary_factory: MP,
    secondary_factory: MS,
}

impl<MP, MS> MakeComparingAdapter<MP, MS> {
    pub fn new(primary_factory: MP, secondary_factory: MS) -> Self {
        Self {
            primary_factory,
            secondary_factory,
        }
    }
}

impl<Cfg, MP, MS, P, S> Service<Cfg> for MakeComparingAdapter<MP, MS>
where
    Cfg: Clone + Send + 'static,
    MP: Service<Cfg, Response = P, Error = Error> + Clone + Send + 'static,
    MP::Future: Send + 'static,
    MS: Service<Cfg, Response = S, Error = Error> + Clone + Send + 'static,
    MS::Future: Send + 'static,
    P: Send + 'static,
    S: Send + 'static,
{
    type Response = ComparingAdapter<P, S>;
    type Error = Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.primary_factory.poll_ready(cx) {
            Poll::Ready(Ok(())) => self.secondary_factory.poll_ready(cx),
            other => other,
        }
    }

    fn call(&mut self, cfg: Cfg) -> Self::Future {
        let primary = self.primary_factory.clone();
        let secondary = self.secondary_factory.clone();
        let cfg_secondary = cfg.clone();
        async move {
            let (p, s) = tokio::try_join!(primary.oneshot(cfg), secondary.oneshot(cfg_secondary))?;
            Ok(ComparingAdapter::new(p, s))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use ton_tower::request::GetMasterchainInfo;
    use ton_tower::response::MasterchainInfo;

    #[tokio::test]
    async fn should_return_primary_when_responses_match() {
        let info = make_info("hash-a");
        let primary = MockService::ok(info.clone());
        let secondary = MockService::ok(info.clone());
        let adapter = ComparingAdapter::new(primary, secondary);

        let result = adapter
            .oneshot(GetMasterchainInfo::default())
            .await
            .unwrap();

        assert_eq!(result, info);
    }

    #[test]
    fn comparing_adapter_implements_ton_transport() {
        use static_assertions::assert_impl_all;
        use ton_client::TonService;
        use ton_liteserver_client::LiteServerAdapter;
        use tonlibjson_client::TonlibjsonAdapter;

        assert_impl_all!(ComparingAdapter<TonlibjsonAdapter, LiteServerAdapter>: TonService);
        assert_impl_all!(ComparingAdapter<LiteServerAdapter, TonlibjsonAdapter>: TonService);
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn should_log_mismatch_and_return_primary() {
        let primary_info = make_info("hash-primary");
        let secondary_info = make_info("hash-secondary");
        let primary = MockService::ok(primary_info.clone());
        let secondary = MockService::ok(secondary_info);
        let adapter = ComparingAdapter::new(primary, secondary);

        let result = adapter
            .oneshot(GetMasterchainInfo::default())
            .await
            .unwrap();

        assert_eq!(result, primary_info);
        assert!(logs_contain("response mismatch"));
    }

    #[test]
    fn pretty_diff_highlights_differing_fields() {
        let primary = make_info("hash-primary");
        let secondary = make_info("hash-secondary");

        let diff = pretty_diff(&primary, &secondary);

        assert!(
            diff.contains("-"),
            "diff must contain deletion markers: {diff}"
        );
        assert!(
            diff.contains("+"),
            "diff must contain insertion markers: {diff}"
        );
        assert!(
            diff.contains("hash-primary"),
            "diff must contain primary value: {diff}"
        );
        assert!(
            diff.contains("hash-secondary"),
            "diff must contain secondary value: {diff}"
        );
    }

    #[test]
    fn pretty_diff_is_empty_for_equal_values() {
        let info = make_info("hash-a");

        let diff = pretty_diff(&info, &info);

        assert!(
            diff.is_empty(),
            "diff for equal values must be empty: {diff}"
        );
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn should_return_primary_when_secondary_errs() {
        let info = make_info("hash-a");
        let primary = MockService::ok(info.clone());
        let secondary = MockService::err("boom");
        let adapter = ComparingAdapter::new(primary, secondary);

        let result = adapter
            .oneshot(GetMasterchainInfo::default())
            .await
            .unwrap();

        assert_eq!(result, info);
        assert!(logs_contain("secondary failed while primary succeeded"));
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn should_return_primary_error_when_primary_errs() {
        let primary = MockService::err("primary boom");
        let secondary = MockService::ok(make_info("hash-a"));
        let adapter = ComparingAdapter::new(primary, secondary);

        let result = adapter.oneshot(GetMasterchainInfo::default()).await;

        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "primary boom");
        assert!(logs_contain("primary failed while secondary succeeded"));
    }

    #[tokio::test]
    async fn should_call_both_services_exactly_once() {
        let primary_calls = Arc::new(AtomicUsize::new(0));
        let secondary_calls = Arc::new(AtomicUsize::new(0));
        let info = make_info("hash-a");
        let primary = MockService::ok_counting(info.clone(), primary_calls.clone());
        let secondary = MockService::ok_counting(info.clone(), secondary_calls.clone());
        let adapter = ComparingAdapter::new(primary, secondary);

        adapter
            .oneshot(GetMasterchainInfo::default())
            .await
            .unwrap();

        assert_eq!(primary_calls.load(Ordering::SeqCst), 1);
        assert_eq!(secondary_calls.load(Ordering::SeqCst), 1);
    }

    fn make_info(root_hash: &str) -> MasterchainInfo {
        MasterchainInfo {
            last: ton_tower::response::BlockIdExt {
                workchain: -1,
                shard: 0,
                seqno: 1,
                root_hash: root_hash.to_string(),
                file_hash: "f".to_string(),
            },
            state_root_hash: "s".to_string(),
            init: ton_tower::response::BlockIdExt {
                workchain: -1,
                shard: 0,
                seqno: 0,
                root_hash: "i".to_string(),
                file_hash: "i".to_string(),
            },
        }
    }

    #[derive(Clone)]
    struct MockService {
        outcome: Result<MasterchainInfo, String>,
        calls: Option<Arc<AtomicUsize>>,
    }

    impl MockService {
        fn ok(info: MasterchainInfo) -> Self {
            Self {
                outcome: Ok(info),
                calls: None,
            }
        }

        fn ok_counting(info: MasterchainInfo, calls: Arc<AtomicUsize>) -> Self {
            Self {
                outcome: Ok(info),
                calls: Some(calls),
            }
        }

        fn err(msg: &str) -> Self {
            Self {
                outcome: Err(msg.to_string()),
                calls: None,
            }
        }
    }

    impl Service<GetMasterchainInfo> for MockService {
        type Response = MasterchainInfo;
        type Error = Error;
        type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _: GetMasterchainInfo) -> Self::Future {
            if let Some(c) = &self.calls {
                c.fetch_add(1, Ordering::SeqCst);
            }
            let outcome = self.outcome.clone();
            async move { outcome.map_err(|e| anyhow::anyhow!(e)) }.boxed()
        }
    }
}
