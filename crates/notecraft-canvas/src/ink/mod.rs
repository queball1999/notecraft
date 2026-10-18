//! The ink engine: capture and smoothing of strokes, variable-width tessellation, a spatial
//! index, and the per-page stroke layer. Nothing here touches a UI type; the front end turns
//! pointer events into [`notecraft_core::PointerSample`]s and meshes into its own vertices.

pub mod capture;
pub mod erase;
pub mod filter;
pub mod index;
pub mod layer;
pub mod tessellate;
pub mod texture;

pub use capture::{velocity_pressures, CaptureParams, StrokeBuilder};
pub use filter::{OneEuro, OneEuroParams};
pub use index::GridIndex;
pub use layer::{stroke_distance, InkLayer, MAX_STROKES};
pub use tessellate::{tessellate, width_at, Edge, StrokeMesh, TessParams};
