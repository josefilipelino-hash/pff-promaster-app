//! Ligações de baixo nível à camada C `shim/momento_libraw.h`. Usar `momento-raw-engine`.
#![allow(non_camel_case_types)]

use std::os::raw::{c_char, c_int, c_uint, c_void};

// Garante que a zlib estática é ligada (usada pela LibRaw com USE_ZLIB).
extern crate libz_sys;

pub const LIBRAW_SUCCESS: c_int = 0;
pub const LIBRAW_FILE_UNSUPPORTED: c_int = -2;
pub const LIBRAW_REQUEST_FOR_NONEXISTENT_IMAGE: c_int = -3;
pub const LIBRAW_NO_THUMBNAIL: c_int = -5;
pub const LIBRAW_UNSUPPORTED_THUMBNAIL: c_int = -6;
pub const LIBRAW_NOT_IMPLEMENTED: c_int = -8;
pub const LIBRAW_UNSUFFICIENT_MEMORY: c_int = -100007;
pub const LIBRAW_DATA_ERROR: c_int = -100008;
pub const LIBRAW_IO_ERROR: c_int = -100009;
pub const LIBRAW_TOO_BIG: c_int = -100012;
pub const MLR_EXCEPTION: c_int = -900001;

#[repr(C)]
pub struct mlr_handle {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mlr_info {
    pub make: [c_char; 64],
    pub model: [c_char; 64],
    pub normalized_make: [c_char; 64],
    pub normalized_model: [c_char; 64],
    pub lens: [c_char; 128],
    pub lens_make: [c_char; 128],
    pub capture_time: [c_char; 32],
    pub iso: f32,
    pub shutter: f32,
    pub aperture: f32,
    pub focal_length: f32,
    pub focal_length_35mm: c_int,
    pub raw_width: c_int,
    pub raw_height: c_int,
    pub width: c_int,
    pub height: c_int,
    pub flip: c_int,
    pub raw_bps: c_int,
    pub colors: c_int,
    pub cdesc: [c_char; 8],
    pub dng_version: c_uint,
    pub is_foveon: c_int,
    pub filters_nonzero: c_int,
    pub decoder_name: [c_char; 64],
    pub decoder_supported: c_int,
    pub thumb_count: c_int,
    pub thumb_index: c_int,
    pub thumb_format: c_int,
    pub thumb_width: c_int,
    pub thumb_height: c_int,
    pub thumb_length: c_uint,
    pub thumb_flip: c_int,
}

#[repr(C)]
pub struct mlr_image {
    pub type_: c_int,
    pub width: c_int,
    pub height: c_int,
    pub colors: c_int,
    pub bits: c_int,
    pub data: *const u8,
    pub size: usize,
    pub opaque: *mut c_void,
}

extern "C" {
    pub fn mlr_open_buffer(
        buf: *const c_void,
        len: usize,
        max_raw_memory_mb: c_uint,
        out: *mut *mut mlr_handle,
        info: *mut mlr_info,
    ) -> c_int;
    pub fn mlr_thumb(h: *mut mlr_handle, index: c_int, out: *mut mlr_image) -> c_int;
    pub fn mlr_develop(h: *mut mlr_handle, half_size: c_int, out: *mut mlr_image) -> c_int;
    pub fn mlr_develop_linear(h: *mut mlr_handle, half_size: c_int, out: *mut mlr_image) -> c_int;
    pub fn mlr_image_free(img: *mut mlr_image);
    pub fn mlr_close(h: *mut mlr_handle);
    pub fn mlr_strerror(code: c_int) -> *const c_char;
    pub fn mlr_version() -> *const c_char;
    pub fn mlr_camera_count() -> c_int;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_links_and_reports_version() {
        // SAFETY: funções sem argumentos que devolvem ponteiros para dados estáticos.
        let v = unsafe { std::ffi::CStr::from_ptr(mlr_version()) }.to_string_lossy().into_owned();
        assert!(v.starts_with("0.22.3"), "{v}");
        assert_eq!(
            unsafe { mlr_camera_count() },
            1274,
            "lista de câmaras da LibRaw 0.22.3 nesta compilação"
        );
    }

    #[test]
    fn garbage_buffer_is_rejected_without_crashing() {
        let junk = vec![0x42u8; 4096];
        let mut h = std::ptr::null_mut();
        let mut info = std::mem::MaybeUninit::<mlr_info>::zeroed();
        // SAFETY: buffer válido durante a chamada; `info` tem o layout de mlr_info.
        let rc = unsafe {
            mlr_open_buffer(
                junk.as_ptr() as *const c_void,
                junk.len(),
                0,
                &mut h,
                info.as_mut_ptr(),
            )
        };
        assert_ne!(rc, LIBRAW_SUCCESS);
        assert!(h.is_null());
    }
}
