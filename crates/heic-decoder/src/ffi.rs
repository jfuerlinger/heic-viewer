//! Minimal hand-written bindings for the subset of the libheif C API we use.
//! Verified against libheif 1.23.5 headers (`libheif/api/libheif/*.h`); all of these
//! functions exist since libheif 1.17, `cancel_decoding` since decoding-options version 6.

#![allow(non_camel_case_types, dead_code)]

use std::os::raw::{c_char, c_int, c_void};

#[repr(C)]
pub struct heif_context {
    _private: [u8; 0],
}
#[repr(C)]
pub struct heif_image_handle {
    _private: [u8; 0],
}
#[repr(C)]
pub struct heif_image {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct heif_error {
    pub code: c_int,
    pub subcode: c_int,
    pub message: *const c_char,
}

pub type heif_item_id = u32;

pub const HEIF_COLORSPACE_RGB: c_int = 1;

pub const HEIF_CHROMA_INTERLEAVED_RGB: c_int = 10;
pub const HEIF_CHROMA_INTERLEAVED_RGBA: c_int = 11;
pub const HEIF_CHROMA_INTERLEAVED_RRGGBB_LE: c_int = 14;
pub const HEIF_CHROMA_INTERLEAVED_RRGGBBAA_LE: c_int = 15;

pub const HEIF_CHANNEL_INTERLEAVED: c_int = 10;

const fn fourcc(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | (s[3] as u32)
}
pub const HEIF_COLOR_PROFILE_TYPE_NCLX: u32 = fourcc(b"nclx");
pub const HEIF_COLOR_PROFILE_TYPE_RICC: u32 = fourcc(b"rICC");
pub const HEIF_COLOR_PROFILE_TYPE_PROF: u32 = fourcc(b"prof");

#[repr(C)]
pub struct heif_color_profile_nclx {
    pub version: u8,
    pub color_primaries: c_int,
    pub transfer_characteristics: c_int,
    pub matrix_coefficients: c_int,
    pub full_range_flag: u8,
    pub color_primary_red_x: f32,
    pub color_primary_red_y: f32,
    pub color_primary_green_x: f32,
    pub color_primary_green_y: f32,
    pub color_primary_blue_x: f32,
    pub color_primary_blue_y: f32,
    pub color_primary_white_x: f32,
    pub color_primary_white_y: f32,
}

#[repr(C)]
pub struct heif_color_conversion_options {
    pub version: u8,
    pub preferred_chroma_downsampling_algorithm: c_int,
    pub preferred_chroma_upsampling_algorithm: c_int,
    pub only_use_preferred_chroma_algorithm: u8,
}

/// Prefix of `heif_decoding_options` up to version 6. The struct is always allocated by
/// libheif (`heif_decoding_options_alloc`), so later fields exist but are never touched here.
#[repr(C)]
pub struct heif_decoding_options_v6 {
    pub version: u8,
    pub ignore_transformations: u8,
    pub start_progress: Option<unsafe extern "C" fn(c_int, c_int, *mut c_void)>,
    pub on_progress: Option<unsafe extern "C" fn(c_int, c_int, *mut c_void)>,
    pub end_progress: Option<unsafe extern "C" fn(c_int, *mut c_void)>,
    pub progress_user_data: *mut c_void,
    pub convert_hdr_to_8bit: u8,
    pub strict_decoding: u8,
    pub decoder_id: *const c_char,
    pub color_conversion_options: heif_color_conversion_options,
    pub cancel_decoding: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
}

extern "C" {
    pub fn heif_init(params: *mut c_void) -> heif_error;
    pub fn heif_get_version() -> *const c_char;

    pub fn heif_context_alloc() -> *mut heif_context;
    pub fn heif_context_free(ctx: *mut heif_context);
    pub fn heif_context_read_from_memory_without_copy(
        ctx: *mut heif_context,
        mem: *const c_void,
        size: usize,
        options: *const c_void,
    ) -> heif_error;
    pub fn heif_context_set_maximum_image_size_limit(ctx: *mut heif_context, maximum_width: c_int);
    pub fn heif_context_get_primary_image_handle(
        ctx: *mut heif_context,
        out: *mut *mut heif_image_handle,
    ) -> heif_error;

    pub fn heif_image_handle_release(handle: *const heif_image_handle);
    pub fn heif_image_handle_get_width(handle: *const heif_image_handle) -> c_int;
    pub fn heif_image_handle_get_height(handle: *const heif_image_handle) -> c_int;
    pub fn heif_image_handle_has_alpha_channel(handle: *const heif_image_handle) -> c_int;
    pub fn heif_image_handle_is_premultiplied_alpha(handle: *const heif_image_handle) -> c_int;
    pub fn heif_image_handle_get_luma_bits_per_pixel(handle: *const heif_image_handle) -> c_int;

    pub fn heif_image_handle_get_number_of_thumbnails(handle: *const heif_image_handle) -> c_int;
    pub fn heif_image_handle_get_list_of_thumbnail_IDs(
        handle: *const heif_image_handle,
        ids: *mut heif_item_id,
        count: c_int,
    ) -> c_int;
    pub fn heif_image_handle_get_thumbnail(
        handle: *const heif_image_handle,
        id: heif_item_id,
        out: *mut *mut heif_image_handle,
    ) -> heif_error;

    pub fn heif_image_handle_get_color_profile_type(handle: *const heif_image_handle) -> u32;
    pub fn heif_image_handle_get_raw_color_profile_size(handle: *const heif_image_handle) -> usize;
    pub fn heif_image_handle_get_raw_color_profile(
        handle: *const heif_image_handle,
        out_data: *mut c_void,
    ) -> heif_error;
    pub fn heif_image_handle_get_nclx_color_profile(
        handle: *const heif_image_handle,
        out: *mut *mut heif_color_profile_nclx,
    ) -> heif_error;
    pub fn heif_nclx_color_profile_free(profile: *mut heif_color_profile_nclx);

    pub fn heif_image_handle_get_number_of_metadata_blocks(
        handle: *const heif_image_handle,
        type_filter: *const c_char,
    ) -> c_int;
    pub fn heif_image_handle_get_list_of_metadata_block_IDs(
        handle: *const heif_image_handle,
        type_filter: *const c_char,
        ids: *mut heif_item_id,
        count: c_int,
    ) -> c_int;
    pub fn heif_image_handle_get_metadata_size(handle: *const heif_image_handle, metadata_id: heif_item_id) -> usize;
    pub fn heif_image_handle_get_metadata(
        handle: *const heif_image_handle,
        metadata_id: heif_item_id,
        out_data: *mut c_void,
    ) -> heif_error;

    pub fn heif_decoding_options_alloc() -> *mut heif_decoding_options_v6;
    pub fn heif_decoding_options_free(options: *mut heif_decoding_options_v6);
    pub fn heif_decode_image(
        handle: *const heif_image_handle,
        out_img: *mut *mut heif_image,
        colorspace: c_int,
        chroma: c_int,
        options: *const heif_decoding_options_v6,
    ) -> heif_error;

    pub fn heif_image_release(img: *const heif_image);
    pub fn heif_image_get_width(img: *const heif_image, channel: c_int) -> c_int;
    pub fn heif_image_get_height(img: *const heif_image, channel: c_int) -> c_int;
    pub fn heif_image_get_bits_per_pixel_range(img: *const heif_image, channel: c_int) -> c_int;
    pub fn heif_image_get_plane_readonly(img: *const heif_image, channel: c_int, out_stride: *mut c_int) -> *const u8;
}
