use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};

pub(crate) trait DeadlineCheck {
    fn check(&self, phase: &str) -> Result<()>;

    fn remaining_timeout_micros(&self, _phase: &str) -> Result<Option<u64>> {
        Ok(None)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CooperativeDeadline {
    deadline: Option<Instant>,
    timeout_ms: Option<u64>,
    operation: &'static str,
}

impl CooperativeDeadline {
    pub(crate) fn new(
        timeout_ms: Option<u64>,
        max_timeout_ms: u64,
        operation: &'static str,
    ) -> Result<Self> {
        if timeout_ms == Some(0) {
            return Err(anyhow!(
                "invalid {operation} timeout_ms: value must be greater than zero"
            ));
        }
        if timeout_ms.is_some_and(|value| value > max_timeout_ms) {
            return Err(anyhow!(
                "invalid {operation} timeout_ms: value must not exceed {max_timeout_ms}"
            ));
        }

        Ok(Self {
            deadline: timeout_ms.map(|value| Instant::now() + Duration::from_millis(value)),
            timeout_ms,
            operation,
        })
    }

    pub(crate) fn check(&self, phase: &str) -> Result<()> {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(self.timeout_error(phase));
        }
        Ok(())
    }

    pub(crate) fn remaining_timeout_ms(&self, phase: &str) -> Result<Option<u64>> {
        let Some(deadline) = self.deadline else {
            return Ok(None);
        };
        let remaining_ms = ceil_duration_millis(deadline.saturating_duration_since(Instant::now()));
        if remaining_ms == 0 {
            return Err(self.timeout_error(phase));
        }
        Ok(Some(remaining_ms))
    }

    fn timeout_error(&self, phase: &str) -> anyhow::Error {
        anyhow!(
            "{} timeout exceeded during {phase}: timeout_ms={}",
            self.operation,
            self.timeout_ms.unwrap_or_default()
        )
    }

    #[cfg(test)]
    pub(crate) fn expired_for_tests(timeout_ms: u64, operation: &'static str) -> Self {
        Self {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(timeout_ms),
            operation,
        }
    }
}

impl DeadlineCheck for CooperativeDeadline {
    fn check(&self, phase: &str) -> Result<()> {
        CooperativeDeadline::check(self, phase)
    }

    fn remaining_timeout_micros(&self, phase: &str) -> Result<Option<u64>> {
        self.remaining_timeout_ms(phase)
            .map(|timeout_ms| timeout_ms.map(|timeout_ms| timeout_ms.saturating_mul(1_000)))
    }
}

fn ceil_duration_millis(duration: Duration) -> u64 {
    duration
        .as_micros()
        .saturating_add(999)
        .saturating_div(1_000)
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{CooperativeDeadline, ceil_duration_millis};

    #[test]
    fn validates_configured_timeout_bounds_and_labels() {
        let zero = CooperativeDeadline::new(Some(0), 10, "semantic skeleton")
            .expect_err("zero timeout should fail");
        assert!(
            zero.to_string()
                .contains("invalid semantic skeleton timeout_ms")
        );

        let excessive = CooperativeDeadline::new(Some(11), 10, "semantic skeleton")
            .expect_err("excessive timeout should fail");
        assert!(excessive.to_string().contains("must not exceed 10"));
        assert!(CooperativeDeadline::new(Some(10), 10, "semantic skeleton").is_ok());
    }

    #[test]
    fn reports_configured_operation_for_expired_deadlines() {
        let deadline = CooperativeDeadline::expired_for_tests(1, "workspace edit preview");
        let error = deadline
            .check("source read")
            .expect_err("expired deadline should fail");
        assert!(
            error
                .to_string()
                .contains("workspace edit preview timeout exceeded during source read")
        );
        assert!(error.to_string().contains("timeout_ms=1"));
    }

    #[test]
    fn exposes_remaining_parser_budget_in_microseconds() {
        let deadline = CooperativeDeadline::new(Some(10), 10, "semantic skeleton").unwrap();
        let remaining = super::DeadlineCheck::remaining_timeout_micros(&deadline, "parsing")
            .unwrap()
            .expect("configured deadline should expose a parser budget");
        assert!(remaining > 0);
        assert!(remaining <= 10_000);

        let unbounded = CooperativeDeadline::new(None, 10, "semantic skeleton").unwrap();
        assert_eq!(
            super::DeadlineCheck::remaining_timeout_micros(&unbounded, "parsing").unwrap(),
            None
        );
    }

    #[test]
    fn rounds_remaining_budget_up_to_milliseconds() {
        assert_eq!(ceil_duration_millis(Duration::from_micros(1)), 1);
        assert_eq!(ceil_duration_millis(Duration::from_millis(1)), 1);
        assert_eq!(ceil_duration_millis(Duration::from_micros(1_001)), 2);
    }
}
