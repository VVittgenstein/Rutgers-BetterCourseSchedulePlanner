use bcsp_contracts::{MAX_ACTIVE_WATCHES, SectionKey};
use thiserror::Error;

/// The product limit is per browser/document session. Duplicate identities do
/// not consume another slot because SectionKey is the complete identity.
pub const MAX_SELECTED_SECTIONS: usize = MAX_ACTIVE_WATCHES as usize;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SelectionError {
    #[error("a browser session may select at most nine Sections")]
    LimitExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionChange {
    Added,
    AlreadySelected,
    Removed,
    NotSelected,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SectionSelection {
    sections: Vec<SectionKey>,
}

impl SectionSelection {
    pub const fn new() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    pub fn try_from_sections(
        sections: impl IntoIterator<Item = SectionKey>,
    ) -> Result<Self, SelectionError> {
        let mut selection = Self::new();
        for section in sections {
            selection.add(section)?;
        }
        Ok(selection)
    }

    pub fn sections(&self) -> &[SectionKey] {
        &self.sections
    }

    pub fn len(&self) -> usize {
        self.sections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    pub fn contains(&self, section: &SectionKey) -> bool {
        self.sections.contains(section)
    }

    pub fn add(&mut self, section: SectionKey) -> Result<SelectionChange, SelectionError> {
        if self.contains(&section) {
            return Ok(SelectionChange::AlreadySelected);
        }
        if self.sections.len() == MAX_SELECTED_SECTIONS {
            return Err(SelectionError::LimitExceeded);
        }
        self.sections.push(section);
        Ok(SelectionChange::Added)
    }

    pub fn remove(&mut self, section: &SectionKey) -> SelectionChange {
        let Some(index) = self.sections.iter().position(|value| value == section) else {
            return SelectionChange::NotSelected;
        };
        self.sections.remove(index);
        SelectionChange::Removed
    }

    pub fn clear(&mut self) {
        self.sections.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(index: usize) -> SectionKey {
        SectionKey::try_new("92026", "NB", &format!("{index:05}")).expect("synthetic SectionKey")
    }

    #[test]
    fn ninth_is_accepted_and_tenth_is_explicitly_rejected() {
        let mut selection = SectionSelection::new();
        for index in 1..=MAX_SELECTED_SECTIONS {
            assert_eq!(selection.add(section(index)), Ok(SelectionChange::Added));
        }
        assert_eq!(
            selection.add(section(MAX_SELECTED_SECTIONS + 1)),
            Err(SelectionError::LimitExceeded)
        );
        assert_eq!(selection.len(), MAX_SELECTED_SECTIONS);
    }

    #[test]
    fn duplicate_identity_is_idempotent_and_does_not_consume_capacity() {
        let mut selection = SectionSelection::new();
        assert_eq!(selection.add(section(1)), Ok(SelectionChange::Added));
        assert_eq!(
            selection.add(section(1)),
            Ok(SelectionChange::AlreadySelected)
        );
        assert_eq!(selection.len(), 1);
    }

    #[test]
    fn replace_style_construction_is_atomic_on_limit_failure() {
        assert_eq!(
            SectionSelection::try_from_sections((1..=10).map(section)),
            Err(SelectionError::LimitExceeded)
        );
    }
}
