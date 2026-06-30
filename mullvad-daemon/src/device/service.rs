use std::{future::Future, sync::Arc, time::Duration};

use futures::future::{AbortHandle, abortable};
#[cfg(target_os = "android")]
use mullvad_types::account::{PlayPurchase, PlayPurchasePaymentToken};
use mullvad_types::{
    account::{AccountData, AccountNumber, VoucherSubmission},
    warren_pubkey::WarrenPubKey,
};

use super::{Error, WarrenAccountBackend};
use mullvad_api::{
    AccountsProxy,
    availability::ApiAvailability,
    rest::{self, MullvadRestHandle},
};
use talpid_future::retry::{ConstantInterval, ExponentialBackoff, Jittered, retry_future};
/// Retry strategy used for user-initiated actions that require immediate feedback
const RETRY_ACTION_STRATEGY: ConstantInterval = ConstantInterval::new(Duration::ZERO, Some(3));
/// Retry strategy used for background tasks
const RETRY_BACKOFF_STRATEGY: Jittered<ExponentialBackoff> = Jittered::jitter(
    ExponentialBackoff::new(Duration::from_secs(4), 5).max_delay(Some(Duration::from_hours(24))),
);

#[derive(Clone)]
pub struct WarrenIdentityService {
    api_availability: ApiAvailability,
    initial_check_abort_handle: AbortHandle,
    /// Mullvad `AccountsProxy` for the non-MVP methods (voucher,
    /// www_auth_token, init/verify_play_purchase Android). Keeps the
    /// legacy path as long as these flows are not migrated.
    proxy: AccountsProxy,
    /// Abstract backend for the critical MVP methods
    /// (`get_data`, `delete_account`, `submit_voucher`). The caller
    /// (`spawn_warren_identity_service`) injects `WarrenRemoteAccountBackend`
    /// (warren-api) or the legacy `RemoteAccountBackend`.
    backend: Arc<dyn WarrenAccountBackend>,
}

