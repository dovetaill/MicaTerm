//! Platform clipboard payload selection and bounded image encoding.

use std::fs::File;
use std::io::{BufReader, Cursor, Seek, SeekFrom, Write};
#[cfg(any(target_os = "windows", test))]
use std::path::Path;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use anyhow::anyhow;
use anyhow::{Context, Result, bail};
use image::{DynamicImage, ImageFormat, ImageReader, Limits};

use crate::app::image_policy::{
    MAX_DECODED_IMAGE_BYTES, MAX_ENCODED_IMAGE_BYTES, MAX_IMAGE_PIXELS,
};

pub(crate) const CLIPBOARD_IMAGE_PREVIEW_MAX_WIDTH: u32 = 320;
pub(crate) const CLIPBOARD_IMAGE_PREVIEW_MAX_HEIGHT: u32 = 180;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClipboardPayload {
    Text(String),
    Image(ClipboardImageSource),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) enum ClipboardImageSource {
    Bitmap(Vec<u8>),
    Encoded(Vec<u8>),
    File(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClipboardImagePreview {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EncodedClipboardImage {
    pub png_bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub preview: ClipboardImagePreview,
}

pub(crate) fn select_clipboard_payload<F>(
    image: Option<ClipboardImageSource>,
    text_reader: F,
) -> Option<ClipboardPayload>
where
    F: FnOnce() -> Option<String>,
{
    image
        .map(ClipboardPayload::Image)
        .or_else(|| text_reader().map(ClipboardPayload::Text))
}

#[cfg(any(target_os = "windows", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowsClipboardImageKind {
    Png,
    Bitmap,
    FileList,
}

#[cfg(any(target_os = "windows", test))]
fn select_windows_clipboard_image_kind(
    png_available: bool,
    bitmap_available: bool,
    dib_available: bool,
    dibv5_available: bool,
    file_list_available: bool,
) -> Option<WindowsClipboardImageKind> {
    if png_available {
        Some(WindowsClipboardImageKind::Png)
    } else if bitmap_available || dib_available || dibv5_available {
        Some(WindowsClipboardImageKind::Bitmap)
    } else if file_list_available {
        Some(WindowsClipboardImageKind::FileList)
    } else {
        None
    }
}

#[cfg(any(target_os = "windows", test))]
fn validate_windows_registered_png_size(size: usize) -> Result<()> {
    if size == 0 {
        bail!("registered PNG clipboard payload is empty or has an unknown size");
    }
    validate_encoded_image_size(size)
}

#[cfg(any(target_os = "windows", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ValidatedWindowsBitmapMetadata {
    width: u32,
    height: u32,
    pixel_copy_bytes: u64,
}

#[cfg(any(target_os = "windows", test))]
fn validate_windows_bitmap_metadata(
    width: i32,
    height: i32,
    planes: u16,
    bits_per_pixel: u16,
) -> Result<ValidatedWindowsBitmapMetadata> {
    let width = u32::try_from(width).context("clipboard bitmap width must be positive")?;
    let height = u32::try_from(height).context("clipboard bitmap height must be positive")?;
    validate_image_dimensions(width, height)?;

    if planes != 1 {
        bail!("clipboard bitmap must have exactly one color plane");
    }
    if !matches!(bits_per_pixel, 1 | 4 | 8 | 16 | 24 | 32) {
        bail!("clipboard bitmap has an unsupported {bits_per_pixel}-bit pixel format");
    }

    let row_bits = u64::from(width)
        .checked_mul(u64::from(bits_per_pixel))
        .context("clipboard bitmap row size overflowed")?;
    let row_bytes = row_bits
        .checked_add(31)
        .and_then(|bits| bits.checked_div(32))
        .and_then(|dwords| dwords.checked_mul(4))
        .context("clipboard bitmap row allocation overflowed")?;
    let pixel_copy_bytes = row_bytes
        .checked_mul(u64::from(height))
        .context("clipboard bitmap pixel allocation overflowed")?;
    let decoded_bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .context("clipboard bitmap decoded allocation overflowed")?;
    if pixel_copy_bytes > MAX_DECODED_IMAGE_BYTES || decoded_bytes > MAX_DECODED_IMAGE_BYTES {
        bail!(
            "clipboard bitmap exceeds the {} MiB decoded limit",
            MAX_DECODED_IMAGE_BYTES / (1024 * 1024)
        );
    }

    Ok(ValidatedWindowsBitmapMetadata {
        width,
        height,
        pixel_copy_bytes,
    })
}

#[cfg(target_os = "windows")]
fn validate_clipboard_bitmap_before_copy() -> Result<ValidatedWindowsBitmapMetadata> {
    use clipboard_win::formats::CF_BITMAP;
    use windows::Win32::Graphics::Gdi::{BITMAP, GetObjectW, HBITMAP};
    use windows::Win32::System::DataExchange::GetClipboardData;

    let clipboard_handle = unsafe { GetClipboardData(CF_BITMAP) }
        .context("failed to access the clipboard bitmap handle")?;
    let mut bitmap = BITMAP::default();
    let expected_metadata_bytes = std::mem::size_of::<BITMAP>();
    let copied_metadata_bytes = unsafe {
        GetObjectW(
            HBITMAP(clipboard_handle.0),
            expected_metadata_bytes as i32,
            Some(std::ptr::addr_of_mut!(bitmap).cast()),
        )
    };
    if copied_metadata_bytes != expected_metadata_bytes as i32 {
        bail!("failed to read complete clipboard bitmap metadata");
    }

    validate_windows_bitmap_metadata(
        bitmap.bmWidth,
        bitmap.bmHeight,
        bitmap.bmPlanes,
        bitmap.bmBitsPixel,
    )
}

#[cfg(target_os = "windows")]
fn copy_windows_registered_png(format: u32) -> Result<Vec<u8>> {
    use std::slice;

    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::GetClipboardData;
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    let clipboard_handle = unsafe { GetClipboardData(format) }
        .context("failed to access the registered PNG clipboard payload")?;
    let global = HGLOBAL(clipboard_handle.0);
    let size = unsafe { GlobalSize(global) };
    validate_windows_registered_png_size(size)?;

    let data = unsafe { GlobalLock(global) };
    if data.is_null() {
        bail!("failed to lock the registered PNG clipboard payload");
    }
    let bytes = unsafe { slice::from_raw_parts(data.cast::<u8>(), size).to_vec() };
    // A false return can mean the lock count reached zero, so the wrapper's
    // Result cannot distinguish that normal case from an unlock failure.
    let _ = unsafe { GlobalUnlock(global) };
    Ok(bytes)
}

#[cfg(target_os = "windows")]
pub(crate) fn system_clipboard_image_source() -> Result<Option<ClipboardImageSource>> {
    use clipboard_win::formats::{Bitmap, CF_DIB, CF_DIBV5, FileList};
    use clipboard_win::{Clipboard, Format, Getter};

    let png_format = clipboard_win::register_format("PNG")
        .context("failed to register the Windows PNG clipboard format")?
        .get();
    let image_kind = select_windows_clipboard_image_kind(
        clipboard_win::is_format_avail(png_format),
        Bitmap.is_format_avail(),
        clipboard_win::is_format_avail(CF_DIB),
        clipboard_win::is_format_avail(CF_DIBV5),
        FileList.is_format_avail(),
    );
    if image_kind.is_none() {
        return Ok(None);
    }

    let _clipboard = Clipboard::new_attempts(10)
        .map_err(|err| anyhow!("failed to open the Windows clipboard: {err}"))?;

    match image_kind.expect("image kind was checked above") {
        WindowsClipboardImageKind::Png => Ok(Some(ClipboardImageSource::Encoded(
            copy_windows_registered_png(png_format)?,
        ))),
        WindowsClipboardImageKind::Bitmap => {
            // GetClipboardData(CF_BITMAP) asks Windows to synthesize a bitmap
            // when the producer supplied CF_DIB or CF_DIBV5 instead.
            let _metadata = validate_clipboard_bitmap_before_copy()?;
            let mut bitmap = Vec::new();
            Bitmap
                .read_clipboard(&mut bitmap)
                .map_err(|err| anyhow!("failed to read the clipboard bitmap: {err}"))?;
            Ok(Some(ClipboardImageSource::Bitmap(bitmap)))
        }
        WindowsClipboardImageKind::FileList => {
            let mut paths = Vec::<PathBuf>::new();
            FileList
                .read_clipboard(&mut paths)
                .map_err(|err| anyhow!("failed to read the clipboard file list: {err}"))?;
            Ok(match paths.as_slice() {
                [path] if is_supported_clipboard_image_path(path) => {
                    Some(ClipboardImageSource::File(path.clone()))
                }
                _ => None,
            })
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn system_clipboard_image_source() -> Result<Option<ClipboardImageSource>> {
    Ok(None)
}

#[cfg(any(target_os = "windows", test))]
pub(crate) fn is_supported_clipboard_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "bmp" | "gif" | "ico" | "jpeg" | "jpg" | "png" | "tif" | "tiff" | "webp"
            )
        })
}

