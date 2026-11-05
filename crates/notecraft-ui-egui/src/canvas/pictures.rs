//! Pictures on the canvas: loading the files a page's `![alt](link)` images point to, and a
//! small cache of the textures made from them.
//!
//! Decoding is bounded (see `notecraft_canvas::picture`). A picture that cannot be shown
//! (missing, damaged, too large, or not a picture) is cached as a failure, so the canvas draws
//! a placeholder and never tries again until the page is shown another time. The cache keeps
//! the pictures of one page; showing another page empties it. To keep a page with many large
//! pictures from freezing the window, at most one picture is decoded per frame; the rest show
//! "Loading picture" for a frame or two.

use std::collections::HashMap;

use egui::{ColorImage, TextureHandle, TextureId, TextureOptions};
use notecraft_canvas::math::draw as draw_math;
use notecraft_canvas::picture::{load_picture, PictureError};
use notecraft_core::Vec2;
use notecraft_engine::Session;
use notecraft_model::ObjectKind;

/// Most pictures kept for one page; further ones show a placeholder.
const MAX_ENTRIES: usize = 256;
/// Pictures decoded in one frame.
const DECODES_PER_FRAME: usize = 1;
/// Most formulas kept for one page; further ones are shown as source text.
const MAX_MATH_ENTRIES: usize = 512;
/// Formulas typeset in one frame; the rest show their source for a frame or two.
const TYPESETS_PER_FRAME: usize = 8;

/// What the canvas can draw for one picture link.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Shown {
    /// Decoded: the texture and the natural size in page units (one pixel is one unit).
    Ready {
        texture: TextureId,
        width: f32,
        height: f32,
    },
    /// Not decoded yet.
    Loading,
    /// Cannot be shown; the reason is a short plain-text line.
    Failed(String),
}

/// What the canvas can draw for one formula.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum MathShown {
    /// Typeset and drawn: the texture and its size in page units.
    Ready {
        texture: TextureId,
        width: f32,
        ascent: f32,
        descent: f32,
    },
    /// Not typeset yet.
    Loading,
    /// Cannot be shown; the canvas shows the source text instead.
    Failed,
}

/// What the canvas can draw for the live preview of the formula being edited.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum MathPreview {
    Ready {
        texture: TextureId,
        width: f32,
        ascent: f32,
        descent: f32,
    },
    /// Not typeset yet.
    Loading,
    /// The formula cannot be typeset; a short plain reason to show in its place.
    Error(String),
}

/// What a formula is typeset with: the source and how it is set. Floats are compared by bits.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct MathKey {
    pub src: String,
    pub display: bool,
    pub em_bits: u32,
    pub rgb: u32,
    /// Bitmap pixels per page unit, a whole number.
    pub scale: u32,
}

/// Where flow layout gets its pictures.
pub(super) trait PictureSource {
    fn picture(&mut self, link: &str) -> Shown;

    /// A formula, typeset and drawn.
    fn math(&mut self, _key: &MathKey) -> MathShown {
        MathShown::Failed
    }

    /// The formula being edited, typeset for the live preview. Only the latest source is
    /// kept, so typing never fills the cache.
    fn math_preview(&mut self, _key: &MathKey) -> MathPreview {
        // A source that cannot typeset shows no preview at all.
        MathPreview::Loading
    }

    /// A width in page units to show the `nth` picture with `link` at instead of its saved
    /// one: the width it is being dragged to.
    fn preview_width(&self, _link: &str, _nth: usize) -> Option<f32> {
        None
    }

    /// A page point to show the `nth` object of `kind` with `link` at instead of where it
    /// is: the spot it is being dragged to.
    fn preview_pos(&self, _kind: ObjectKind, _link: &str, _nth: usize) -> Option<Vec2> {
        None
    }
}

/// A source with no pictures, for layout tests: every picture is a placeholder.
#[cfg(test)]
pub(super) struct NoPictures;

#[cfg(test)]
impl PictureSource for NoPictures {
    fn picture(&mut self, _link: &str) -> Shown {
        Shown::Failed("pictures are off".to_owned())
    }
}

enum Entry {
    Ready {
        texture: TextureHandle,
        width: f32,
        height: f32,
    },
    Failed(PictureError),
}

enum MathEntry {
    Ready {
        texture: TextureHandle,
        width: f32,
        ascent: f32,
        descent: f32,
    },
    Failed,
}

enum PreviewEntry {
    Ready {
        texture: TextureHandle,
        width: f32,
        ascent: f32,
        descent: f32,
    },
    Failed(String),
}

