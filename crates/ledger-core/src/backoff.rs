//! Retry backoff, as a pure function of the attempt count.

const BASE_MS: u64 = 1_000;
const CAP_MS: u64 = 60_000;

/// The delay before the next attempt, after `attempts_so_far` attempts have
/// failed: 1s, 2s, 4s, and so on, capped at 60s. There is no jitter. If
/// jitter is ever needed, its entropy must be passed in, because the core may
/// not draw it.
pub fn backoff_ms(attempts_so_far: u32) -> u64 {
    let doublings = attempts_so_far.saturating_sub(1).min(16);
    BASE_MS.saturating_mul(1_u64 << doublings).min(CAP_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_then_caps() {
        assert_eq!(backoff_ms(1), 1_000);
        assert_eq!(backoff_ms(2), 2_000);
        assert_eq!(backoff_ms(3), 4_000);
        assert_eq!(backoff_ms(7), 60_000);
        assert_eq!(backoff_ms(u32::MAX), 60_000);
    }
}
