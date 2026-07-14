use thiserror::Error;

const SCALE: u32 = 100;

/// A fixed or continuous variable-credit range, represented in hundredths
/// to keep comparisons deterministic and avoid floating point edge drift.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditValue {
    minimum: u32,
    maximum: u32,
}

impl CreditValue {
    pub const fn minimum_hundredths(self) -> u32 {
        self.minimum
    }

    pub const fn maximum_hundredths(self) -> u32 {
        self.maximum
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CreditValueParseError {
    #[error("credit value is empty, arranged, or variable without numeric bounds")]
    Unbounded,
    #[error("credit value is not one fixed number or one ordered numeric range")]
    Invalid,
}

/// Parse the canonical Rutgers credit display without guessing arranged or
/// unbounded values. Decimal precision is capped at two places because the
/// query wire contract uses hundredths.
pub fn parse_credit_value(value: &str) -> Result<CreditValue, CreditValueParseError> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.is_empty()
        || ["BA", "BY ARRANGEMENT", "ARRANGED", "VARIABLE", "VAR"].contains(&normalized.as_str())
    {
        return Err(CreditValueParseError::Unbounded);
    }

    let numeric = normalized
        .replace(" TO ", "-")
        .replace(" THROUGH ", "-")
        .replace(['–', '—'], "-");
    let parts = numeric.split('-').map(str::trim).collect::<Vec<_>>();
    let (minimum, maximum) = match parts.as_slice() {
        [fixed] => {
            let fixed = parse_decimal(fixed)?;
            (fixed, fixed)
        }
        [minimum, maximum] => (parse_decimal(minimum)?, parse_decimal(maximum)?),
        _ => return Err(CreditValueParseError::Invalid),
    };
    if minimum > maximum {
        return Err(CreditValueParseError::Invalid);
    }
    Ok(CreditValue { minimum, maximum })
}

fn parse_decimal(value: &str) -> Result<u32, CreditValueParseError> {
    if value.is_empty() || value.starts_with('+') || value.starts_with('-') {
        return Err(CreditValueParseError::Invalid);
    }
    let components = value.split('.').collect::<Vec<_>>();
    let (whole, fractional) = match components.as_slice() {
        [whole] => (*whole, ""),
        [whole, fractional] => (*whole, *fractional),
        _ => return Err(CreditValueParseError::Invalid),
    };
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fractional.len() > 2
        || !fractional.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(CreditValueParseError::Invalid);
    }
    let whole = whole
        .parse::<u32>()
        .map_err(|_| CreditValueParseError::Invalid)?;
    let fraction = if fractional.is_empty() {
        0
    } else {
        fractional
            .parse::<u32>()
            .map_err(|_| CreditValueParseError::Invalid)?
            * 10_u32.pow(2 - fractional.len() as u32)
    };
    whole
        .checked_mul(SCALE)
        .and_then(|scaled| scaled.checked_add(fraction))
        .ok_or(CreditValueParseError::Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_decimal_and_ranges_are_exact() {
        assert_eq!(
            parse_credit_value(" 1.25 ").unwrap(),
            CreditValue {
                minimum: 125,
                maximum: 125
            }
        );
        assert_eq!(
            parse_credit_value("1.5 TO 4").unwrap(),
            CreditValue {
                minimum: 150,
                maximum: 400
            }
        );
        assert_eq!(
            parse_credit_value("1–3").unwrap(),
            CreditValue {
                minimum: 100,
                maximum: 300
            }
        );
    }

    #[test]
    fn unbounded_and_malformed_values_are_not_guessed() {
        assert_eq!(
            parse_credit_value("BA"),
            Err(CreditValueParseError::Unbounded)
        );
        assert_eq!(
            parse_credit_value("3 OR 4"),
            Err(CreditValueParseError::Invalid)
        );
        assert_eq!(
            parse_credit_value("4-1"),
            Err(CreditValueParseError::Invalid)
        );
    }
}
