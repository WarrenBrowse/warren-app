//! Trusted clock for the wallet-signed community-forum requests (doc 55).
//!
//! The broker refuses a signature whose timestamp is more than a minute from
//! its own clock. A machine whose clock has never synchronised is therefore
//! refused on every attempt, and the refusal reads as "your clock is off",
//! which the reporter filing a bug report can do nothing about from inside
//! the app. The mobile clients already correct for it: they read the `Date`
//! header of a TLS-authenticated answer from the connect host and stamp the
//! request with the server's clock. The desktop signs inside the daemon, so
//! the correction belongs here, in front of the signature, and the GUI never
//! gets to choose the timestamp a wallet signs at.
//!
//! **No-log policy**: only the offset (a duration) is ever logged, never the
//! request, the key or anything the report carries.

use std::time::Duration;

/// Connect and total budget of the clock read. Deliberately under the
/// deadline of the report upload that follows: a broker that cannot be
/// reached at all must cost the reporter a few seconds, not the whole
/// attempt, and the signature still goes out on the device clock.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(8);

/// The window the broker accepts a signature in, either side of its own
/// clock. A device inside it needs no correction at all, so none is applied:
/// the server gets to move the stamp only when the stamp would otherwise be
/// refused.
const BROKER_WINDOW_SECS: i64 = 60;

/// How far the stamp may be moved FORWARD, at the answer's word.
///
/// The two directions are not the same risk. A stamp moved into the past is
/// spent: a signature carrying an instant that has gone by is replayable only
/// at an instant that has gone by. A stamp moved into the future is not: an
/// answer claiming a `Date` months ahead mints signatures the broker can hold
/// and present at the moment they become current, for a wallet whose owner
/// was not acting then. So a backward correction is bounded only by the
/// representable range, and a forward one by the largest drift this has ever
/// had to cover: the incident that put the correction here was five minutes
/// (`73e03e8bfb`), and past a quarter of an hour the device clock is the more
/// credible of the two.
const MAX_FORWARD_CORRECTION_SECS: i64 = 15 * 60;

/// The endpoint the clock is read from: the broker's health answer is
/// unauthenticated, carries nothing about the caller, and is served by the
/// same host under the same certificate as the signed POST that follows.
pub(crate) fn health_url(host: &str) -> String {
    format!("https://{host}/healthz")
}

/// The device clock's offset from the broker's, in seconds, positive when the
/// device is behind. Zero when the answer carried no usable `Date`: the
/// request is then stamped with the device clock and the broker decides, as
/// before the correction existed.
pub(crate) fn offset_from_date(date_header: Option<&str>, device_now: u64) -> i64 {
    date_header
        .and_then(|date| warren_forum::clock_offset_secs(date, device_now))
        .unwrap_or(0)
}

/// The timestamp a forum request is signed at: the device clock shifted by the
/// part of the measured offset that is actually applied. A shift that would
/// leave the representable range keeps the device stamp, so a broken clock
/// degrades to the old behaviour instead of signing at an absurd instant.
pub(crate) fn corrected_timestamp(device_now: u64, offset_secs: i64) -> u64 {
    let applied = applicable_offset(offset_secs);
    i64::try_from(device_now)
        .ok()
        .and_then(|now| now.checked_add(applied))
        .and_then(|corrected| u64::try_from(corrected).ok())
        .unwrap_or(device_now)
}

/// The part of the measured offset the stamp is actually moved by.
///
/// Zero in the two cases where the answer must not decide the instant a
/// wallet signs at: a device already inside the broker's window (nothing to
/// fix), and a `Date` further ahead than any drift this exists to cover (see
/// [`MAX_FORWARD_CORRECTION_SECS`]).
pub(crate) fn applicable_offset(offset_secs: i64) -> i64 {
    if offset_secs.abs() * 2 <= BROKER_WINDOW_SECS || offset_secs > MAX_FORWARD_CORRECTION_SECS {
        0
    } else {
        offset_secs
    }
}

/// Reads the broker's clock once and returns the timestamp to sign at.
/// Every failure (no client, no answer, no `Date`) falls back to the device
/// clock, which is what the daemon signed with before this existed.
pub(crate) async fn signing_timestamp(host: &str) -> u64 {
    let device_now = crate::warren_artifact_refresh::now_unix();
    let offset = read_offset(host, device_now).await;
    match applicable_offset(offset) {
        0 if offset > MAX_FORWARD_CORRECTION_SECS => log::info!(
            "Forum clock: the broker answered {offset} s ahead, too far to stamp at; signing on this machine's clock"
        ),
        0 => (),
        applied => log::info!(
            "Forum clock: this machine is {applied} s off the broker, correcting the stamp"
        ),
    }
    corrected_timestamp(device_now, offset)
}

