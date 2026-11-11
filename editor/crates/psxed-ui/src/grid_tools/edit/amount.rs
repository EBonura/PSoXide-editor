//! The amount typed during a gesture: digits, a minus, a decimal point.
//!
//! The number is in clicks (256 grid units). A half click is 128, a quarter
//! 64, so a decimal point is kept as far as the 64 lattice allows and the
//! result is rounded to it.

use psxed_project::grid::units::snap_to;
use psxed_project::grid::{CLICK_UNITS, FINE_UNITS};

/// The characters typed so far.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Amount {
    text: String,
}

impl Amount {
    /// Whether nothing has been typed.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The text as typed, for the hint.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Append a digit.
    pub fn digit(&mut self, digit: u8) {
        if self.text.len() < 8 {
            self.text.push(char::from(b'0' + digit.min(9)));
        }
    }

    /// A minus sign: only at the start, and typing it again flips it off.
    pub fn minus(&mut self) {
        if let Some(rest) = self.text.strip_prefix('-') {
            self.text = rest.to_string();
        } else {
            self.text.insert(0, '-');
        }
    }

    /// A decimal point: once.
    pub fn point(&mut self) {
        if !self.text.contains('.') && self.text.len() < 8 {
            if self.text.is_empty() || self.text == "-" {
                self.text.push('0');
            }
            self.text.push('.');
        }
    }

    /// Remove the last character.
    pub fn backspace(&mut self) {
        self.text.pop();
    }

    /// Forget it all.
    pub fn clear(&mut self) {
        self.text.clear();
    }

    /// The amount in grid units on the 64 lattice, `None` while the text is
    /// not a number yet ("", "-", "3.").
    pub fn units(&self) -> Option<i32> {
        let value: f32 = self.text.parse().ok()?;
        Some(snap_to(
            (value * CLICK_UNITS as f32).round() as i32,
            FINE_UNITS,
        ))
    }

    /// The amount as a whole number of steps (pushing a wall counts sectors,
    /// not clicks).
    pub fn steps(&self) -> Option<i32> {
        let value: f32 = self.text.parse().ok()?;
        Some(value.round() as i32)
    }
}

/// An amount of grid units in clicks, for words: `+2`, `-0.5`, `+0.25`.
pub(super) fn clicks_text(units: i32) -> String {
    let clicks = units as f32 / CLICK_UNITS as f32;
    let mut text = format!("{clicks:+.2}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_numbers_become_units_on_the_lattice() {
        let mut a = Amount::default();
        assert_eq!(a.units(), None);
        a.minus();
        assert_eq!(a.units(), None);
        a.digit(3);
        assert_eq!(a.units(), Some(-768));
        a.backspace();
        a.backspace();
        assert!(a.is_empty());
        a.point();
        a.digit(5);
        assert_eq!(a.text(), "0.5");
        assert_eq!(a.units(), Some(128));
        a.digit(1);
        // 0.51 clicks is 130.6 units: the nearest multiple of 64.
        assert_eq!(a.units(), Some(128));
    }

    #[test]
    fn clicks_read_back_without_trailing_zeros() {
        assert_eq!(clicks_text(512), "+2");
        assert_eq!(clicks_text(-128), "-0.5");
        assert_eq!(clicks_text(64), "+0.25");
    }
}
