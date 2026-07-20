pub mod animation;
pub mod transition;

mod transform;

pub(crate) mod interpolate;

/// Reset all animation state associated with the given element ID.
///
/// Call this before navigating away from a view that uses `transition_on_hover`
/// so that stale hover state does not reappear when the element is re-rendered.
///
/// ```ignore
/// .on_mouse_down(MouseButton::Left, move |_e, w, cx| {
///     gpui_animation::reset_transition(&ElementId::Name("my-row".into()));
///     on_click(w, cx);
/// })
/// ```
pub fn reset_transition(id: &gpui::ElementId) {
    transition::TransitionRegistry::reset_state(id);
}

/// Reset all animation states across every element.
///
/// Call this when tearing down an entire view (e.g. on window close or full
/// re-mount) to avoid carrying over stale transition state from unrelated
/// elements.
///
/// ```ignore
/// .on_mouse_down(MouseButton::Left, move |_e, w, cx| {
///     gpui_animation::reset_all_transitions();
///     on_click(w, cx);
/// })
/// ```
pub fn reset_all_transitions() {
    transition::TransitionRegistry::reset_all_states();
}