pub(crate) fn encode_clipboard_image(
    source: ClipboardImageSource,
) -> Result<EncodedClipboardImage> {
    match source {
        ClipboardImageSource::Bitmap(bytes) | ClipboardImageSource::Encoded(bytes) => {
            encode_with_reader_factory(|| {
                ImageReader::new(Cursor::new(bytes.as_slice()))
                    .with_guessed_format()
                    .context("failed to inspect the clipboard image format")
            })
        }
        ClipboardImageSource::File(path) => encode_with_reader_factory(|| {
            let file = File::open(path.as_path()).context("failed to open the clipboard image")?;
            ImageReader::new(BufReader::new(file))
                .with_guessed_format()
                .context("failed to inspect the clipboard image format")
        }),
    }
}

fn encode_with_reader_factory<R, F>(mut reader_factory: F) -> Result<EncodedClipboardImage>
where
    R: std::io::BufRead + Seek,
    F: FnMut() -> Result<ImageReader<R>>,
{
    let (width, height) = reader_factory()?
        .into_dimensions()
        .context("failed to read clipboard image dimensions")?;
    validate_image_dimensions(width, height)?;

    let mut reader = reader_factory()?;
    reader.limits(image_decode_limits());
    let image = reader
        .decode()
        .context("failed to decode the clipboard image")?;
    encode_png(image, width, height)
}

