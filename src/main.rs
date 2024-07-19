use image::{save_buffer_with_format, ColorType, ImageFormat};
use swash::scale::image::Image;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::shape::cluster::{Glyph, GlyphCluster};
use swash::shape::ShapeContext;
use swash::text::Script;
use swash::{zeno, Attributes, CacheKey, Charmap, FontRef, Metrics};

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
    let roboto = Font::from_file("Roboto-Regular.ttf", 0).unwrap();
    let noto_cjk = Font::from_file("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 0).unwrap();

    let mut roboto_glyphs = Vec::new();
    let mut roboto_glyph_images = Vec::new();

    let mut noto_cjk_glyphs = Vec::new();
    let mut noto_cjk_glyph_images = Vec::new();

    let font_size: f32 = 64.;

    // Shape context to turn chars into glyphs
    let mut shape_ctx = ShapeContext::new();

    // Scale context to turn glyphs into images
    let mut scale_ctx = ScaleContext::new();

    let roboto_metrics = {
        let mut roboto_shaper = shape_ctx
            .builder(roboto.as_ref())
            .script(Script::Latin)
            .build();

        roboto_shaper.add_str("a quick brown fox?   ");

        let metrics = roboto_shaper.metrics().scale(font_size);

        // Start shapin
        roboto_shaper.shape_with(|glyph_cluster: &GlyphCluster| {
            roboto_glyphs.extend_from_slice(glyph_cluster.glyphs);

            roboto_glyph_images.extend((glyph_cluster.glyphs.iter()).filter_map(|glyph| {
                // render each glyph individually
                render_glyph(&mut scale_ctx, &roboto.as_ref(), font_size, true, glyph)
            }));
        });

        metrics
    };

    let noto_cjk_metrics = {
        let mut noto_cjk_shaper = shape_ctx
            .builder(noto_cjk.as_ref())
            .script(Script::Hiragana)
            .build();

        noto_cjk_shaper.add_str("怠惰な犬の上にジャンプするのだーー！");

        let metrics = noto_cjk_shaper.metrics().scale(font_size);

        // Start shapin
        noto_cjk_shaper.shape_with(|glyph_cluster: &GlyphCluster| {
            noto_cjk_glyphs.extend_from_slice(glyph_cluster.glyphs);

            noto_cjk_glyph_images.extend((glyph_cluster.glyphs.iter()).filter_map(|glyph| {
                // render each glyph individually
                render_glyph(&mut scale_ctx, &noto_cjk.as_ref(), font_size, true, glyph)
            }));
        });

        metrics
    };

    // I somehow figured out that this is the correct formula to convert something
    // like `glyph.advance` to the correct number needed when drawing out glyphs.
    let em_to_px =
        |em: f32, metrics: &Metrics| (em * font_size / metrics.units_per_em as f32) as usize;

    // measure dimensions and baseline, and create image buffer
    let total_width: usize = {
        let roboto_cluster_width: usize = (roboto_glyphs.iter())
            .map(|g| em_to_px(g.advance, &roboto_metrics))
            .sum();

        let noto_cjk_cluster_width: usize = (noto_cjk_glyphs.iter())
            .map(|g| em_to_px(g.advance, &noto_cjk_metrics))
            .sum();

        roboto_cluster_width + noto_cjk_cluster_width
    };

    let baseline_height: usize = {
        let roboto_baseline_height = (roboto_glyph_images.iter())
            .map(|glyph_img| glyph_img.placement.height as usize)
            .max()
            .unwrap_or_default();

        let noto_cjk_baseline_height = (noto_cjk_glyph_images.iter())
            .map(|glyph_img| glyph_img.placement.height as usize)
            .max()
            .unwrap_or_default();

        roboto_baseline_height.max(noto_cjk_baseline_height)
    };

    let total_height: usize = {
        let roboto_total_height = (roboto_glyph_images.iter())
            .map(|glyph_img| {
                glyph_img.placement.height as usize
                    + baseline_height.saturating_add_signed(-glyph_img.placement.top as isize)
            })
            .max()
            .unwrap_or_default();

        let noto_cjk_total_height = (noto_cjk_glyph_images.iter())
            .map(|glyph_img| {
                glyph_img.placement.height as usize
                    + baseline_height.saturating_add_signed(-glyph_img.placement.top as isize)
            })
            .max()
            .unwrap_or_default();

        roboto_total_height.max(noto_cjk_total_height)
    };

    let mut img_buffer = vec![0; total_width * total_height];

    let mut glyph_advance: usize = 0;
    for (metrics, glyphs, glyph_images) in [
        (roboto_metrics, roboto_glyphs, roboto_glyph_images),
        (noto_cjk_metrics, noto_cjk_glyphs, noto_cjk_glyph_images),
    ] {
        // draw each glyph image in a loop
        for (glyph_idx, (glyph_img, glyph)) in glyph_images.iter().zip(glyphs.iter()).enumerate() {
            let width = glyph_img.placement.width as usize;
            let height = glyph_img.placement.height as usize;

            if height == 0 {
                println!("Glyph #{} has height 0 (probably a space)", glyph_idx);
            } else {
                let x_off = glyph_img.placement.left as isize;
                let y_off =
                    baseline_height.saturating_add_signed(-glyph_img.placement.top as isize);

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

            glyph_advance += em_to_px(glyph.advance, &metrics);
        }
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
