//! Settings category fade transition.
//!
//! Encapsulates the cross-fade state machine used when the user switches
//! between settings categories (Appearance / Terminal / Theme / Shortcuts /
//! SSH). The body content fades out, the category is swapped, and the new
//! content fades back in. View and update sites only delegate to this type;
//! all animation state lives here.
//!
//! State machine (each tick of `iced::Animation<bool>`):
//!
//! ```text
//!   Idle ──request_switch(target, animations_enabled=true)──▶ FadingOut(target)
//!   FadingOut(pending) ──tick after fade-out completes──▶ FadingIn (and
//!       returns `Some(pending)` so the caller swaps the live category)
//!   FadingIn ──tick after fade-in completes──▶ Idle
//! ```
//!
//! When `animations_enabled == false`, `request_switch` short-circuits and
//! tells the caller to swap immediately without starting a transition.

use crate::gui::settings::SettingsCategory;
use iced::Animation;
use iced::time::Instant;
use std::time::Duration;

/// One half of the cross-fade (out + in).
const FADE_DURATION: Duration = Duration::from_millis(120);

/// Current phase of the fade state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// No transition in flight.
    Idle,
    /// Body is fading to opaque; once complete the category swaps to `pending`.
    FadingOut,
    /// Body is fading back to transparent after the swap.
    FadingIn,
}

/// State for the settings category fade transition. Single instance lives on
/// `App`; view/update sites call into it instead of touching `Animation`.
pub struct CategoryTransition {
    /// Category to swap to once the fade-out phase finishes. Latest click wins
    /// while we are still fading out.
    pending: Option<SettingsCategory>,
    phase: Phase,
    /// `true` = overlay opaque (content hidden), `false` = overlay transparent
    /// (content fully visible).
    anim: Animation<bool>,
}

impl CategoryTransition {
    pub fn new() -> Self {
        Self {
            pending: None,
            phase: Phase::Idle,
            anim: Animation::new(false)
                .duration(FADE_DURATION)
                .easing(iced::animation::Easing::EaseOut),
        }
    }

    /// Request a category switch.
    ///
    /// * Returns `Some(target)` when the caller should update its
    ///   `settings_category` immediately (animations disabled, target equals
    ///   current, or we are already mid-transition into the same target).
    /// * Returns `None` when a fade-out has been started; the caller should
    ///   leave `settings_category` untouched and wait for `tick` to hand back
    ///   the swap.
    pub fn request_switch(
        &mut self,
        target: SettingsCategory,
        current: SettingsCategory,
        animations_enabled: bool,
        now: Instant,
    ) -> Option<SettingsCategory> {
        if !animations_enabled {
            // Reset any in-flight transition so we do not leave a stale overlay.
            self.pending = None;
            self.phase = Phase::Idle;
            self.anim.go_mut(false, now - FADE_DURATION);
            return Some(target);
        }

        if target == current && self.phase == Phase::Idle {
            return Some(target);
        }

        match self.phase {
            Phase::Idle => {
                // No-op if we are already on the requested category.
                if target == current {
                    return Some(target);
                }
                self.pending = Some(target);
                self.phase = Phase::FadingOut;
                self.anim.go_mut(true, now);
                None
            }
            Phase::FadingOut => {
                // A later click during the fade-out just updates the target.
                self.pending = Some(target);
                None
            }
            Phase::FadingIn => {
                // Clicked again while content was fading back in: restart from
                // a fade-out toward the new target.
                self.pending = Some(target);
                self.phase = Phase::FadingOut;
                self.anim.go_mut(true, now);
                None
            }
        }
    }

    /// Drive the state machine. Call once per `AnimationTick`.
    ///
    /// Returns `Some(category)` exactly on the tick where the fade-out has
    /// completed: the caller should set `settings_category = category` and the
    /// fade-in is started internally.
    pub fn tick(&mut self, now: Instant) -> Option<SettingsCategory> {
        match self.phase {
            Phase::Idle => None,
            Phase::FadingOut => {
                // Fade-out is finished when the animation has settled at the
                // opaque end (value == true and not animating).
                if !self.anim.is_animating(now) && self.anim.value() {
                    let target = self.pending.take();
                    self.phase = Phase::FadingIn;
                    self.anim.go_mut(false, now);
                    target
                } else {
                    None
                }
            }
            Phase::FadingIn => {
                if !self.anim.is_animating(now) && !self.anim.value() {
                    self.phase = Phase::Idle;
                }
                None
            }
        }
    }

    /// Overlay alpha in `[0.0, 1.0]`, or `None` when no transition is active.
    pub fn overlay_alpha(&self, now: Instant) -> Option<f32> {
        if self.phase == Phase::Idle {
            return None;
        }
        let alpha = self.anim.interpolate(0.0, 1.0, now).clamp(0.0, 1.0);
        Some(alpha)
    }

    /// Whether a redraw should be scheduled for the next frame.
    pub fn is_animating(&self, now: Instant) -> bool {
        self.anim.is_animating(now) || self.phase != Phase::Idle
    }
}

impl Default for CategoryTransition {
    fn default() -> Self {
        Self::new()
    }
}
