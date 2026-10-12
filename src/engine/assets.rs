//! Host-side image conversion. No assembly or hardware state is generated here.
use std::{fmt, io, path::PathBuf};

/// Mapping from PNG colors to the four Game Boy pixel indices.
#[derive(Debug, Clone, Copy, Default)]
pub enum PngColorMapping {
    /// Require an exact RGB match; reject unknown colors with their pixel coordinates.
    #[default]
    Exact,
    /// Choose the smallest squared RGB distance. Ties choose the lowest index.
    Nearest,
}

/// Tile serialization order in the generated 2bpp bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TileOrder {
    /// Left-to-right tiles, then the next eight-pixel row.
    #[default]
    RowMajor,
    /// Top/bottom tile pairs, left-to-right, then the next sixteen-pixel row.
    /// Height must be a multiple of 16; pairs align with 8×16 sprite tile numbering.
    Sprite8x16,
}

/// PNG conversion settings. Palette positions are pixel indices, not display shades.
/// Default: white/light gray/dark gray/black, exact mapping, transparent pixels at
/// index zero, row-major tiles. Semi-transparent pixels are rejected (no blending).
#[derive(Debug, Clone)]
pub struct PngOptions {
    colors: [[u8; 3]; 4],
    mapping: PngColorMapping,
    transparent: Option<u8>,
    order: TileOrder,
}
impl Default for PngOptions {
    fn default() -> Self {
        Self::new([[255; 3], [170; 3], [85; 3], [0; 3]])
    }
}
impl PngOptions {
    /// Explicit RGB colors for indices 0..=3. Duplicate colors choose the first index.
    pub fn new(colors: [[u8; 3]; 4]) -> Self {
        Self {
            colors,
            mapping: PngColorMapping::Exact,
            transparent: Some(0),
            order: TileOrder::RowMajor,
        }
    }
    pub fn mapping(mut self, mapping: PngColorMapping) -> Self {
        self.mapping = mapping;
        self
    }
    /// Map fully transparent pixels to an index, or reject them with `None`.
    /// Panics if the supplied index is greater than three.
    pub fn transparent_index(mut self, index: Option<u8>) -> Self {
        assert!(
            index.is_none_or(|index| index < 4),
            "transparent pixel index must be 0..=3"
        );
        self.transparent = index;
        self
    }
    pub fn tile_order(mut self, order: TileOrder) -> Self {
        self.order = order;
        self
    }
    fn index(&self, rgba: [u8; 4], x: u32, y: u32) -> Result<u8, AssetError> {
        if rgba[3] == 0 {
            return self.transparent.ok_or(AssetError::Pixel { x, y, rgba });
        }
        if rgba[3] != 255 {
            return Err(AssetError::Pixel { x, y, rgba });
        }
        let rgb = [rgba[0], rgba[1], rgba[2]];
        if let Some(index) = self.colors.iter().position(|color| *color == rgb) {
            return Ok(index as u8);
        }
        match self.mapping {
            PngColorMapping::Exact => Err(AssetError::Pixel { x, y, rgba }),
            PngColorMapping::Nearest => Ok(self
                .colors
                .iter()
                .enumerate()
                .min_by_key(|(_, color)| {
                    color
                        .iter()
                        .zip(rgb)
                        .map(|(&a, b)| {
                            let delta = i32::from(a) - i32::from(b);
                            delta * delta
                        })
                        .sum::<i32>()
                })
                .unwrap()
                .0 as u8),
        }
    }
}