impl PreviewEntry {
    fn shown(&self) -> MathPreview {
        match self {
            Self::Ready {
                texture,
                width,
                ascent,
                descent,
            } => MathPreview::Ready {
                texture: texture.id(),
                width: *width,
                ascent: *ascent,
                descent: *descent,
            },
            Self::Failed(why) => MathPreview::Error(why.clone()),
        }
    }
}

/// A resize drag in progress: the picture and the width it is being dragged to.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ResizeDrag {
    pub link: String,
    pub nth: usize,
    /// The width when the drag started, in page units.
    pub start: f32,
    /// How far the pointer moved since, in page units.
    pub dx: f32,
    pub dy: f32,
    /// The width shown now.
    pub width: f32,
}

/// A move drag in progress: the picture or file and how far it has been dragged.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct MoveDrag {
    pub kind: ObjectKind,
    pub link: String,
    pub nth: usize,
    /// Top-left corner when the drag started, in page units.
    pub start: Vec2,
    /// How far the pointer moved since, in page units.
    pub delta: Vec2,
}

impl MoveDrag {
    /// Where the object is shown now.
    pub fn pos(&self) -> Vec2 {
        Vec2::new(
            (self.start.x + self.delta.x).max(0.0),
            (self.start.y + self.delta.y).max(0.0),
        )
    }
}

/// The pictures of the page shown now.
#[derive(Default)]
pub(crate) struct PictureCache {
    page: Option<String>,
    entries: HashMap<String, Entry>,
    decoded_this_frame: usize,
    /// Formulas typeset for the page shown now.
    math: HashMap<MathKey, MathEntry>,
    typeset_this_frame: usize,
    /// The preview of the formula being edited: only the latest, by source.
    preview: Option<(MathKey, PreviewEntry)>,
    /// The picture with resize handles: its link and which one with that link.
    pub(super) selected: Option<(String, usize)>,
    pub(super) drag: Option<ResizeDrag>,
    /// A picture or file being dragged to a new place.
    pub(super) moving: Option<MoveDrag>,
}

impl std::fmt::Debug for PictureCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PictureCache")
            .field("page", &self.page)
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl PictureCache {
    /// Start a frame for `page`: pictures of another page are dropped.
    pub(super) fn begin_frame(&mut self, page: Option<&str>) {
        if self.page.as_deref() != page {
            self.page = page.map(str::to_owned);
            self.entries.clear();
            self.math.clear();
            self.preview = None;
            self.selected = None;
            self.drag = None;
            self.moving = None;
        }
        self.decoded_this_frame = 0;
        self.typeset_this_frame = 0;
    }

    /// How many pictures are decoded and how many could not be shown, for tests.
    #[cfg(test)]
    pub(super) fn counts(&self) -> (usize, usize) {
        let ready = self
            .entries
            .values()
            .filter(|e| matches!(e, Entry::Ready { .. }))
            .count();
        (ready, self.entries.len() - ready)
    }
}

/// A [`PictureSource`] that reads files through the session and keeps textures in a cache.
pub(super) struct PictureLoader<'a> {
    pub cache: &'a mut PictureCache,
    pub ctx: &'a egui::Context,
    pub session: &'a Session,
}

fn reason(err: &PictureError) -> String {
    match err {
        PictureError::Missing => "the file is missing".to_owned(),
        other => other.to_string(),
    }
}