fn image_decode_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(u32::try_from(MAX_IMAGE_PIXELS).unwrap_or(u32::MAX));
    limits.max_image_height = Some(u32::try_from(MAX_IMAGE_PIXELS).unwrap_or(u32::MAX));
    limits.max_alloc = Some(MAX_DECODED_IMAGE_BYTES);
    limits
}

pub(crate) fn validate_image_dimensions(width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 {
        bail!("clipboard image dimensions must be non-zero");
    }
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if pixels > MAX_IMAGE_PIXELS {
        bail!("clipboard image has {pixels} pixels, exceeding the {MAX_IMAGE_PIXELS}-pixel limit");
    }
    Ok(())
}

fn encode_png(image: DynamicImage, width: u32, height: u32) -> Result<EncodedClipboardImage> {
    let preview = create_clipboard_image_preview(&image);
    let mut writer = LimitedVecWriter::new(MAX_ENCODED_IMAGE_BYTES);
    let encode_result = image.write_to(&mut writer, ImageFormat::Png);
    if writer.exceeded_limit() {
        bail!(
            "clipboard image exceeds the {} MiB encoded limit",
            MAX_ENCODED_IMAGE_BYTES / (1024 * 1024)
        );
    }
    encode_result.context("failed to encode the clipboard image as PNG")?;
    let png_bytes = writer.into_inner();
    validate_encoded_image_size(png_bytes.len())?;
    Ok(EncodedClipboardImage {
        png_bytes,
        width,
        height,
        preview,
    })
}

fn create_clipboard_image_preview(image: &DynamicImage) -> ClipboardImagePreview {
    let thumbnail = if image.width() <= CLIPBOARD_IMAGE_PREVIEW_MAX_WIDTH
        && image.height() <= CLIPBOARD_IMAGE_PREVIEW_MAX_HEIGHT
    {
        image.to_rgba8()
    } else {
        image
            .thumbnail(
                CLIPBOARD_IMAGE_PREVIEW_MAX_WIDTH,
                CLIPBOARD_IMAGE_PREVIEW_MAX_HEIGHT,
            )
            .to_rgba8()
    };
    ClipboardImagePreview {
        width: thumbnail.width(),
        height: thumbnail.height(),
        rgba: thumbnail.into_raw(),
    }
}