impl WarrenIdentityService {
    pub fn get_www_auth_token(
        &self,
        account: AccountNumber,
    ) -> impl Future<Output = Result<String, rest::Error>> + use<> {
        let proxy = self.proxy.clone();
        let api_handle = self.api_availability.clone();
        retry_future(
            move || proxy.get_www_auth_token(account.clone()),
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
    }

    /// Takes a `WarrenPubKey` instead of a raw `AccountNumber`.
    /// Routes through [`WarrenAccountBackend`] to support local-mode POC.
    pub async fn get_data(&self, pubkey: WarrenPubKey) -> Result<AccountData, rest::Error> {
        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();
        let account_number: AccountNumber = pubkey.as_str().to_owned();
        let result = retry_future(
            move || backend.get_data(account_number.clone()),
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
        .await;
        if handle_account_data_result(&result, &self.api_availability) {
            self.initial_check_abort_handle.abort();
        }
        result
    }

    pub async fn get_data_2(&self, pubkey: WarrenPubKey) -> Result<AccountData, Error> {
        self.get_data(pubkey).await.map_err(map_rest_error)
    }

    pub async fn submit_voucher(
        &self,
        pubkey: WarrenPubKey,
        voucher: String,
    ) -> Result<VoucherSubmission, Error> {
        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();
        let account_number: AccountNumber = pubkey.as_str().to_owned();
        let result = retry_future(
            move || backend.submit_voucher(account_number.clone(), voucher.clone()),
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
        .await;
        if result.is_ok() {
            self.initial_check_abort_handle.abort();
            self.api_availability.resume_background();
        }
        result.map_err(map_rest_error)
    }

    #[cfg(target_os = "android")]
    pub async fn init_play_purchase(
        &self,
        pubkey: WarrenPubKey,
    ) -> Result<PlayPurchasePaymentToken, Error> {
        let mut proxy = self.proxy.clone();
        let api_handle = self.api_availability.clone();
        let account_number: AccountNumber = pubkey.as_str().to_owned();
        let factory = move || {
            let account_number = account_number.clone();

            proxy.init_play_purchase(account_number)
        };
        let result = retry_future(
            factory,
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
        .await;
        if result.is_ok() {
            self.initial_check_abort_handle.abort();
            self.api_availability.resume_background();
        }
        result.map_err(map_rest_error)
    }

    #[cfg(target_os = "android")]
    pub async fn verify_play_purchase(
        &self,
        pubkey: WarrenPubKey,
        play_purchase: PlayPurchase,
    ) -> Result<(), Error> {
        let mut proxy = self.proxy.clone();
        let api_handle = self.api_availability.clone();
        let account_number: AccountNumber = pubkey.as_str().to_owned();
        let factory = move || {
            let account_number = account_number.clone();
            let play_purchase = play_purchase.clone();

            proxy.verify_play_purchase(account_number, play_purchase)
        };
        let result = retry_future(
            factory,
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
        .await;
        if result.is_ok() {
            self.initial_check_abort_handle.abort();
            self.api_availability.resume_background();
        }
        result.map_err(map_rest_error)
    }

    #[cfg(target_os = "android")]
    pub async fn delete_account(&self, pubkey: WarrenPubKey) -> Result<(), Error> {
        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();
        let account_number: AccountNumber = pubkey.as_str().to_owned();

        let factory = move || {
            let account_number = account_number.clone();
            backend.delete_account(account_number)
        };
        retry_future(
            factory,
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
        .await
        .map_err(map_rest_error)?;

        Ok(())
    }
}

pub fn spawn_warren_identity_service(
    api_handle: MullvadRestHandle,
    number: Option<AccountNumber>,
    api_availability: ApiAvailability,
    backend: Arc<dyn WarrenAccountBackend>,
) -> WarrenIdentityService {
    let accounts_proxy = AccountsProxy::new(api_handle);
    api_availability.pause_background();

    let api_availability_copy = api_availability.clone();
    let accounts_proxy_copy = accounts_proxy.clone();
    let backend_copy = backend.clone();

    let (future, initial_check_abort_handle) = abortable(async move {
        let Some(number) = number else {
            // Fresh install pre-login: no `AccountNumber` known,
            // nothing to ask the backend. We leave `api_availability`
            // paused (already done just before the `abortable`).
            api_availability.pause_background();
            return;
        };

        let future_generator = move || {
            let backend = backend.clone();
            let number = number.clone();
            let api_availability = api_availability.clone();
            async move {
                let expiry_fut = api_availability.when_online(backend.get_data(number)).await;
                handle_account_data_result(&expiry_fut, &api_availability)
            }
        };
        let should_retry = move |state_was_updated: &bool| -> bool { !*state_was_updated };
        retry_future(future_generator, should_retry, RETRY_BACKOFF_STRATEGY).await;
    });
    tokio::spawn(future);

    WarrenIdentityService {
        api_availability: api_availability_copy,
        initial_check_abort_handle,
        proxy: accounts_proxy_copy,
        backend: backend_copy,
    }
}

fn handle_account_data_result(
    result: &Result<AccountData, rest::Error>,
    api_availability: &ApiAvailability,
) -> bool {
    match result {
        Ok(_data) if _data.expiry >= chrono::Utc::now() => {
            api_availability.resume_background();
            true
        }
        Ok(_data) => {
            api_availability.pause_background();
            true
        }
        Err(mullvad_api::rest::Error::ApiError(status, code)) => {
            // HTTP 404 from warren-api means "no subscription found for this pubkey".
            // This is NOT a transient network error - retrying forever would produce
            // an infinite loop before the user redeems a voucher.  Treat it the same
            // way as INVALID_ACCOUNT: pause background activity and report "settled"
            // (= stop the retry loop) so the caller can surface the state to the UI.
            if *status == rest::StatusCode::NOT_FOUND {
                log::info!(
                    "handle_account_data_result: 404 from warren-api \
                     (no subscription yet) - pausing background, stopping retry loop"
                );
                api_availability.pause_background();
                return true;
            }
            if code == mullvad_api::INVALID_ACCOUNT {
                api_availability.pause_background();
                return true;
            }
            false
        }
        Err(_) => false,
    }
}

fn should_retry<T>(result: &Result<T, rest::Error>, api_handle: &ApiAvailability) -> bool {
    match result {
        Err(error) if error.is_network_error() => !api_handle.is_offline(),
        _ => false,
    }
}

fn map_rest_error(error: rest::Error) -> Error {
    match error {
        rest::Error::ApiError(_status, ref code) => match code.as_str() {
            // TODO: Implement invalid payment
            mullvad_api::DEVICE_NOT_FOUND | mullvad_api::INVALID_ACCESS_TOKEN => {
                Error::InvalidDevice
            }
            mullvad_api::INVALID_ACCOUNT => Error::InvalidAccount,
            mullvad_api::INVALID_VOUCHER => Error::InvalidVoucher,
            mullvad_api::VOUCHER_USED => Error::UsedVoucher,
            _ => Error::OtherRestError(error),
        },
        error => Error::OtherRestError(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mullvad_api::availability::{ApiAvailability, State};

    /// Builds a fresh `ApiAvailability` whose background is initially
    /// un-paused (= online, active). Spawning the inactivity timer
    /// requires a tokio runtime, so all tests here use `#[tokio::test]`.
    fn online_availability() -> ApiAvailability {
        ApiAvailability::new(State::default())
    }

    // ------------------------------------------------------------------
    // G-2: 404 from warren-api must stop the retry loop
    // ------------------------------------------------------------------

    /// A 404 ApiError must be treated as "settled" (return `true`) so
    /// the `spawn_warren_identity_service` retry loop terminates
    /// immediately. Without this fix the loop would continue forever
    /// before the user redeems a voucher.
    #[tokio::test]
    async fn handle_account_data_result_404_stops_retry_loop() {
        let api_availability = online_availability();

        let result: Result<AccountData, rest::Error> = Err(rest::Error::ApiError(
            rest::StatusCode::NOT_FOUND,
            "warren-api 404".to_owned(),
        ));

        // Must return true (= settled, stop retrying).
        let settled = handle_account_data_result(&result, &api_availability);
        assert!(
            settled,
            "404 from warren-api must settle the retry loop (return true)"
        );
    }

    /// After a 404, background API calls must be paused so the daemon
    /// does not keep making requests while the user has no subscription.
    /// We verify "paused" by asserting that `wait_background()` does NOT
    /// resolve within 50 ms - if it did, background would be un-paused.
    #[tokio::test]
    async fn handle_account_data_result_404_pauses_background() {
        let api_availability = online_availability();

        let result: Result<AccountData, rest::Error> = Err(rest::Error::ApiError(
            rest::StatusCode::NOT_FOUND,
            "warren-api 404".to_owned(),
        ));

        handle_account_data_result(&result, &api_availability);

        // If background is paused, wait_background blocks indefinitely.
        // A 50 ms timeout expiring means it is indeed paused (correct).
        // If wait_background resolves instantly the future returns Ok(()),
        // meaning background is NOT paused (bug).
        let paused = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            api_availability.wait_background(),
        )
        .await
        .is_err(); // Err(Elapsed) -> paused; Ok(_) -> NOT paused (wrong)

        assert!(paused, "404 must pause background API activity");
    }

    /// A transient network error (= `rest::Error::Aborted`) must NOT
    /// stop the retry loop - the daemon should keep retrying until the
    /// network is back. This is the pre-existing contract; the G-2 fix
    /// must not regress it.
    #[tokio::test]
    async fn handle_account_data_result_transient_error_keeps_retrying() {
        let api_availability = online_availability();

        let result: Result<AccountData, rest::Error> = Err(rest::Error::Aborted);

        // Must return false (= not settled, keep retrying).
        let settled = handle_account_data_result(&result, &api_availability);
        assert!(
            !settled,
            "transient network error must NOT stop the retry loop"
        );
    }

    /// INVALID_ACCOUNT (symbolic string code) must still stop the retry
    /// loop as before - this is the pre-existing behavior and must not
    /// be broken by the G-2 change.
    #[tokio::test]
    async fn handle_account_data_result_invalid_account_stops_retry_loop() {
        let api_availability = online_availability();

        let result: Result<AccountData, rest::Error> = Err(rest::Error::ApiError(
            rest::StatusCode::UNAUTHORIZED,
            mullvad_api::INVALID_ACCOUNT.to_owned(),
        ));

        let settled = handle_account_data_result(&result, &api_availability);
        assert!(
            settled,
            "INVALID_ACCOUNT must settle the retry loop (pre-existing behavior)"
        );
    }
}
