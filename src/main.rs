use image::{save_buffer_with_format, ColorType, ImageFormat};
use swash::scale::image::Image;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::shape::cluster::{Glyph, GlyphCluster};
use swash::shape::ShapeContext;
use swash::text::Script;
use swash::{zeno, Attributes, CacheKey, Charmap, FontRef};

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
    glyph: &Glyph,
) -> Option<Image> {
    use zeno::{Format, Vector};

    // Scale context to turn glyphs into images
    let mut scaler = context.builder(*font).size(size).hint(hint).build();

    // Compute the fractional offset-- you'll likely want to quantize this
    // in a real renderer
    let offset = Vector::new(glyph.x.fract(), glyph.y.fract());

    // Render glyph into image (subpixel format = alpha)
    // This will give us an image with only an alpha channel
    Render::new(&[
        Source::ColorOutline(0),
        Source::ColorBitmap(StrikeWith::BestFit),
        Source::Outline,
    ])
    .format(Format::Alpha)
    .offset(offset)
    .render(&mut scaler, glyph.id)
}

fn main() {
    let font = Font::from_file("Roboto-Regular.ttf", 0).unwrap();

    // Shape context to turn chars into glyphs
    let mut shape_ctx = ShapeContext::new();
    let mut shaper = shape_ctx
        .builder(font.as_ref())
        .script(Script::Latin)
        .build();

    // feed shaper with chars to get them as glyphs later
    shaper.add_str("A quick brown fox?   ");
    shaper.add_str("怠惰な犬の上にジャンプするのだ！");

    // Scale context to turn glyphs into images
    let mut scale_ctx = ScaleContext::new();
    let font_size: f32 = 64.;
    let font_metrics = shaper.metrics().scale(font_size);

    // I somehow figured out that this is the correct formula to convert something
    // like `glyph.advance` to the correct number needed when drawing out glyphs.
    let em_to_px = |em: f32| (em * font_size / font_metrics.units_per_em as f32) as usize;

    // Start shapin
    let font_ref = font.as_ref();
    let mut glyphs = Vec::new();
    let mut glyph_images = Vec::new();
    shaper.shape_with(|glyph_cluster: &GlyphCluster| {
        glyphs.extend_from_slice(glyph_cluster.glyphs);

        glyph_images.extend((glyph_cluster.glyphs.iter()).filter_map(|glyph| {
            // render each glyph individually
            render_glyph(&mut scale_ctx, &font_ref, font_size, true, glyph)
        }));
    });

    // measure dimensions and baseline, and create image buffer
    let total_width: usize = glyphs.iter().map(|glyph| em_to_px(glyph.advance)).sum();

    let baseline_height: usize = (glyph_images.iter())
        .map(|glyph_img| glyph_img.placement.height as usize)
        .max()
        .unwrap_or_default();

    let total_height: usize = (glyph_images.iter())
        .map(|glyph_img| {
            glyph_img.placement.height as usize
                + baseline_height.saturating_add_signed(-glyph_img.placement.top as isize)
        })
        .max()
        .unwrap_or_default();

    let mut img_buffer = vec![0; total_width * total_height];

    // draw each glyph image in a loop
    let mut glyph_advance: usize = 0;
    for (glyph_idx, (glyph_img, glyph)) in glyph_images.iter().zip(glyphs.iter()).enumerate() {
        let width = glyph_img.placement.width as usize;
        let height = glyph_img.placement.height as usize;

        if height == 0 {
            println!("Glyph #{} has height 0 (probably a space)", glyph_idx);
        } else {
            let x_off = glyph_img.placement.left as isize;
            let y_off = baseline_height.saturating_add_signed(-glyph_img.placement.top as isize);

            for y in 0..usize::min(height, total_height) {
                for x in 0..width {
                    let x_buf = x.saturating_add_signed(x_off) + glyph_advance;
                    let y_buf = y.saturating_add(y_off).min(total_height - 1);

                    let buffer_idx = y_buf * total_width + x_buf;
                    let glyph_idx = y * width + x;

                    let pixel: u8 = img_buffer[buffer_idx];
                    img_buffer[buffer_idx] = pixel.saturating_add(glyph_img.data[glyph_idx]);
                }
            }
        }

        glyph_advance += em_to_px(glyph.advance);
    }

    save_buffer_with_format(
        "swash-text.png",
        &img_buffer,
        total_width as u32,
        total_height as u32,
        ColorType::L8,
        ImageFormat::Png,
    )
    .unwrap();
}
