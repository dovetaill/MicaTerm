//! Shared resource budgets for clipboard and terminal-protocol images.

pub(crate) const MAX_IMAGE_PIXELS: u64 = 25_000_000;
pub(crate) const MAX_ENCODED_IMAGE_BYTES: usize = 20 * 1024 * 1024;
pub(crate) const MAX_DECODED_IMAGE_BYTES: u64 = 100 * 1024 * 1024;
pub(crate) const MAX_TERMINAL_IMAGE_RESOURCE_BYTES: usize = 128 * 1024 * 1024;

const BASE64_ENCODED_IMAGE_BYTES: usize = MAX_ENCODED_IMAGE_BYTES.div_ceil(3) * 4;
const MAX_IMAGE_PROTOCOL_HEADER_BYTES: usize = 64 * 1024;

pub(crate) const MAX_BASE64_IMAGE_SEQUENCE_BYTES: usize =
    BASE64_ENCODED_IMAGE_BYTES + MAX_IMAGE_PROTOCOL_HEADER_BYTES;
pub(crate) const MAX_SIXEL_SEQUENCE_BYTES: usize =
    MAX_ENCODED_IMAGE_BYTES + MAX_IMAGE_PROTOCOL_HEADER_BYTES;