impl PictureSource for PictureLoader<'_> {
    fn preview_pos(&self, kind: ObjectKind, link: &str, nth: usize) -> Option<Vec2> {
        let moving = self.cache.moving.as_ref()?;
        (moving.kind == kind && moving.link == link && moving.nth == nth).then(|| moving.pos())
    }

    fn preview_width(&self, link: &str, nth: usize) -> Option<f32> {
        let drag = self.cache.drag.as_ref()?;
        (drag.link == link && drag.nth == nth).then_some(drag.width)
    }

    fn math_preview(&mut self, key: &MathKey) -> MathPreview {
        if let Some((have, entry)) = &self.cache.preview {
            if have == key {
                return entry.shown();
            }
        }
        if self.cache.typeset_this_frame >= TYPESETS_PER_FRAME {
            self.ctx.request_repaint();
            return MathPreview::Loading;
        }
        self.cache.typeset_this_frame += 1;
        let entry = match draw_math(
            key.src.as_str(),
            key.display,
            f32::from_bits(key.em_bits),
            key.rgb,
            key.scale as f32,
        ) {
            Ok(image) => {
                let size = [image.texture_width as usize, image.texture_height as usize];
                let pixels = ColorImage::from_rgba_unmultiplied(size, &image.rgba);
                PreviewEntry::Ready {
                    texture: self.ctx.load_texture(
                        "equation-preview",
                        pixels,
                        TextureOptions::LINEAR,
                    ),
                    width: image.width,
                    ascent: image.ascent,
                    descent: image.descent,
                }
            }
            Err(err) => PreviewEntry::Failed(err.to_string()),
        };
        let shown = entry.shown();
        self.cache.preview = Some((key.clone(), entry));
        shown
    }

    fn math(&mut self, key: &MathKey) -> MathShown {
        if let Some(entry) = self.cache.math.get(key) {
            return match entry {
                MathEntry::Ready {
                    texture,
                    width,
                    ascent,
                    descent,
                } => MathShown::Ready {
                    texture: texture.id(),
                    width: *width,
                    ascent: *ascent,
                    descent: *descent,
                },
                MathEntry::Failed => MathShown::Failed,
            };
        }
        if self.cache.math.len() >= MAX_MATH_ENTRIES {
            return MathShown::Failed;
        }
        if self.cache.typeset_this_frame >= TYPESETS_PER_FRAME {
            self.ctx.request_repaint();
            return MathShown::Loading;
        }
        self.cache.typeset_this_frame += 1;
        let entry = match draw_math(
            key.src.as_str(),
            key.display,
            f32::from_bits(key.em_bits),
            key.rgb,
            key.scale as f32,
        ) {
            Ok(image) => {
                let size = [image.texture_width as usize, image.texture_height as usize];
                let pixels = ColorImage::from_rgba_unmultiplied(size, &image.rgba);
                MathEntry::Ready {
                    texture: self
                        .ctx
                        .load_texture("equation", pixels, TextureOptions::LINEAR),
                    width: image.width,
                    ascent: image.ascent,
                    descent: image.descent,
                }
            }
            Err(err) => {
                tracing::debug!("equation: {err}");
                MathEntry::Failed
            }
        };
        let shown = match &entry {
            MathEntry::Ready {
                texture,
                width,
                ascent,
                descent,
            } => MathShown::Ready {
                texture: texture.id(),
                width: *width,
                ascent: *ascent,
                descent: *descent,
            },
            MathEntry::Failed => MathShown::Failed,
        };
        self.cache.math.insert(key.clone(), entry);
        shown
    }

    fn picture(&mut self, link: &str) -> Shown {
        if let Some(entry) = self.cache.entries.get(link) {
            return match entry {
                Entry::Ready {
                    texture,
                    width,
                    height,
                } => Shown::Ready {
                    texture: texture.id(),
                    width: *width,
                    height: *height,
                },
                Entry::Failed(err) => Shown::Failed(reason(err)),
            };
        }
        if self.cache.entries.len() >= MAX_ENTRIES {
            return Shown::Failed("too many pictures on this page".to_owned());
        }
        let Some(path) = self.session.attachment_path(link) else {
            let err = PictureError::Missing;
            self.cache
                .entries
                .insert(link.to_owned(), Entry::Failed(err.clone()));
            return Shown::Failed("not a picture file in this notebook".to_owned());
        };
        if self.cache.decoded_this_frame >= DECODES_PER_FRAME {
            self.ctx.request_repaint();
            return Shown::Loading;
        }
        self.cache.decoded_this_frame += 1;
        let entry = match load_picture(&path) {
            Ok(picture) => {
                let size = [
                    picture.texture_width as usize,
                    picture.texture_height as usize,
                ];
                let image = ColorImage::from_rgba_unmultiplied(size, &picture.rgba);
                let name = format!("picture:{link}");
                Entry::Ready {
                    texture: self.ctx.load_texture(name, image, TextureOptions::LINEAR),
                    width: picture.width as f32,
                    height: picture.height as f32,
                }
            }
            Err(err) => {
                tracing::debug!("picture {link}: {err}");
                Entry::Failed(err)
            }
        };
        let shown = match &entry {
            Entry::Ready {
                texture,
                width,
                height,
            } => Shown::Ready {
                texture: texture.id(),
                width: *width,
                height: *height,
            },
            Entry::Failed(err) => Shown::Failed(reason(err)),
        };
        self.cache.entries.insert(link.to_owned(), entry);
        shown
    }
}
