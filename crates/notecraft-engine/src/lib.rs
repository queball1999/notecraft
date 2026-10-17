//! NoteCraft L5: session, command registry, undo and redo, clipboard, export.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![forbid(unsafe_code)]

pub mod cmd;
pub mod ink;
#[cfg(not(target_arch = "wasm32"))]
pub mod io;
pub mod mock;
pub mod open;
pub mod recent;
pub mod session;
pub mod undo;
pub mod view;

pub use ink::{DrawTool, EraserMode, EraserSize, InkState, PenStyle};
pub use notecraft_storage::RecoveryEntry;
pub use session::{
    ConvertOutcome, PageNotice, PageSource, ReadPurpose, ReadRequest, RecoveryItem, SaveOutcome,
    SaveState, Selection, Session, TextCursor,
};
pub use view::PageStyle;

/// Layer number, read by `cargo xtask layers`.
pub const LAYER: u8 = 5;
