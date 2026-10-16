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
use notecraft_canvas::picture::{load_picture, PictureError};
use notecraft_engine::Session;

/// Most pictures kept for one page; further ones show a placeholder.
const MAX_ENTRIES: usize = 256;
/// Pictures decoded in one frame.
const DECODES_PER_FRAME: usize = 1;

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

/// Where flow layout gets its pictures.
pub(super) trait PictureSource {
    fn picture(&mut self, link: &str) -> Shown;
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

/// The pictures of the page shown now.
#[derive(Default)]
pub(crate) struct PictureCache {
    page: Option<String>,
    entries: HashMap<String, Entry>,
    decoded_this_frame: usize,
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
        }
        self.decoded_this_frame = 0;
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
