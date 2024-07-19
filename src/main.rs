use image::{save_buffer_with_format, ColorType, ImageFormat};
use swash::scale::image::Image;
use swash::scale::{Render, ScaleContext, Scaler, Source, StrikeWith};
use swash::shape::cluster::{Glyph, GlyphCluster};
use swash::shape::{Direction, ShapeContext};
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

fn render_glyph(scaler: &mut Scaler, glyph: &Glyph) -> Option<Image> {
    use zeno::{Format, Vector};

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
    .render(scaler, glyph.id)
}

fn main() {
    let roboto = Font::from_file("Roboto-Regular.ttf", 0).unwrap();
    let noto_cjk = Font::from_file("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 0).unwrap();
    let noto_arab =
        Font::from_file("/usr/share/fonts/noto/NotoNaskhArabic-Regular.ttf", 0).unwrap();

    let mut glyphs = Vec::new();
    let mut glyph_images = Vec::new();

    let font_size = 64.;
    let hint = false;

    // Shape context to turn chars into glyphs
    let mut shape_ctx = ShapeContext::new();

    // Scale context to turn glyphs into images
    let mut scale_ctx = ScaleContext::new();

    {
        let mut scaler = scale_ctx
            .builder(roboto.as_ref())
            .size(font_size)
            .hint(hint)
            .build();

        let mut roboto_shaper = shape_ctx
            .builder(roboto.as_ref())
            .script(Script::Latin)
            .size(font_size)
            .build();

        roboto_shaper.add_str("a quick brown fox?   ");

        // Start shapin
        roboto_shaper.shape_with(|glyph_cluster: &GlyphCluster| {
            glyphs.extend_from_slice(glyph_cluster.glyphs);

            glyph_images.extend((glyph_cluster.glyphs.iter()).filter_map(|glyph| {
                // render each glyph individually
                render_glyph(&mut scaler, glyph)
            }));
        });
    };

    {
        let mut scaler = scale_ctx
            .builder(noto_cjk.as_ref())
            .size(font_size)
            .hint(hint)
            .build();

        let mut noto_cjk_shaper = shape_ctx
            .builder(noto_cjk.as_ref())
            .script(Script::Hiragana)
            .size(font_size)
            .build();

        noto_cjk_shaper.add_str("怠惰な犬の上にジャンプするのだ！  ");

        // Start shapin
        noto_cjk_shaper.shape_with(|glyph_cluster: &GlyphCluster| {
            glyphs.extend_from_slice(glyph_cluster.glyphs);

            glyph_images.extend((glyph_cluster.glyphs.iter()).filter_map(|glyph| {
                // render each glyph individually
                render_glyph(&mut scaler, glyph)
            }));
        });
    };

    {
        let mut scaler = scale_ctx
            .builder(noto_arab.as_ref())
            .size(font_size)
            .hint(hint)
            .build();

        let mut arab_shaper = shape_ctx
            .builder(noto_arab.as_ref())
            .script(Script::Arabic)
            .direction(Direction::RightToLeft)
            .size(font_size)
            // .features(&[("dlig", 1)])
            .build();

        let arab_str = "لكن لا بد أن أوضح لك أن كل    ";
        arab_shaper.add_str(arab_str);
        println!("{} chars", arab_str.chars().count());

        // Start shapin
        let mut n_glyphs = 0;
        arab_shaper.shape_with(|glyph_cluster: &GlyphCluster| {
            n_glyphs += glyph_cluster.glyphs.len();
            println!("{} glyphs", glyph_cluster.glyphs.len());
            glyphs.extend_from_slice(glyph_cluster.glyphs);

            glyph_images.extend((glyph_cluster.glyphs.iter()).filter_map(|glyph| {
                // render each glyph individually
                render_glyph(&mut scaler, glyph)
            }));
        });

        println!("total glyphs: {n_glyphs}");
    };

    // measure dimensions and baseline, and create image buffer
    let total_width: usize = (glyphs.iter()).map(|g| g.advance).sum::<f32>() as usize;

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

    let mut img_buffer: Vec<[u8; 4]> = vec![[0, 0, 0, 0]; total_width * total_height];

    // draw each glyph image in a loop
    let mut col = 0;

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

                    let [r, g, b, a] = img_buffer[buffer_idx];
                    let v = glyph_img.data[glyph_idx];

                    img_buffer[buffer_idx] = [
                        v.saturating_add(r),
                        v.saturating_add(g),
                        v.saturating_add(b),
                        v.saturating_add(a),
                    ];

                    if col & 0b001 > 0 {
                        img_buffer[buffer_idx][0] = r;
                    }

                    if col & 0b010 > 0 {
                        img_buffer[buffer_idx][1] = g;
                    }

                    if col & 0b100 > 0 {
                        img_buffer[buffer_idx][2] = b;
                    }
                }
            }
        }

        glyph_advance += glyph.advance.round() as usize; // em_to_px(glyph.advance, &metrics);
        col = (col + 1) % 8;
    }

    save_buffer_with_format(
        "swash-text.png",
        elements_as_bytes(&img_buffer),
        total_width as u32,
        total_height as u32,
        ColorType::Rgba8,
        ImageFormat::Png,
    )
    .unwrap();
}

pub(crate) fn elements_as_bytes<T>(elements: &[T]) -> &'_ [u8] {
    // SAFETY: the length of the slice is always right
    unsafe {
        std::slice::from_raw_parts(
            elements.as_ptr() as *const u8,
            elements.len() * std::mem::size_of::<T>(),
        )
    }
}