async fn read_offset(host: &str, device_now: u64) -> i64 {
    let Ok(client) = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(TOTAL_TIMEOUT)
        .build()
    else {
        return 0;
    };
    match client.get(health_url(host)).send().await {
        Ok(response) => {
            let date = response
                .headers()
                .get(reqwest::header::DATE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            offset_from_date(date.as_deref(), device_now)
        }
        Err(_) => {
            // The cause is not logged: a transport error can quote the
            // request, and the class is all this line needs.
            log::debug!("Forum clock: the broker's clock could not be read, signing anyway");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{applicable_offset, corrected_timestamp, health_url, offset_from_date};

    /// 2023-11-14 22:13:20 UTC, the instant the vectors are pinned at.
    const SERVER_NOW: u64 = 1_700_000_000;
    const SERVER_DATE: &str = "Tue, 14 Nov 2023 22:13:20 GMT";

    #[test]
    fn a_machine_behind_the_broker_signs_at_the_brokers_clock() {
        // The class this exists for: five minutes slow is inside nobody's
        // notice and outside the broker's 60 s window, so every report was
        // refused with a notice about the clock and no way to act on it.
        let device_now = SERVER_NOW - 300;
        let offset = offset_from_date(Some(SERVER_DATE), device_now);
        assert_eq!(offset, 300);
        assert_eq!(corrected_timestamp(device_now, offset), SERVER_NOW);
    }

    #[test]
    fn a_machine_ahead_of_the_broker_is_pulled_back() {
        let device_now = SERVER_NOW + 3_600;
        let offset = offset_from_date(Some(SERVER_DATE), device_now);
        assert_eq!(offset, -3_600);
        assert_eq!(corrected_timestamp(device_now, offset), SERVER_NOW);
    }

    #[test]
    fn an_answer_without_a_usable_date_leaves_the_device_clock_alone() {
        // Nothing to correct against is not a reason to refuse to sign: the
        // broker decides, as it did before the correction existed.
        assert_eq!(offset_from_date(None, SERVER_NOW), 0);
        assert_eq!(offset_from_date(Some("not a date"), SERVER_NOW), 0);
        assert_eq!(corrected_timestamp(SERVER_NOW, 0), SERVER_NOW);
    }

    #[test]
    fn a_correction_that_would_leave_the_range_keeps_the_device_stamp() {
        // A clock at the epoch with a server an hour ahead must not wrap.
        assert_eq!(corrected_timestamp(10, -3_600), 10);
        assert_eq!(corrected_timestamp(u64::MAX, 1), u64::MAX);
    }

    /// The answer decides the instant a wallet signs at, so how far forward
    /// it may push that instant is bounded: a `Date` months ahead would mint
    /// signatures the broker can hold and present when they become current.
    #[test]
    fn a_broker_answering_from_the_future_does_not_move_the_stamp() {
        let a_year = 365 * 24 * 60 * 60;
        assert_eq!(applicable_offset(a_year), 0);
        assert_eq!(corrected_timestamp(SERVER_NOW, a_year), SERVER_NOW);

        assert_eq!(applicable_offset(3_600), 0);
        assert_eq!(corrected_timestamp(SERVER_NOW, 3_600), SERVER_NOW);
    }

    /// The other direction is spent the moment it is signed: a stamp in the
    /// past is replayable only in the past, so a device stuck years ahead is
    /// still pulled back to the broker and can still file its report.
    #[test]
    fn a_device_running_ahead_is_still_pulled_back_however_far() {
        let a_year = 365 * 24 * 60 * 60;
        assert_eq!(applicable_offset(-a_year), -a_year);
        assert_eq!(
            corrected_timestamp(SERVER_NOW + u64::try_from(a_year).unwrap(), -a_year),
            SERVER_NOW
        );
    }

    /// The broker accepts a stamp within a minute of its own, so a device
    /// already inside that window is left exactly as it is: the correction
    /// exists for a stamp that would otherwise be refused, and nothing else.
    #[test]
    fn a_device_inside_the_brokers_window_is_left_alone() {
        assert_eq!(applicable_offset(20), 0);
        assert_eq!(applicable_offset(-30), 0);
        assert_eq!(corrected_timestamp(SERVER_NOW, 20), SERVER_NOW);
    }

    #[test]
    fn the_clock_is_read_from_the_brokers_own_health_endpoint() {
        // Same host and same certificate as the signed POST that follows, and
        // an endpoint that carries nothing about the caller.
        assert_eq!(
            health_url("connect.warrenbrowse.com"),
            "https://connect.warrenbrowse.com/healthz"
        );
    }
}