pub(crate) fn validate_encoded_image_size(encoded_bytes: usize) -> Result<()> {
    if encoded_bytes > MAX_ENCODED_IMAGE_BYTES {
        bail!(
            "clipboard image exceeds the {} MiB encoded limit",
            MAX_ENCODED_IMAGE_BYTES / (1024 * 1024)
        );
    }
    Ok(())
}

struct LimitedVecWriter {
    cursor: Cursor<Vec<u8>>,
    limit: usize,
    exceeded_limit: bool,
}

impl LimitedVecWriter {
    fn new(limit: usize) -> Self {
        Self {
            cursor: Cursor::new(Vec::new()),
            limit,
            exceeded_limit: false,
        }
    }

    fn exceeded_limit(&self) -> bool {
        self.exceeded_limit
    }

    fn into_inner(self) -> Vec<u8> {
        self.cursor.into_inner()
    }
}

impl Write for LimitedVecWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let end = self.cursor.position().saturating_add(buffer.len() as u64);
        if end > self.limit as u64 {
            self.exceeded_limit = true;
            return Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "encoded clipboard image limit exceeded",
            ));
        }
        self.cursor.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.cursor.flush()
    }
}

impl Seek for LimitedVecWriter {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let next = self.cursor.seek(position)?;
        if next > self.limit as u64 {
            self.exceeded_limit = true;
            return Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "encoded clipboard image limit exceeded",
            ));
        }
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    #[test]
    fn image_payload_takes_precedence_over_text() {
        let image = ClipboardImageSource::Bitmap(vec![1, 2, 3]);
        let text_was_read = std::cell::Cell::new(false);
        assert_eq!(
            select_clipboard_payload(Some(image.clone()), || {
                text_was_read.set(true);
                Some("fallback".into())
            }),
            Some(ClipboardPayload::Image(image))
        );
        assert!(!text_was_read.get());
    }

    #[test]
    fn windows_image_selector_prefers_registered_png_then_convertible_bitmap() {
        assert_eq!(
            select_windows_clipboard_image_kind(true, true, true, true, true),
            Some(WindowsClipboardImageKind::Png)
        );
        assert_eq!(
            select_windows_clipboard_image_kind(false, false, true, false, true),
            Some(WindowsClipboardImageKind::Bitmap)
        );
        assert_eq!(
            select_windows_clipboard_image_kind(false, false, false, true, true),
            Some(WindowsClipboardImageKind::Bitmap)
        );
    }

    #[test]
    fn windows_image_selector_uses_one_file_only_when_no_image_format_exists() {
        assert_eq!(
            select_windows_clipboard_image_kind(false, false, false, false, true),
            Some(WindowsClipboardImageKind::FileList)
        );
        assert_eq!(
            select_windows_clipboard_image_kind(false, false, false, false, false),
            None
        );
    }

    #[test]
    fn text_payload_is_preserved_when_no_image_is_available() {
        assert_eq!(
            select_clipboard_payload(None, || Some("line 1\r\nline 2".into())),
            Some(ClipboardPayload::Text("line 1\r\nline 2".into()))
        );
    }

    #[test]
    fn supported_image_extensions_are_case_insensitive() {
        assert!(is_supported_clipboard_image_path(Path::new("capture.PNG")));
        assert!(is_supported_clipboard_image_path(Path::new("photo.JpEg")));
        assert!(!is_supported_clipboard_image_path(Path::new("notes.txt")));
    }

    #[test]
    fn pixel_and_encoded_limits_reject_oversized_images() {
        assert!(validate_image_dimensions(5_000, 5_000).is_ok());
        assert!(validate_image_dimensions(5_001, 5_000).is_err());
        assert!(validate_encoded_image_size(MAX_ENCODED_IMAGE_BYTES).is_ok());
        assert!(validate_encoded_image_size(MAX_ENCODED_IMAGE_BYTES + 1).is_err());
    }

    #[test]
    fn registered_png_size_is_checked_before_windows_payload_copy() {
        assert!(validate_windows_registered_png_size(0).is_err());
        assert!(validate_windows_registered_png_size(MAX_ENCODED_IMAGE_BYTES).is_ok());
        assert!(validate_windows_registered_png_size(MAX_ENCODED_IMAGE_BYTES + 1).is_err());
    }

    #[test]
    fn windows_bitmap_metadata_is_bounded_before_pixel_copy() {
        let accepted = validate_windows_bitmap_metadata(5_000, 5_000, 1, 32)
            .expect("accept bitmap at the pixel limit");
        assert_eq!((accepted.width, accepted.height), (5_000, 5_000));
        assert_eq!(accepted.pixel_copy_bytes, 100_000_000);

        assert!(validate_windows_bitmap_metadata(5_001, 5_000, 1, 32).is_err());
        assert!(validate_windows_bitmap_metadata(0, 100, 1, 32).is_err());
        assert!(validate_windows_bitmap_metadata(100, -1, 1, 32).is_err());
        assert!(validate_windows_bitmap_metadata(100, 100, 0, 32).is_err());
        assert!(validate_windows_bitmap_metadata(100, 100, 2, 32).is_err());
    }

    #[test]
    fn bitmap_source_is_reencoded_as_png() {
        let bitmap =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(2, 1, Rgba([10, 20, 30, 200])));
        let mut bmp_bytes = Cursor::new(Vec::new());
        bitmap
            .write_to(&mut bmp_bytes, ImageFormat::Bmp)
            .expect("encode BMP fixture");

        let encoded = encode_clipboard_image(ClipboardImageSource::Bitmap(bmp_bytes.into_inner()))
            .expect("encode clipboard bitmap");

        assert_eq!((encoded.width, encoded.height), (2, 1));
        assert!(encoded.png_bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        let decoded =
            image::load_from_memory_with_format(encoded.png_bytes.as_slice(), ImageFormat::Png)
                .expect("decode generated PNG");
        assert_eq!((decoded.width(), decoded.height()), (2, 1));
    }

    #[test]
    fn encoded_png_clipboard_source_uses_the_same_bounded_encoder() {
        let image =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(3, 2, Rgba([40, 50, 60, 255])));
        let mut source_png = Cursor::new(Vec::new());
        image
            .write_to(&mut source_png, ImageFormat::Png)
            .expect("encode source PNG fixture");

        let encoded =
            encode_clipboard_image(ClipboardImageSource::Encoded(source_png.into_inner()))
                .expect("encode clipboard PNG payload");

        assert_eq!((encoded.width, encoded.height), (3, 2));
        assert!(encoded.png_bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn image_file_source_uses_the_same_bounded_png_encoder() {
        let image =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(3, 2, Rgba([40, 50, 60, 255])));
        let mut source_png = Cursor::new(Vec::new());
        image
            .write_to(&mut source_png, ImageFormat::Png)
            .expect("encode source PNG fixture");
        let path = std::env::temp_dir().join(format!(
            "mica-term-clipboard-image-{}.png",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(path.as_path(), source_png.into_inner())
            .expect("write clipboard image fixture");

        let encoded = encode_clipboard_image(ClipboardImageSource::File(path.clone()))
            .expect("encode clipboard image file");
        let _ = std::fs::remove_file(path);

        assert_eq!((encoded.width, encoded.height), (3, 2));
        assert!(encoded.png_bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn encoded_clipboard_image_contains_bounded_landscape_preview() {
        let source = DynamicImage::new_rgba8(640, 480);
        let encoded = encode_png(source, 640, 480).expect("encode landscape image");

        assert_eq!((encoded.preview.width, encoded.preview.height), (240, 180));
        assert_eq!(
            encoded.preview.rgba.len(),
            encoded.preview.width as usize * encoded.preview.height as usize * 4,
        );
    }

    #[test]
    fn encoded_clipboard_image_contains_bounded_portrait_preview() {
        let source = DynamicImage::new_rgba8(300, 900);
        let encoded = encode_png(source, 300, 900).expect("encode portrait image");

        assert_eq!((encoded.preview.width, encoded.preview.height), (60, 180));
        assert_eq!(encoded.preview.rgba.len(), 60 * 180 * 4);
    }

    #[test]
    fn encoded_clipboard_image_does_not_upscale_preview() {
        let source = DynamicImage::new_rgba8(80, 40);
        let encoded = encode_png(source, 80, 40).expect("encode small image");

        assert_eq!((encoded.preview.width, encoded.preview.height), (80, 40));
        assert_eq!(encoded.preview.rgba.len(), 80 * 40 * 4);
    }
}
