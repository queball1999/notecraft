//! The GPU side of the textured pens: each texture's procedural image (built in
//! `notecraft_canvas::ink::texture`) is uploaded once, on first use, and meshes point at it.
//! Nothing is stored in a page or bundled with the app.

use std::collections::HashMap;

use egui::{ColorImage, Context, TextureHandle, TextureId, TextureOptions};
use notecraft_canvas::ink::texture;
use notecraft_core::InkTexture;

/// The uploaded images, kept in egui's memory for the whole run.
#[derive(Clone, Default)]
struct Uploaded(HashMap<InkTexture, TextureHandle>);

/// The id of `kind`'s image, uploading it the first time. `None` for a plain stroke.
pub(crate) fn texture_id(ctx: &Context, kind: InkTexture) -> Option<TextureId> {
    texture::spec(kind)?;
    let key = egui::Id::new("notecraft.ink.textures");
    if let Some(handle) = ctx.data(|data| {
        data.get_temp::<Uploaded>(key)
            .and_then(|uploaded| uploaded.0.get(&kind).map(TextureHandle::id))
    }) {
        return Some(handle);
    }
    let image = texture::image(kind)?;
    let color = ColorImage::from_rgba_unmultiplied([image.width, image.height], &image.rgba);
    let handle = ctx.load_texture(
        format!("notecraft.ink.{}", kind.id()),
        color,
        TextureOptions::LINEAR_REPEAT,
    );
    let id = handle.id();
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Uploaded>(key)
            .0
            .insert(kind, handle);
    });
    Some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_is_uploaded_once_and_a_plain_stroke_has_none() {
        let ctx = Context::default();
        assert_eq!(texture_id(&ctx, InkTexture::Plain), None);
        let a = texture_id(&ctx, InkTexture::Lava);
        assert!(a.is_some());
        assert_eq!(texture_id(&ctx, InkTexture::Lava), a);
        assert_ne!(texture_id(&ctx, InkTexture::Gold), a);
    }
}
