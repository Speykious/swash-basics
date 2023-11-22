use std::fs;

use image::{ColorType, ImageFormat, save_buffer_with_format};
use swash::scale::image::Image;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::shape::cluster::GlyphCluster;
use swash::shape::ShapeContext;
use swash::text::Script;
use swash::{zeno, Attributes, CacheKey, Charmap, FontRef, GlyphId};

pub struct Font {
    /// Full content of the font file
    data: Vec<u8>,
    /// Offset to the table directory
    offset: u32,
    /// Cache key
    key: CacheKey,
}

impl Font {
    pub fn from_file(path: &str, index: usize) -> Option<Self> {
        // Read the full font file
        let data = std::fs::read(path).ok()?;

        // Create a temporary font reference for the first font in the file.
        // This will do some basic validation, compute the necessary offset
        // and generate a fresh cache key for us.
        let font = FontRef::from_index(&data, index)?;
        let (offset, key) = (font.offset, font.key);

        // Return our struct with the original file data and copies of the
        // offset and key from the font reference
        Some(Self { data, offset, key })
    }

    // As a convenience, you may want to forward some methods.
    pub fn attributes(&self) -> Attributes {
        self.as_ref().attributes()
    }

    pub fn charmap(&self) -> Charmap {
        self.as_ref().charmap()
    }

    /// Create the transient font reference for accessing this crate's
    /// functionality.
    pub fn as_ref(&self) -> FontRef {
        // Note that you'll want to initialize the struct directly here as
        // using any of the FontRef constructors will generate a new key which,
        // while completely safe, will nullify the performance optimizations of
        // the caching mechanisms used in this crate.
        FontRef {
            data: &self.data,
            offset: self.offset,
            key: self.key,
        }
    }
}

fn render_glyph(
    context: &mut ScaleContext,
    font: &FontRef,
    size: f32,
    hint: bool,
    glyph_id: GlyphId,
    x: f32,
    y: f32,
) -> Option<Image> {
    use zeno::{Format, Vector};

    // Scale context to turn glyphs into images
    let mut scaler = context.builder(*font).size(size).hint(hint).build();

    // Compute the fractional offset-- you'll likely want to quantize this
    // in a real renderer
    let offset = Vector::new(x.fract(), y.fract());

    // Render glyph into image (subpixel format = alpha)
    // This will give us an image with only an alpha channel
    Render::new(&[
        Source::ColorOutline(0),
        Source::ColorBitmap(StrikeWith::BestFit),
        Source::Outline,
    ])
    .format(Format::Alpha)
    .offset(offset)
    .render(&mut scaler, glyph_id)
}

fn main() {
    // we're gonna put glyph images in that folder
    fs::create_dir_all("glyph_imgs").unwrap();

    let font = Font::from_file("Roboto-Regular.ttf", 0).unwrap();

    // Shape context to turn chars into glyphs
    let mut shape_ctx = ShapeContext::new();
    let mut shaper = shape_ctx
        .builder(font.as_ref())
        .script(Script::Latin)
        .build();

    // feed shaper with chars to get them as glyphs later
    shaper.add_str("A quick brown fox?");

    // Scale context to turn glyphs into images
    let mut scale_ctx = ScaleContext::new();

    // Start shapin
    let font_ref = font.as_ref();
    let mut glyph_idx = 0;
    shaper.shape_with(|glyph_cluster: &GlyphCluster| {
        for glyph in glyph_cluster.glyphs {
            // render each glyph individually
            let glyph_img = render_glyph(
                &mut scale_ctx,
                &font_ref,
                28.,
                true,
                glyph.id,
                glyph.x,
                glyph.y,
            )
            .unwrap();

            if glyph_img.placement.height == 0 {
                println!("Glyph #{} has height 0 (probably a space)", glyph_idx);
            } else {
                let width = glyph_img.placement.width;
                let height = glyph_img.placement.height;

                save_buffer_with_format(
                    format!("glyph_imgs/glyph-{}.png", glyph_idx),
                    &glyph_img.data,
                    width,
                    height,
                    ColorType::L8,
                    ImageFormat::Png,
                )
                .unwrap();
            }

            glyph_idx += 1;
        }
    });
}
