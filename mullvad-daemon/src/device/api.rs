use std::pin::Pin;

use chrono::{DateTime, Utc};
use futures::{Future, future::FusedFuture};
use mullvad_types::account::VoucherSubmission;

use super::{Error, ResponseTx};

pub(crate) struct CurrentApiCall {
    current_call: Option<Call>,
}

impl CurrentApiCall {
    pub fn new() -> Self {
        Self { current_call: None }
    }

    pub fn clear(&mut self) {
        self.current_call = None;
    }

    pub fn set_expiry_check(&mut self, expiry_call: ApiCall<DateTime<Utc>>) {
        self.current_call = Some(Call::ExpiryCheck(expiry_call));
    }

    pub fn set_voucher_submission(
        &mut self,
        voucher_call: ApiCall<VoucherSubmission>,
        tx: ResponseTx<VoucherSubmission>,
    ) {
        self.current_call = Some(Call::VoucherSubmission(voucher_call, Some(tx)));
    }

    pub fn is_checking_expiry(&self) -> bool {
        matches!(&self.current_call, Some(Call::ExpiryCheck(_)))
    }

    /// Login is purely local, so there is never a login call in flight.
    ///
    /// Kept so the account-level handlers preserve the "reject while a
    /// state change is mid-flight" guard shape.
    pub fn is_logging_in(&self) -> bool {
        false
    }
}

impl Future for CurrentApiCall {
    type Output = ApiResult;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.current_call.as_mut() {
            Some(call) => {
                let result = Pin::new(call).poll(cx);
                if result.is_ready() {
                    self.current_call = None;
                }
                result
            }
            None => panic!("Polled an unfinished future"),
        }
    }
}

impl FusedFuture for CurrentApiCall {
    fn is_terminated(&self) -> bool {
        self.current_call.is_none()
    }
}

type ApiCall<T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send>>;

enum Call {
    VoucherSubmission(
        ApiCall<VoucherSubmission>,
        Option<ResponseTx<VoucherSubmission>>,
    ),
    ExpiryCheck(ApiCall<DateTime<Utc>>),
}

impl futures::Future for Call {
    type Output = ApiResult;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        use Call::*;
        match &mut *self {
            VoucherSubmission(call, tx) => match Pin::new(call).poll(cx) {
                std::task::Poll::Ready(response) => std::task::Poll::Ready(
                    ApiResult::VoucherSubmission(response, tx.take().unwrap()),
                ),
                _ => std::task::Poll::Pending,
            },
            ExpiryCheck(call) => Pin::new(call).poll(cx).map(ApiResult::ExpiryCheck),
        }
    }
}

pub(crate) enum ApiResult {
    VoucherSubmission(
        Result<VoucherSubmission, Error>,
        ResponseTx<VoucherSubmission>,
    ),
    ExpiryCheck(Result<DateTime<Utc>, Error>),
}
