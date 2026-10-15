//! The About page of the File backstage: version and build details, the license and
//! third-party notices, links, the settings and log folders, a Copy button for bug reports,
//! and the Microsoft trademark notice.
//!
//! The build details are gathered by the app (layer 7) and passed in as an [`AppInfo`], so
//! this crate stays testable with fake values.

use egui::{CollapsingHeader, Frame, Grid, Margin, RichText, ScrollArea, Stroke, Ui};
use notecraft_engine::cmd::{ids, Value};

use crate::ribbon::Clicked;
use crate::start;
use crate::themes::{mix, ThemeTokens};

/// What NoteCraft is, in one line.
pub const DESCRIPTION: &str =
    "A clean-room, open-source note-taking app compatible with the OneNote workflow.";

/// Reverse-DNS app ID.
pub const APP_ID: &str = "io.github.queball1999.notecraft";

/// The copyright line.
pub const COPYRIGHT: &str = "Copyright (c) 2026 The NoteCraft Contributors";

/// The license summary.
pub const LICENSE_LINE: &str =
    "Dual-licensed under the MIT License or the Apache License, Version 2.0, at your option.";

/// The trademark notice, always visible on the page.
pub const TRADEMARK_NOTICE: &str = "Microsoft, OneNote, Office, and Microsoft 365 are trademarks of the Microsoft group of companies. NoteCraft is an independent open-source project. It is not affiliated with, sponsored by, or endorsed by Microsoft Corporation. These names are used only to describe the workflow NoteCraft is compatible with. NoteCraft contains no Microsoft code, artwork, icons, fonts, or templates.";

/// Where the source code lives.
pub const SOURCE_URL: &str = "https://github.com/Queball1999/notecraft";
/// Where to report a problem.
pub const ISSUES_URL: &str = "https://github.com/Queball1999/notecraft/issues";
/// Where the releases are.
pub const RELEASES_URL: &str = "https://github.com/Queball1999/notecraft/releases";

const THIRD_PARTY_SENTENCE: &str = "NoteCraft is built with open-source Rust crates, each under its own license. They are listed in the source repository (Cargo.lock).";

const MIT_TEXT: &str = include_str!("../../../LICENSE-MIT");
const APACHE_TEXT: &str = include_str!("../../../LICENSE-APACHE");
const ATTRIBUTION_TEXT: &str = include_str!("../../../ATTRIBUTION.md");

/// Widest the page text gets, in points, so lines stay readable in a wide window.
const PAGE_WIDTH: f32 = 760.0;

/// Build and system details the app gathers at startup. Every text field falls back to
/// "unknown" when empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInfo {
    /// The app version, for example `0.1.0`.
    pub version: String,
    /// The short git commit the build came from.
    pub commit: String,
    /// The UTC build date, `YYYY-MM-DD`.
    pub build_date: String,
    /// `debug` or `release`.
    pub profile: String,
    /// The Rust target triple, for example `x86_64-pc-windows-msvc`.
    pub target: String,
    /// The operating system and CPU architecture the app runs on, for example `windows x86_64`.
    pub os: String,
    /// The settings folder in use; `None` when settings are kept in memory.
    pub settings_dir: Option<String>,
    /// The log folder in use; `None` when file logging is off.
    pub log_dir: Option<String>,
}

impl Default for AppInfo {
    /// Placeholder details: the crate's own version and "unknown" for the rest.
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            commit: "unknown".to_owned(),
            build_date: "unknown".to_owned(),
            profile: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
            .to_owned(),
            target: "unknown".to_owned(),
            os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            settings_dir: None,
            log_dir: None,
        }
    }
}

fn or_unknown(text: &str) -> &str {
    if text.trim().is_empty() {
        "unknown"
    } else {
        text.trim()
    }
}

impl AppInfo {
    /// The plain-text block the Copy button puts on the clipboard, for bug reports.
    pub fn plain_text(&self) -> String {
        format!(
            "NoteCraft {}\nCommit: {}\nBuilt: {}\nProfile: {}\nTarget: {}\nOS: {}\n",
            or_unknown(&self.version),
            or_unknown(&self.commit),
            or_unknown(&self.build_date),
            or_unknown(&self.profile),
            or_unknown(&self.target),
            or_unknown(&self.os),
        )
    }
}

fn heading(ui: &mut Ui, text: &str, t: &ThemeTokens) {
    ui.add_space(14.0);
    ui.label(RichText::new(text).size(18.0).color(t.text));
    ui.add_space(4.0);
}

fn body(ui: &mut Ui, text: &str, t: &ThemeTokens) {
    ui.label(RichText::new(text).color(t.text));
}

fn row(ui: &mut Ui, key: &str, value: &str, t: &ThemeTokens) {
    ui.label(RichText::new(key).color(t.text_muted));
    ui.label(RichText::new(value).color(t.text));
    ui.end_row();
}

