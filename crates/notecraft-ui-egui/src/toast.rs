//! Toasts: short messages near the bottom of the window that fade out on their own. The
//! main use is "Coming Soon: <label>" when a command is not implemented yet.

use std::time::Duration;

use notecraft_engine::cmd::{CommandError, Registry};

/// How long a toast stays up, in seconds.
pub const TOAST_SECONDS: f64 = 3.0;
/// At most this many toasts show at once; older ones are dropped first.
const MAX_TOASTS: usize = 4;
/// Gap between the toasts and the bottom of the window, in points (clears the status bar).
const BOTTOM_GAP: f32 = 40.0;

#[derive(Debug)]
struct Toast {
    text: String,
    /// When it was first drawn; its 3 seconds start then, not when it was pushed.
    shown_at: Option<f64>,
}

/// The toasts waiting to be shown or on screen.
#[derive(Debug, Default)]
pub struct Toasts {
    items: Vec<Toast>,
}

impl Toasts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Show `text`. Pushing a toast that is already up restarts its timer instead of
    /// adding a second copy.
    pub fn push(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.items.retain(|toast| toast.text != text);
        self.items.push(Toast {
            text,
            shown_at: None,
        });
        let excess = self.items.len().saturating_sub(MAX_TOASTS);
        self.items.drain(..excess);
    }

    /// The texts of the current toasts, oldest first.
    pub fn texts(&self) -> impl Iterator<Item = &str> {
        self.items.iter().map(|toast| toast.text.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Start the clock on new toasts and drop expired ones. Returns the seconds until the
    /// next one expires, or `None` when there are no toasts left.
    pub fn tick(&mut self, now: f64) -> Option<f64> {
        for toast in &mut self.items {
            toast.shown_at.get_or_insert(now);
        }
        self.items.retain(|toast| {
            toast
                .shown_at
                .is_some_and(|shown_at| now - shown_at < TOAST_SECONDS)
        });
        self.items
            .iter()
            .filter_map(|toast| toast.shown_at)
            .map(|shown_at| shown_at + TOAST_SECONDS - now)
            .reduce(f64::min)
    }

    /// Draw the toasts, bottom center, above everything else.
    pub fn show(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|input| input.time);
        let Some(wait) = self.tick(now) else {
            return;
        };
        ctx.request_repaint_after(Duration::try_from_secs_f64(wait.max(0.0)).unwrap_or_default());
        egui::Area::new(egui::Id::new("toasts"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -BOTTOM_GAP))
            .interactable(false)
            .show(ctx, |ui| {
                for toast in &self.items {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(&toast.text);
                    });
                }
            });
    }
}

/// The toast text for a command that did not run.
pub fn error_text(registry: &Registry, err: &CommandError) -> String {
    match err {
        CommandError::ComingSoon { label, .. } => format!("Coming Soon: {label}"),
        CommandError::NotFound { id } => format!("Unknown command: {id}"),
        CommandError::InvalidArgs { id, reason } => {
            let label = registry
                .get(id.as_str())
                .map_or(id.as_str(), |spec| spec.label);
            format!("{label}: {reason}")
        }
        CommandError::Internal { id, .. } => {
            let label = registry
                .get(id.as_str())
                .map_or(id.as_str(), |spec| spec.label);
            format!("{label} ran into a problem. Your notes were not changed.")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_lasts_three_seconds_from_first_show() {
        let mut toasts = Toasts::new();
        toasts.push("Coming Soon: Paste");
        assert_eq!(toasts.tick(10.0), Some(TOAST_SECONDS));
        assert_eq!(toasts.tick(12.5), Some(0.5));
        assert!(toasts.tick(13.0).is_none());
        assert!(toasts.is_empty());
    }

    #[test]
    fn pushing_the_same_text_restarts_it() {
        let mut toasts = Toasts::new();
        toasts.push("Coming Soon: Paste");
        toasts.tick(0.0);
        toasts.push("Coming Soon: Paste");
        assert_eq!(toasts.tick(2.0), Some(TOAST_SECONDS));
        assert_eq!(toasts.texts().count(), 1);
    }

    #[test]
    fn keeps_only_the_newest_toasts() {
        let mut toasts = Toasts::new();
        for n in 0..10 {
            toasts.push(format!("toast {n}"));
        }
        let texts: Vec<&str> = toasts.texts().collect();
        assert_eq!(texts, ["toast 6", "toast 7", "toast 8", "toast 9"]);
    }

    #[test]
    fn coming_soon_text_uses_the_label() {
        let registry = Registry::builtin();
        let err = CommandError::ComingSoon {
            id: notecraft_engine::cmd::ids::EDIT_PASTE,
            label: "Paste",
        };
        assert_eq!(error_text(&registry, &err), "Coming Soon: Paste");
    }
}
