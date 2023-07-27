#![cfg(test)]

use std::error::Error;
use std::fmt;

use xpct::core::{Matcher, SimpleMatch};
use xpct::format::MismatchFormat;
use xpct::matchers::{Mismatch, Pattern};

struct ErrorMatcher<'a, ExpectedErr> {
    pattern: Pattern<'a, ExpectedErr>,
}

impl<'a, Actual, ActualErr, ExpectedErr> SimpleMatch<Result<Actual, ActualErr>>
    for ErrorMatcher<'a, ExpectedErr>
where
    ActualErr: AsRef<dyn Error + 'static>,
    ExpectedErr: Error + Send + Sync + 'static,
{
    type Fail = Mismatch<Pattern<'a, ExpectedErr>, Result<Actual, ActualErr>>;

    fn matches(&mut self, actual: &Result<Actual, ActualErr>) -> xpct::Result<bool> {
        match actual {
            Ok(_) => Ok(false),
            Err(err) => match ActualErr::as_ref(err).downcast_ref::<ExpectedErr>() {
                Some(downcast_err) if self.pattern.matches(downcast_err) => Ok(true),
                _ => Ok(false),
            },
        }
    }

    fn fail(self, actual: Result<Actual, ActualErr>) -> Self::Fail {
        Mismatch {
            actual,
            expected: self.pattern,
        }
    }
}

/// Succeeds when the actual value is an error which can be downcast and matches the expected
/// pattern.
pub fn match_err<'a, Actual, ActualErr, ExpectedErr>(
    pattern: Pattern<'a, ExpectedErr>,
) -> Matcher<'a, Result<Actual, ActualErr>, Result<Actual, ActualErr>>
where
    Actual: fmt::Debug + 'a,
    ActualErr: fmt::Debug + AsRef<dyn Error + 'static> + 'a,
    ExpectedErr: fmt::Debug + Error + Send + Sync + 'static,
{
    Matcher::simple(
        ErrorMatcher { pattern },
        MismatchFormat::new(
            "to be an error matching the pattern",
            "to not be an error matching the pattern",
        ),
    )
}