fn link(ui: &mut Ui, label: &str, url: &str, t: &ThemeTokens) {
    ui.hyperlink_to(RichText::new(label).color(t.accent).underline(), url);
}

fn notice(ui: &mut Ui, title: &str, text: &str, t: &ThemeTokens) {
    CollapsingHeader::new(RichText::new(title).color(t.text))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new(text).monospace().size(12.0).color(t.text));
        });
}

/// Draw the About page inside the backstage's content area. Pushes `file.copy_about_info`
/// when Copy is clicked.
pub(crate) fn show(ui: &mut Ui, t: &ThemeTokens, info: &AppInfo, out: &mut Vec<Clicked>) {
    let output = ScrollArea::vertical()
        .id_salt("about_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_max_width(PAGE_WIDTH);
            ui.label(RichText::new("About NoteCraft").size(28.0).color(t.text));
            ui.add_space(8.0);
            body(ui, DESCRIPTION, t);

            ui.add_space(12.0);
            Frame::new()
                .fill(mix(t.canvas_bg, t.text, 0.06))
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(t.radius)
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    body(ui, TRADEMARK_NOTICE, t);
                });

            heading(ui, "Version", t);
            Grid::new("about_info")
                .num_columns(2)
                .spacing([24.0, 6.0])
                .show(ui, |ui| {
                    row(ui, "Version", or_unknown(&info.version), t);
                    row(ui, "Commit", or_unknown(&info.commit), t);
                    row(ui, "Build date", or_unknown(&info.build_date), t);
                    row(ui, "Build profile", or_unknown(&info.profile), t);
                    row(ui, "Target", or_unknown(&info.target), t);
                    row(ui, "System", or_unknown(&info.os), t);
                    row(ui, "App ID", APP_ID, t);
                    row(ui, "Copyright", COPYRIGHT, t);
                });
            ui.add_space(8.0);
            if ui.add(start::framed("Copy", 14.0, t)).clicked() {
                out.push(Clicked {
                    command: ids::FILE_COPY_ABOUT_INFO,
                    args: Value::Null,
                });
            }

            heading(ui, "Folders", t);
            Grid::new("about_paths")
                .num_columns(2)
                .spacing([24.0, 6.0])
                .show(ui, |ui| {
                    row(
                        ui,
                        "Settings folder",
                        info.settings_dir
                            .as_deref()
                            .unwrap_or("none (settings are kept in memory)"),
                        t,
                    );
                    row(
                        ui,
                        "Log folder",
                        info.log_dir
                            .as_deref()
                            .unwrap_or("none (file logging is off)"),
                        t,
                    );
                });

            heading(ui, "License", t);
            body(ui, LICENSE_LINE, t);
            notice(ui, "MIT License", MIT_TEXT, t);
            notice(ui, "Apache License, Version 2.0", APACHE_TEXT, t);

            heading(ui, "Third-party notices", t);
            body(ui, THIRD_PARTY_SENTENCE, t);
            notice(ui, "Bundled assets and adapted code", ATTRIBUTION_TEXT, t);

            heading(ui, "Links", t);
            ui.horizontal_wrapped(|ui| {
                link(ui, "Source code", SOURCE_URL, t);
                link(ui, "Report an issue", ISSUES_URL, t);
                link(ui, "Releases", RELEASES_URL, t);
            });
            ui.add_space(16.0);
        });
    crate::a11y::name_scroll_bars(ui.ctx(), output.id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppShell;
    use egui_kittest::kittest::Queryable;

    fn fake() -> AppInfo {
        AppInfo {
            version: "1.2.3".to_owned(),
            commit: "abc123def456".to_owned(),
            build_date: "2026-10-15".to_owned(),
            profile: "release".to_owned(),
            target: "x86_64-pc-windows-msvc".to_owned(),
            os: "windows x86_64".to_owned(),
            settings_dir: None,
            log_dir: None,
        }
    }

    #[test]
    fn plain_text_has_every_field() {
        let text = fake().plain_text();
        for needle in [
            "NoteCraft 1.2.3",
            "Commit: abc123def456",
            "Built: 2026-10-15",
            "Profile: release",
            "Target: x86_64-pc-windows-msvc",
            "OS: windows x86_64",
        ] {
            assert!(text.contains(needle), "{needle} missing from {text}");
        }
    }

    #[test]
    fn empty_fields_read_unknown() {
        let info = AppInfo {
            version: String::new(),
            commit: "  ".to_owned(),
            build_date: String::new(),
            profile: String::new(),
            target: String::new(),
            os: String::new(),
            settings_dir: None,
            log_dir: None,
        };
        let text = info.plain_text();
        assert!(text.contains("NoteCraft unknown"));
        assert!(text.contains("Commit: unknown"));
        assert!(text.contains("Built: unknown"));
        assert!(text.contains("Profile: unknown"));
        assert!(text.contains("Target: unknown"));
        assert!(text.contains("OS: unknown"));
    }

    #[test]
    fn default_info_is_usable() {
        let text = AppInfo::default().plain_text();
        assert!(text.starts_with(&format!("NoteCraft {}", env!("CARGO_PKG_VERSION"))));
        assert!(text.contains("Commit: unknown"));
    }

    const WINDOW: egui::Vec2 = egui::vec2(1280.0, 900.0);

    fn harness(shell: AppShell) -> egui_kittest::Harness<'static, AppShell> {
        egui_kittest::Harness::builder()
            .with_size(WINDOW)
            .build_ui_state(
                |ui, shell: &mut AppShell| {
                    shell.logic(ui.ctx());
                    shell.ui(ui);
                },
                shell,
            )
    }

    /// A harness with the About page open.
    fn about_page() -> egui_kittest::Harness<'static, AppShell> {
        let mut shell = AppShell::new().with_about(AppInfo {
            settings_dir: Some("C:\\Fake\\Settings".to_owned()),
            log_dir: Some("C:\\Fake\\Settings\\logs".to_owned()),
            ..fake()
        });
        assert!(shell.dispatch("file.about", serde_json::Value::Null));
        let mut harness = harness(shell);
        harness.run();
        harness
    }

    #[test]
    fn file_about_is_ready_and_opens_the_page() {
        use notecraft_engine::cmd::Status;
        let harness = about_page();
        let spec = harness.state().registry().get("file.about");
        assert_eq!(spec.map(|spec| spec.status), Some(Status::Ready));
        assert!(harness.state().ribbon().backstage_open());
        assert!(
            harness.state().toasts().is_empty(),
            "no Coming Soon toast for About"
        );
        let _ = harness.get_by_label("About NoteCraft");
        assert!(harness.query_by_label("Coming Soon").is_none());
    }

    #[test]
    fn page_shows_version_notice_and_license() {
        let harness = about_page();
        for text in [
            "1.2.3",
            "abc123def456",
            "2026-10-15",
            "release",
            "x86_64-pc-windows-msvc",
            APP_ID,
            COPYRIGHT,
            DESCRIPTION,
            LICENSE_LINE,
            TRADEMARK_NOTICE,
            "C:\\Fake\\Settings",
            "C:\\Fake\\Settings\\logs",
        ] {
            let _ = harness.get_by_label(text);
        }
        for link in ["Source code", "Report an issue", "Releases"] {
            let _ = harness.get_by_label(link);
        }
        let _ = harness.get_by_label("MIT License");
        let _ = harness.get_by_label("Apache License, Version 2.0");
    }

    #[test]
    fn license_sections_open() {
        let mut harness = about_page();
        harness.get_by_label("MIT License").click();
        harness.run();
        assert!(
            harness
                .query_all_by_label_contains("Permission is hereby granted")
                .count()
                > 0
        );
    }

    #[test]
    fn copy_button_puts_the_details_on_the_clipboard() {
        let mut harness = about_page();
        harness.get_by_label("Copy").click();
        let mut copied = Vec::new();
        for _ in 0..4 {
            harness.step();
            copied.extend(
                harness.output().platform_output.commands.iter().filter_map(
                    |command| match command {
                        egui::OutputCommand::CopyText(text) => Some(text.clone()),
                        _ => None,
                    },
                ),
            );
        }
        assert_eq!(copied, [fake().plain_text()]);
    }

    #[test]
    fn about_controls_are_named_and_pressable() {
        use egui::accesskit::{Action, Role};
        use egui_kittest::kittest::{By, NodeT};
        const ROLES: [Role; 4] = [Role::Button, Role::Link, Role::ScrollBar, Role::Unknown];
        let harness = about_page();
        let bad: Vec<String> = harness
            .query_all(By::new().predicate(|node| ROLES.contains(&node.role())))
            .filter_map(|node| {
                let ak = node.accesskit_node();
                let named = ak.label().is_some_and(|l| !l.trim().is_empty());
                let pressable = ak.role() == Role::ScrollBar
                    || ak.data().supports_action(Action::Click)
                    || ak.data().is_disabled();
                (!named || !pressable)
                    .then(|| format!("{:?} {:?}", ak.role(), ak.label().unwrap_or_default()))
            })
            .collect();
        assert_eq!(bad, Vec::<String>::new());
    }

    #[test]
    fn fixed_texts_are_plain_ascii() {
        for text in [
            DESCRIPTION,
            COPYRIGHT,
            LICENSE_LINE,
            TRADEMARK_NOTICE,
            THIRD_PARTY_SENTENCE,
            MIT_TEXT,
            APACHE_TEXT,
            ATTRIBUTION_TEXT,
        ] {
            assert!(text.is_ascii());
        }
        assert!(TRADEMARK_NOTICE.contains("not affiliated with, sponsored by, or endorsed by"));
    }
}
