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
pub mod find;
pub mod ink;
#[cfg(not(target_arch = "wasm32"))]
pub mod io;
#[cfg(not(target_arch = "wasm32"))]
pub mod link_index;
pub mod link_rename;
pub mod links;
pub mod mock;
pub mod open;
pub mod recent;
pub mod search;
pub mod session;
pub mod stamp;
#[cfg(not(target_arch = "wasm32"))]
pub mod tags_index;
pub mod undo;
pub mod view;
pub mod whats_new;

pub use find::{FindState, Match as FindMatch};
pub use ink::{
    default_pen_tools, DrawTool, EraserMode, EraserSize, InkState, PenStyle, PenTool, ToolKind,
    MAX_PEN_POINTS, MAX_PEN_TOOLS, MIN_PEN_POINTS,
};
pub use notecraft_core::InkTexture;
pub use notecraft_storage::{PageEncoding, RecoveryEntry, NOT_UTF8_NOTICE};
pub use search::{
    AttachmentInfo, HitKind, SearchContext, SearchHit, SearchResults, SearchRoot, SearchScope,
    SearchSort, SearchState,
};
pub use session::{
    AttachKind, Attached, ConvertOutcome, InsertError, LinkFixOffer, LinkUpdateReport, PageNotice,
    PageSource, ReadPurpose, ReadRequest, RecoveryItem, SaveOutcome, SaveState, Selection, Session,
    TextCursor, Utf8Outcome,
};
pub use view::PageStyle;

/// Layer number, read by `cargo xtask layers`.
pub const LAYER: u8 = 5;