/// Asset failures are returned before modifying a tile manager or writing any files.
#[derive(Debug)]
pub enum AssetError {
    Io {
        path: PathBuf,
        error: io::Error,
    },
    Png(png::DecodingError),
    /// Binary tiles must be nonempty and divisible into sixteen-byte tiles.
    Invalid2bppLength(usize),
    /// PNG dimensions must fit the selected tile grid without padding/cropping.
    Dimensions {
        width: u32,
        height: u32,
        order: TileOrder,
    },
    /// More than 64 MiB of decoded samples, or a size not representable on this host.
    ImageTooLarge,
    /// Only static PNGs are accepted, so animation is never silently discarded.
    AnimatedPng,
    /// Unknown palette color, transparency disabled, or alpha other than 0/255.
    Pixel {
        x: u32,
        y: u32,
        rgba: [u8; 4],
    },
    UnsupportedFormat,
}
impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, error } => write!(f, "{}: {error}", path.display()),
            Self::Png(error) => write!(f, "PNG: {error}"),
            Self::Invalid2bppLength(len) => write!(
                f,
                "2bpp data has {len} bytes; expected a nonempty multiple of 16"
            ),
            Self::Dimensions {
                width,
                height,
                order,
            } => write!(
                f,
                "PNG {width}×{height} does not fit the {order:?} tile grid"
            ),
            Self::ImageTooLarge => write!(f, "PNG exceeds the 64 MiB decoded-image limit"),
            Self::AnimatedPng => write!(f, "animated PNGs are not supported"),
            Self::Pixel { x, y, rgba } => write!(
                f,
                "PNG pixel ({x}, {y}) {rgba:?} cannot map to the selected palette/transparency policy"
            ),
            Self::UnsupportedFormat => write!(f, "unsupported normalized PNG sample format"),
        }
    }
}
impl std::error::Error for AssetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { error, .. } => Some(error),
            Self::Png(error) => Some(error),
            _ => None,
        }
    }
}
impl From<png::DecodingError> for AssetError {
    fn from(error: png::DecodingError) -> Self {
        Self::Png(error)
    }
}
pub(crate) fn read_file(path: &std::path::Path) -> Result<Vec<u8>, AssetError> {
    std::fs::read(path).map_err(|error| AssetError::Io {
        path: path.to_path_buf(),
        error,
    })
}

