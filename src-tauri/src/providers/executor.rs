// Native adaptation of Harness src/providers/retry.ts (MIT).
// P2: one standard retry per stage, one read/decode recovery, same operation budget.
use super::{
    transport::{CallBudget, TransportError},
    ProtocolError,
};
use std::future::Future;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Category {
    Standard,
    SingleRecovery,
    Never,
}
fn category(error: TransportError) -> Category {
    match error {
        TransportError::HttpStatus {
            status: 408 | 425 | 429 | 500 | 502 | 503 | 504 | 524 | 529,
            ..
        }
        | TransportError::Network
        | TransportError::Timeout => Category::Standard,
        TransportError::BodyRead
        | TransportError::InvalidJson
        | TransportError::Protocol(ProtocolError::EmptyResponse) => Category::SingleRecovery,
        _ => Category::Never,
    }
}
fn delay_ms(error: TransportError, failed_attempt: u8, fraction: f64) -> u64 {
    if let TransportError::HttpStatus {
        retry_after_ms: Some(ms),
        ..
    } = error
    {
        return ms.min(30_000);
    }
    let ceiling = 500u64
        .saturating_mul(
            1u64.checked_shl(u32::from(failed_attempt))
                .unwrap_or(u64::MAX),
        )
        .min(8000);
    let fraction = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    (fraction * ceiling as f64).floor() as u64
}
fn jitter() -> f64 {
    (uuid::Uuid::new_v4().as_u128() as u64) as f64 / (u64::MAX as f64)
}
/// The attempt returns only an atomic complete result. No streaming/UI callbacks
/// exist here: once Ok is returned the executor exits and cannot replay it.
/// Single recovery repeats the unchanged request; it never appends partial text,
/// changes model/route/key, increases token limits or bypasses provider refusal.
pub(super) async fn run<T, F, Fut>(budget: &CallBudget, mut attempt: F) -> Result<T, TransportError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, TransportError>>,
{
    let mut standard_used = false;
    let mut recovery_used = false;
    let mut failed_attempt = 0;
    loop {
        budget.ensure_active()?;
        match attempt().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                match category(error) {
                    Category::Standard if !standard_used => standard_used = true,
                    Category::SingleRecovery if !recovery_used => {
                        recovery_used = true;
                        standard_used = false;
                    }
                    _ => return Err(error),
                }
                let delay =
                    std::time::Duration::from_millis(delay_ms(error, failed_attempt, jitter()));
                failed_attempt = failed_attempt.saturating_add(1);
                budget.wait_for_retry(delay).await?;
            }
        }
    }
}
#[cfg(test)]
mod tests;
