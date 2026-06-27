//! transform: translate via `position: relative` + `inset` offset.
//!
//! GPUI 0.2 does not expose a real compositing transform for `div`
//! (`TransformationMatrix` is only wired to SVG sprites). For the common
//! case of moving an element — modal slide-in, card enter/exit, list
//! reordering — a `relative` element with an animated `inset` offset is
//! visually identical to `translateX/Y`, does not disturb sibling layout,
//! and keeps hit-testing correct because GPUI folds the relative offset
//! into the element's hitbox.
//!
//! NOTE: the target element MUST be `.relative()` for the offset to apply.

use gpui::{AbsoluteLength, DefiniteLength, Length, Pixels};

use crate::interpolate::State;

#[inline]
fn px_to_length(p: Pixels) -> Length {
    Length::Definite(DefiniteLength::Absolute(AbsoluteLength::Pixels(p)))
}

impl State<gpui::StyleRefinement> {
    /// Horizontal translate. Positive moves right, negative moves left.
    /// Maps to `inset.left` on a `relative` element.
    pub fn translate_x(mut self, x: Pixels) -> Self {
        self.to.inset.left = Some(px_to_length(x));
        self
    }

    /// Vertical translate. Positive moves down, negative moves up.
    /// Maps to `inset.top` on a `relative` element.
    pub fn translate_y(mut self, y: Pixels) -> Self {
        self.to.inset.top = Some(px_to_length(y));
        self
    }

    /// Combined translate.
    pub fn translate(self, x: Pixels, y: Pixels) -> Self {
        self.translate_x(x).translate_y(y)
    }
}
