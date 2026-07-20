use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{sync::Arc, time::Duration};

use dashmap::DashMap;
use gpui::*;
use parking_lot::RwLock;
use smol::channel::{self, Receiver, Sender};

use crate::animation::{AnimationPriority, Event};
use crate::interpolate::State;

pub mod color;
pub mod general;
pub mod position;

pub trait Transition: Send + Sync + 'static {
    fn run(&self, start: std::time::Instant, duration: std::time::Duration) -> f32 {
        let t = (start.elapsed().as_secs_f32() / duration.as_secs_f32()).min(1.0);
        self.calculate(t)
    }

    fn calculate(&self, t: f32) -> f32;
}

pub trait IntoArcTransition<T: Transition + 'static> {
    fn into_arc(self) -> Arc<T>;
}

impl<T: Transition + 'static> IntoArcTransition<T> for T {
    fn into_arc(self) -> Arc<T> {
        Arc::new(self)
    }
}

impl<T: Transition + 'static> IntoArcTransition<T> for Arc<T> {
    fn into_arc(self) -> Arc<T> {
        self
    }
}

pub(crate) struct PersistentContext {
    event: Event,
    style: StyleRefinement,
    duration: Duration,
    transition: Arc<dyn Transition>,
    priority: AnimationPriority,
}

pub(crate) struct ActiveAnimation {
    event: Event,
    duration: Duration,
    origin_duration: Duration,
    transition: Arc<dyn Transition>,
    ver: usize,
    persistent: bool,
}

pub(crate) struct TransitionRegistry {
    initialized: AtomicBool,
    rem_size: RwLock<Pixels>,
    states: DashMap<ElementId, State<StyleRefinement>>,
    active_animations: DashMap<ElementId, ActiveAnimation>,
    saved_contexts: DashMap<ElementId, PersistentContext>,
    wakeup_tx: Sender<()>,
    wakeup_rx: Receiver<()>,
}

pub(crate) static TRANSITION_REGISTRY: LazyLock<TransitionRegistry> = LazyLock::new(|| {
    let (tx, rx) = channel::unbounded();

    TransitionRegistry {
        initialized: AtomicBool::new(false),
        rem_size: RwLock::new(Pixels::from(16.)),
        // PERF 3: 预分配容量 + 固定 4 分片，减少锁竞争和 rehash
        states: DashMap::with_capacity_and_shard_amount(64, 4),
        active_animations: DashMap::with_capacity_and_shard_amount(32, 4),
        saved_contexts: DashMap::with_capacity_and_shard_amount(16, 4),
        wakeup_tx: tx,
        wakeup_rx: rx,
    }
});

impl TransitionRegistry {
    pub fn init(window: &mut Window, cx: &mut App) {
        let new_rem = window.rem_size();
        if *TRANSITION_REGISTRY.rem_size.read() != new_rem {
            *TRANSITION_REGISTRY.rem_size.write() = new_rem;
        }

        if !TRANSITION_REGISTRY.initialized.swap(true, Ordering::SeqCst) {
            cx.spawn(Self::animation_tick).detach();
        }
    }

    pub fn rem_size() -> Pixels {
        *TRANSITION_REGISTRY.rem_size.read()
    }

    pub fn save_persistent_context(
        id: &ElementId,
        style: &StyleRefinement,
        duration: Duration,
        transition: Arc<dyn Transition>,
        priority: AnimationPriority,
    ) {
        if let Some(active_anim) = TRANSITION_REGISTRY.active_animations.get(id)
            && active_anim.persistent
        {
            TRANSITION_REGISTRY.saved_contexts.insert(
                id.clone(),
                PersistentContext {
                    event: active_anim.event.clone(),
                    style: style.clone(),
                    duration,
                    transition,
                    priority,
                },
            );
        }
    }

    pub fn remove_persistent_context(id: &ElementId, event: Event) {
        TRANSITION_REGISTRY
            .saved_contexts
            .remove_if(id, |_, ctx| ctx.event.eq(&event));
    }

    pub fn background_animated_task(
        id: ElementId,
        event: Event,
        duration: Duration,
        origin_duration: Duration,
        transition: Arc<dyn Transition>,
        ver: usize,
        persistent: bool,
    ) {
        TRANSITION_REGISTRY.active_animations.insert(
            id,
            ActiveAnimation {
                event,
                duration,
                origin_duration,
                transition,
                ver,
                persistent,
            },
        );

        if TRANSITION_REGISTRY.wakeup_tx.try_send(()).is_err() {
            #[cfg(debug_assertions)]
            eprintln!("[gpui-animation] wakeup channel closed, animation tick may be dead");
        }
    }