/// Decode a static PNG into Game Boy tiles: for each tile row, low plane then high
/// plane, with the leftmost pixel in bit 7. Supports indexed, gray, RGB and alpha
/// PNGs, including interlaced images. Packed samples expand to 8 bits; 16-bit samples
/// use their high byte. Palette matching uses those bytes without gamma correction.
/// Dimensions must be multiples of eight (height sixteen for Sprite8x16).
/// Produces owned bytes, never reads/writes an external file. It does not deduplicate
/// tiles: order and tile count are stable. PNG CRCs and the complete stream are checked.
pub fn png_to_2bpp(input: &[u8], options: &PngOptions) -> Result<Vec<u8>, AssetError> {
    const LIMIT: usize = 64 * 1024 * 1024;
    let mut decoder =
        png::Decoder::new_with_limits(io::Cursor::new(input), png::Limits { bytes: LIMIT });
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;
    if reader.info().animation_control.is_some() {
        return Err(AssetError::AnimatedPng);
    }
    let (width, height) = (reader.info().width, reader.info().height);
    let row_height = if options.order == TileOrder::Sprite8x16 {
        16
    } else {
        8
    };
    if width == 0 || height == 0 || width % 8 != 0 || height % row_height != 0 {
        return Err(AssetError::Dimensions {
            width,
            height,
            order: options.order,
        });
    }
    let size = reader
        .output_buffer_size()
        .filter(|&size| size <= LIMIT)
        .ok_or(AssetError::ImageTooLarge)?;
    let mut buffer = vec![0; size];
    let info = reader.next_frame(&mut buffer)?;
    reader.finish()?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err(AssetError::UnsupportedFormat);
    }
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err(AssetError::UnsupportedFormat),
    };
    let mut indices = Vec::with_capacity(size / channels);
    for (i, pixel) in buffer[..info.buffer_size()]
        .chunks_exact(channels)
        .enumerate()
    {
        let rgba = match channels {
            1 => [pixel[0], pixel[0], pixel[0], 255],
            2 => [pixel[0], pixel[0], pixel[0], pixel[1]],
            3 => [pixel[0], pixel[1], pixel[2], 255],
            _ => [pixel[0], pixel[1], pixel[2], pixel[3]],
        };
        indices.push(options.index(
            rgba,
            (i % width as usize) as u32,
            (i / width as usize) as u32,
        )?);
    }
    let mut output = Vec::with_capacity(indices.len() / 4);
    for block_y in (0..height).step_by(row_height as usize) {
        for tile_x in (0..width).step_by(8) {
            for tile_y in (block_y..block_y + row_height).step_by(8) {
                for y in tile_y..tile_y + 8 {
                    let (mut low, mut high) = (0, 0);
                    for x in tile_x..tile_x + 8 {
                        let index = indices[y as usize * width as usize + x as usize];
                        low = (low << 1) | (index & 1);
                        high = (high << 1) | (index >> 1);
                    }
                    output.extend([low, high]);
                }
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) fn encode(
        width: u32,
        height: u32,
        color: png::ColorType,
        depth: png::BitDepth,
        pixels: &[u8],
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(color);
        encoder.set_depth(depth);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(pixels).unwrap();
        writer.finish().unwrap();
        bytes
    }
    #[test]
    fn bitplanes_and_interlaced_rgb_are_in_pixel_order() {
        let actual = png_to_2bpp(
            include_bytes!("../../tests/fixtures/png/adam7-rgb.png"),
            &PngOptions::default(),
        )
        .unwrap();
        assert_eq!(
            actual,
            [
                0x55, 0x33, 0xaa, 0x66, 0x55, 0xcc, 0xaa, 0x99, 0x55, 0x33, 0xaa, 0x66, 0x55, 0xcc,
                0xaa, 0x99
            ]
        );
        let actual = png_to_2bpp(
            include_bytes!("../../tests/fixtures/png/gray2.png"),
            &PngOptions::default(),
        )
        .unwrap();
        assert_eq!(
            actual,
            [
                0xaa, 0xcc, 0x55, 0x99, 0xaa, 0x33, 0x55, 0x66, 0xaa, 0xcc, 0x55, 0x99, 0xaa, 0x33,
                0x55, 0x66
            ]
        );
    }
    #[test]
    fn row_major_and_sprite_order_match_the_four_quadrants() {
        let pixels: Vec<_> = (0..16)
            .flat_map(|y| (0..16).map(move |x| 255 - 85 * ((y / 8) * 2 + x / 8)))
            .collect();
        let image = encode(
            16,
            16,
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            &pixels,
        );
        let tiles = [
            vec![0; 16],
            [255, 0].repeat(8),
            [0, 255].repeat(8),
            vec![255; 16],
        ];
        assert_eq!(
            png_to_2bpp(&image, &PngOptions::default()).unwrap(),
            tiles.concat()
        );
        assert_eq!(
            png_to_2bpp(
                &image,
                &PngOptions::default().tile_order(TileOrder::Sprite8x16)
            )
            .unwrap(),
            [
                tiles[0].clone(),
                tiles[2].clone(),
                tiles[1].clone(),
                tiles[3].clone()
            ]
            .concat()
        );
    }
    #[test]
    fn indexed_transparency_uses_pixel_colors_not_palette_positions() {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 8, 8);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Two);
        encoder.set_palette(vec![0, 0, 0, 255, 255, 255, 170, 170, 170, 1, 2, 3]);
        encoder.set_trns(vec![255, 255, 255, 0]);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0x1b, 0x1b].repeat(8)).unwrap();
        writer.finish().unwrap();
        // Input palette indices 0,1,2,3 => output indices 3,0,1,0.
        assert_eq!(
            png_to_2bpp(&bytes, &PngOptions::default()).unwrap(),
            [0xaa, 0x88].repeat(8)
        );
        assert!(matches!(
            png_to_2bpp(&bytes, &PngOptions::default().transparent_index(None)),
            Err(AssetError::Pixel { x: 3, y: 0, .. })
        ));
    }
    #[test]
    fn alpha_custom_palette_and_nearest_mapping_are_explicit() {
        let colors = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [0, 0, 0]];
        let mut pixels = [255, 0, 0, 255].repeat(64);
        pixels[4..8].copy_from_slice(&[42, 99, 12, 0]);
        let image = encode(8, 8, png::ColorType::Rgba, png::BitDepth::Eight, &pixels);
        let options = PngOptions::new(colors).transparent_index(Some(3));
        let output = png_to_2bpp(&image, &options).unwrap();
        assert_eq!(&output[..2], &[0x40, 0x40]);
        pixels[4..8].copy_from_slice(&[250, 0, 0, 255]);
        let image = encode(8, 8, png::ColorType::Rgba, png::BitDepth::Eight, &pixels);
        assert!(matches!(
            png_to_2bpp(&image, &options),
            Err(AssetError::Pixel { x: 1, y: 0, .. })
        ));
        assert_eq!(
            png_to_2bpp(&image, &options.mapping(PngColorMapping::Nearest)).unwrap(),
            [0; 16]
        );
        let options = PngOptions::new([[0, 0, 0], [2, 0, 0], [255; 3], [100; 3]])
            .mapping(PngColorMapping::Nearest);
        let image = encode(
            8,
            8,
            png::ColorType::Rgb,
            png::BitDepth::Eight,
            &[1, 0, 0].repeat(64),
        );
        assert_eq!(png_to_2bpp(&image, &options).unwrap(), [0; 16]); // distance tie => index zero
        let image = encode(
            8,
            8,
            png::ColorType::GrayscaleAlpha,
            png::BitDepth::Eight,
            &[0, 128].repeat(64),
        );
        assert!(matches!(
            png_to_2bpp(&image, &options),
            Err(AssetError::Pixel {
                rgba: [0, 0, 0, 128],
                ..
            })
        ));
    }
    #[test]
    fn sixteen_bit_and_one_bit_samples_are_normalized() {
        let image = encode(
            8,
            8,
            png::ColorType::Grayscale,
            png::BitDepth::Sixteen,
            &[0xff, 0xff, 0xaa, 0xaa, 0x55, 0x55, 0, 0].repeat(16),
        );
        assert_eq!(
            png_to_2bpp(&image, &PngOptions::default()).unwrap(),
            [0x55, 0x33].repeat(8)
        );
        let image = encode(
            8,
            8,
            png::ColorType::Grayscale,
            png::BitDepth::One,
            &[0x55; 8],
        );
        assert_eq!(
            png_to_2bpp(&image, &PngOptions::default()).unwrap(),
            [0xaa, 0xaa].repeat(8)
        );
    }
    #[test]
    fn decoded_size_is_checked_before_allocating_the_frame() {
        let mut bytes = Vec::new();
        let encoder = png::Encoder::new(&mut bytes, 16384, 8192);
        let mut writer = encoder.write_header().unwrap();
        writer.write_chunk(png::chunk::IDAT, &[]).unwrap();
        writer.write_chunk(png::chunk::IEND, &[]).unwrap();
        drop(writer);
        assert!(matches!(
            png_to_2bpp(&bytes, &PngOptions::default()),
            Err(AssetError::ImageTooLarge)
        ));
    }
    #[test]
    fn bad_geometry_corruption_truncation_and_animation_are_errors() {
        let image = encode(
            9,
            8,
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            &[0; 72],
        );
        assert!(matches!(
            png_to_2bpp(&image, &PngOptions::default()),
            Err(AssetError::Dimensions { .. })
        ));
        let image = encode(
            8,
            8,
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            &[0; 64],
        );
        assert!(matches!(
            png_to_2bpp(
                &image,
                &PngOptions::default().tile_order(TileOrder::Sprite8x16)
            ),
            Err(AssetError::Dimensions { .. })
        ));
        for end in [0, 8, 32, image.len() - 12] {
            assert!(png_to_2bpp(&image[..end], &PngOptions::default()).is_err());
        }
        let mut corrupt = image.clone();
        corrupt[29] ^= 1;
        assert!(png_to_2bpp(&corrupt, &PngOptions::default()).is_err());
        let mut corrupt = image.clone();
        *corrupt.last_mut().unwrap() ^= 1;
        assert!(png_to_2bpp(&corrupt, &PngOptions::default()).is_err());
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 8, 8);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_animated(2, 0).unwrap();
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0; 64]).unwrap();
        writer.write_image_data(&[255; 64]).unwrap();
        writer.finish().unwrap();
        assert!(matches!(
            png_to_2bpp(&bytes, &PngOptions::default()),
            Err(AssetError::AnimatedPng)
        ));
    }
}

#[cfg(test)]
mod example_asset_tests {
    use super::*;
    #[test]
    fn fosdem_png_matches_the_committed_binary_tiles() {
        let palette = PngOptions::new([[255; 3], [207; 3], [104; 3], [0, 0, 1]])
            .tile_order(TileOrder::Sprite8x16);
        assert_eq!(
            png_to_2bpp(
                include_bytes!("../../examples/fosdem/char-dx.png"),
                &palette
            )
            .unwrap(),
            include_bytes!("../../examples/fosdem/char-dx.2bpp")
        );
    }
}
