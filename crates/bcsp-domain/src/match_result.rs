use bcsp_contracts::MatchOutcome;

pub trait MatchOutcomeAlgebra: Sized {
    fn and(self, other: Self) -> Self;
    fn or(self, other: Self) -> Self;
    fn all(values: impl IntoIterator<Item = Self>) -> Self;
    fn any(values: impl IntoIterator<Item = Self>) -> Self;
}

impl MatchOutcomeAlgebra for MatchOutcome {
    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::NoMatch, _) | (_, Self::NoMatch) => Self::NoMatch,
            (Self::Uncertain, _) | (_, Self::Uncertain) => Self::Uncertain,
            (Self::Match, Self::Match) => Self::Match,
        }
    }

    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::Match, _) | (_, Self::Match) => Self::Match,
            (Self::Uncertain, _) | (_, Self::Uncertain) => Self::Uncertain,
            (Self::NoMatch, Self::NoMatch) => Self::NoMatch,
        }
    }

    fn all(values: impl IntoIterator<Item = Self>) -> Self {
        values
            .into_iter()
            .fold(Self::Match, |combined, value| combined.and(value))
    }

    fn any(values: impl IntoIterator<Item = Self>) -> Self {
        values
            .into_iter()
            .fold(Self::NoMatch, |combined, value| combined.or(value))
    }
}

pub fn evaluate_active_dimension(values: impl IntoIterator<Item = MatchOutcome>) -> MatchOutcome {
    let mut values = values.into_iter().peekable();
    if values.peek().is_none() {
        MatchOutcome::Match
    } else {
        MatchOutcome::any(values)
    }
}