    pub async fn animation_tick(cx: &mut AsyncApp) {
        let frame_duration = Duration::from_secs_f32(1. / 120.);
        let registry = &*TRANSITION_REGISTRY;

        let mut ids: Vec<ElementId> = Vec::with_capacity(32);
        let mut completed: Vec<(ElementId, Duration, Arc<dyn Transition>)> = Vec::with_capacity(8);

        loop {
            ids.clear();
            completed.clear();

            ids.extend(registry.active_animations.iter().map(|e| e.key().clone()));

            let mut changed = false;

            for id in &ids {
                let Some(active) = registry.active_animations.get(id) else {
                    continue;
                };
                let Some(mut state) = registry.states.get_mut(id) else {
                    drop(active);
                    registry.active_animations.remove(id);
                    continue;
                };

                changed = true;

                let done = state.animated(
                    active.ver,
                    active.duration,
                    &active.transition,
                    active.persistent,
                );

                if done {
                    if active.event.ne(&Event::NONE) {
                        state.priority = AnimationPriority::Lowest;
                        completed.push((
                            id.clone(),
                            active.origin_duration,
                            active.transition.clone(),
                        ));
                    }
                    drop(active);
                    drop(state);
                    registry.active_animations.remove(id);
                }
            }

            let still_animating = !registry.active_animations.is_empty();
            if changed && still_animating {
                cx.update(|cx| cx.refresh_windows()).ok();
            }

            for (id, origin_duration, transition) in &completed {
                if let Some((_, ctx)) = registry.saved_contexts.remove(id) {
                    if let Some(mut state) = registry.states.get_mut(id) {
                        state.priority = ctx.priority;
                        state.to = ctx.style;
                        let (ver, dt) = state.pre_animated(ctx.duration);
                        Self::background_animated_task(
                            id.clone(),
                            ctx.event.clone(),
                            dt,
                            dt,
                            ctx.transition.clone(),
                            ver,
                            true,
                        );
                    }
                } else if let Some(mut state) = registry.states.get_mut(id) {
                    state.priority = AnimationPriority::Lowest;
                    // Set the new target before pre_animated so that
                    // is_reversing (which compares `to == cur`) is evaluated
                    // against the correct target, matching the persistent
                    // branch above.
                    state.to = state.origin.clone();
                    let (ver, dt) = state.pre_animated(*origin_duration);
                    Self::background_animated_task(
                        id.clone(),
                        Event::NONE,
                        dt,
                        dt,
                        transition.clone(),
                        ver,
                        false,
                    );
                }
            }

            if changed && !still_animating {
                cx.update(|cx| cx.refresh_windows()).ok();
            }

            if registry.active_animations.is_empty() {
                registry.wakeup_rx.recv().await.ok();
            } else {
                smol::Timer::after(frame_duration).await;
            }
        }
    }

    #[inline]
    pub fn with_state_default<R>(
        id: ElementId,
        default: &StyleRefinement,
        f: impl FnOnce(&mut State<StyleRefinement>) -> R,
    ) -> R {
        let mut state = TRANSITION_REGISTRY
            .states
            .entry(id)
            .or_insert_with(|| State::new(default.clone()));

        f(&mut *state)
    }

    #[inline]
    pub fn state_mut(
        id: ElementId,
    ) -> Option<dashmap::mapref::one::RefMut<'static, ElementId, State<StyleRefinement>>> {
        if !TRANSITION_REGISTRY
            .initialized
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return None;
        }

        TRANSITION_REGISTRY.states.get_mut(&id)
    }

    /// Remove the stored animation state for an element.
    /// Call this when an element is about to be removed from the view tree
    /// (e.g. on click before navigating away) so that stale hover/click
    /// animation state does not persist and reappear when the element is
    /// re-rendered later.
    pub fn reset_state(id: &ElementId) {
        TRANSITION_REGISTRY.states.remove(id);
        TRANSITION_REGISTRY.active_animations.remove(id);
        TRANSITION_REGISTRY.saved_contexts.remove(id);
    }

    /// Remove all stored animation states, active animations, and saved
    /// contexts. Call this when tearing down an entire view (e.g. on window
    /// close or full re-mount) to avoid carrying over stale transition state
    /// from unrelated elements.
    pub fn reset_all_states() {
        TRANSITION_REGISTRY.states.clear();
        TRANSITION_REGISTRY.active_animations.clear();
        TRANSITION_REGISTRY.saved_contexts.clear();
        let _ = TRANSITION_REGISTRY.wakeup_tx.try_send(());
    }
}
