use momento_core_types::{CoreError, CoreResult};
use momento_libraw_sys as sys;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};

/// Limite de memória por ficheiro RAW dentro da LibRaw (MB).
const MAX_RAW_MEMORY_MB: u32 = 4096;

fn cstr(buf: &[c_char]) -> String {
    let bytes: Vec<u8> = buf.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

fn non_empty(s: String) -> Option<String> {
    (!s.is_empty()).then_some(s)
}

fn error_text(code: c_int) -> String {
    // SAFETY: mlr_strerror devolve sempre um ponteiro para uma string estática.
    let p = unsafe { sys::mlr_strerror(code) };
    if p.is_null() {
        return format!("erro LibRaw {code}");
    }
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

/// Converte um código LibRaw no erro do núcleo com a categoria certa.
pub(crate) fn map_error(code: c_int, what: &str) -> CoreError {
    let msg = format!("{what}: {} (LibRaw {code})", error_text(code));
    match code {
        sys::LIBRAW_FILE_UNSUPPORTED | sys::LIBRAW_NOT_IMPLEMENTED => {
            CoreError::UnsupportedFormat(msg)
        }
        sys::LIBRAW_UNSUFFICIENT_MEMORY | sys::LIBRAW_TOO_BIG => CoreError::Other(msg),
        _ => CoreError::Corrupt(msg),
    }
}

pub fn libraw_version() -> String {
    // SAFETY: string estática.
    unsafe { CStr::from_ptr(sys::mlr_version()) }.to_string_lossy().into_owned()
}

pub fn libraw_camera_count() -> usize {
    // SAFETY: sem argumentos.
    unsafe { sys::mlr_camera_count() }.max(0) as usize
}

/// Orientação LibRaw (`sizes.flip`) → orientação EXIF (1–8).
pub fn flip_to_exif_orientation(flip: i32) -> u8 {
    match flip {
        3 => 3,
        5 => 8,
        6 => 6,
        _ => 1,
    }
}

/// Informação lida na abertura (sem descodificar os dados do sensor).
#[derive(Debug, Clone, PartialEq)]
pub struct RawInfo {
    pub make: String,
    pub model: String,
    pub normalized_make: String,
    pub normalized_model: String,
    pub lens: Option<String>,
    pub capture_time: Option<String>,
    pub iso: Option<u32>,
    pub shutter: Option<f64>,
    pub aperture: Option<f64>,
    pub focal_length: Option<f64>,
    pub focal_length_35mm: Option<u32>,
    pub raw_width: u32,
    pub raw_height: u32,
    /// Dimensões de saída antes da rotação.
    pub width: u32,
    pub height: u32,
    pub orientation: u8,
    pub bits: u8,
    pub colors: u8,
    pub color_description: String,
    pub is_dng: bool,
    pub is_foveon: bool,
    pub decoder: String,
    /// A LibRaw consegue descodificar os dados do sensor deste ficheiro.
    pub sensor_supported: bool,
    /// Maior preview embutida utilizável: (índice, formato LibRaw, largura, altura).
    pub preview: Option<(i32, i32, u32, u32)>,
}

/// Imagem linear de 16 bits (Rec. 2020) para a edição.
#[derive(Debug, Clone)]
pub struct LinearBitmap {
    pub width: u32,
    pub height: u32,
    /// RGB intercalado, 0–65535, linear.
    pub rgb: Vec<u16>,
}

/// Bitmap RGB/cinzento devolvido pela LibRaw.
#[derive(Debug, Clone)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum EmbeddedPreview {
    Jpeg(Vec<u8>),
    JpegXl(Vec<u8>),
    Bitmap(Bitmap),
}

/// Ficheiro RAW aberto. O buffer é mantido vivo enquanto a instância existir.
pub struct RawFile<'a> {
    handle: *mut sys::mlr_handle,
    info: RawInfo,
    _data: &'a [u8],
}

// SAFETY: cada RawFile tem a sua própria instância LibRaw (sem estado global partilhado
// quando compilada sem LIBRAW_NOTHREADS); pode ser movida entre threads.
unsafe impl Send for RawFile<'_> {}

struct Image(sys::mlr_image);

impl Drop for Image {
    fn drop(&mut self) {
        // SAFETY: imagem obtida de mlr_thumb/mlr_develop; libertada uma única vez.
        unsafe { sys::mlr_image_free(&mut self.0) };
    }
}

impl Image {
    fn bytes(&self) -> &[u8] {
        if self.0.data.is_null() {
            return &[];
        }
        // SAFETY: a LibRaw garante `size` bytes válidos em `data` até mlr_image_free.
        unsafe { std::slice::from_raw_parts(self.0.data, self.0.size) }
    }

    fn to_bitmap(&self) -> CoreResult<Bitmap> {
        let (w, h, c, bits) =
            (self.0.width.max(0) as u32, self.0.height.max(0) as u32, self.0.colors, self.0.bits);
        let px = w as usize * h as usize;
        let data = self.bytes();
        let rgb = match (c, bits) {
            (3, 8) if data.len() >= px * 3 => data[..px * 3].to_vec(),
            (1, 8) if data.len() >= px => data[..px].iter().flat_map(|&v| [v, v, v]).collect(),
            (3, 16) if data.len() >= px * 6 => data[..px * 6]
                .chunks_exact(2)
                .map(|b| (u16::from_ne_bytes([b[0], b[1]]) >> 8) as u8)
                .collect(),
            (1, 16) if data.len() >= px * 2 => data[..px * 2]
                .chunks_exact(2)
                .flat_map(|b| {
                    let v = (u16::from_ne_bytes([b[0], b[1]]) >> 8) as u8;
                    [v, v, v]
                })
                .collect(),
            _ => {
                return Err(CoreError::Corrupt(format!(
                    "bitmap LibRaw inesperado: {c} canais, {bits} bits"
                )))
            }
        };
        Ok(Bitmap { width: w, height: h, rgb })
    }
}

impl<'a> RawFile<'a> {
    /// Abre um RAW a partir dos bytes do ficheiro. Nunca entra em pânico com dados inválidos.
    pub fn open(data: &'a [u8]) -> CoreResult<Self> {
        let mut handle = std::ptr::null_mut();
        let mut raw = std::mem::MaybeUninit::<sys::mlr_info>::zeroed();
        // SAFETY: `data` é válido durante a vida de RawFile (lifetime 'a); `raw` tem o layout
        // de mlr_info; o shim apanha todas as exceções C++.
        let rc = unsafe {
            sys::mlr_open_buffer(
                data.as_ptr() as *const c_void,
                data.len(),
                MAX_RAW_MEMORY_MB,
                &mut handle,
                raw.as_mut_ptr(),
            )
        };
        if rc != sys::LIBRAW_SUCCESS || handle.is_null() {
            return Err(map_error(rc, "abrir RAW"));
        }
        // SAFETY: inicializado pelo shim em caso de sucesso.
        let r = unsafe { raw.assume_init() };
        let pos = |v: f32| (v.is_finite() && v > 0.0).then_some(v as f64);
        let preview = (r.thumb_index >= 0 && matches!(r.thumb_format, 1 | 2 | 3 | 7)).then_some((
            r.thumb_index,
            r.thumb_format,
            r.thumb_width.max(0) as u32,
            r.thumb_height.max(0) as u32,
        ));
        let info = RawInfo {
            make: cstr(&r.make),
            model: cstr(&r.model),
            normalized_make: cstr(&r.normalized_make),
            normalized_model: cstr(&r.normalized_model),
            lens: non_empty(cstr(&r.lens)),
            capture_time: non_empty(cstr(&r.capture_time)),
            iso: pos(r.iso).map(|v| v.round() as u32),
            shutter: pos(r.shutter),
            aperture: pos(r.aperture),
            focal_length: pos(r.focal_length),
            focal_length_35mm: (r.focal_length_35mm > 0).then_some(r.focal_length_35mm as u32),
            raw_width: r.raw_width.max(0) as u32,
            raw_height: r.raw_height.max(0) as u32,
            width: r.width.max(0) as u32,
            height: r.height.max(0) as u32,
            orientation: flip_to_exif_orientation(r.flip),
            bits: r.raw_bps.clamp(0, 32) as u8,
            colors: r.colors.clamp(0, 4) as u8,
            color_description: cstr(&r.cdesc),
            is_dng: r.dng_version != 0,
            is_foveon: r.is_foveon != 0,
            decoder: cstr(&r.decoder_name),
            sensor_supported: r.decoder_supported != 0,
            preview,
        };
        Ok(RawFile { handle, info, _data: data })
    }

    pub fn info(&self) -> &RawInfo {
        &self.info
    }

    /// Extrai a maior preview embutida (sem descodificar o sensor).
    pub fn embedded_preview(&mut self) -> CoreResult<Option<EmbeddedPreview>> {
        let Some((index, _, _, _)) = self.info.preview else { return Ok(None) };
        let mut img = Image(unsafe { std::mem::zeroed() });
        // SAFETY: handle válido; `img` libertada no Drop.
        let rc = unsafe { sys::mlr_thumb(self.handle, index as c_int, &mut img.0) };
        if rc == sys::LIBRAW_NO_THUMBNAIL
            || rc == sys::LIBRAW_UNSUPPORTED_THUMBNAIL
            || rc == sys::LIBRAW_REQUEST_FOR_NONEXISTENT_IMAGE
        {
            return Ok(None);
        }
        if rc != sys::LIBRAW_SUCCESS {
            return Err(map_error(rc, "preview embutida"));
        }
        Ok(Some(match img.0.type_ {
            1 => EmbeddedPreview::Jpeg(img.bytes().to_vec()),
            3 => EmbeddedPreview::JpegXl(img.bytes().to_vec()),
            2 => EmbeddedPreview::Bitmap(img.to_bitmap()?),
            _ => return Ok(None),
        }))
    }

    /// Revela os dados do sensor (sRGB 8 bits, balanço de brancos da câmara), sem rotação.
    pub fn develop(&mut self, half_size: bool) -> CoreResult<Bitmap> {
        if !self.info.sensor_supported {
            return Err(CoreError::UnsupportedFormat(format!(
                "compressão do sensor não suportada pela LibRaw nesta compilação ({})",
                self.info.decoder
            )));
        }
        let mut img = Image(unsafe { std::mem::zeroed() });
        // SAFETY: handle válido; `img` libertada no Drop.
        let rc = unsafe { sys::mlr_develop(self.handle, half_size as c_int, &mut img.0) };
        if rc != sys::LIBRAW_SUCCESS {
            return Err(map_error(rc, "revelar RAW"));
        }
        if img.0.type_ != 2 {
            return Err(CoreError::Corrupt("revelação não devolveu um bitmap".into()));
        }
        img.to_bitmap()
    }
}

impl RawFile<'_> {
    /// Revela para edição: RGB 16 bits linear em Rec. 2020 (sem rotação).
    pub fn develop_linear(&mut self, half_size: bool) -> CoreResult<LinearBitmap> {
        if !self.info.sensor_supported {
            return Err(CoreError::UnsupportedFormat(format!(
                "compressão do sensor não suportada pela LibRaw nesta compilação ({})",
                self.info.decoder
            )));
        }
        let mut img = Image(unsafe { std::mem::zeroed() });
        // SAFETY: handle válido; `img` libertada no Drop.
        let rc = unsafe { sys::mlr_develop_linear(self.handle, half_size as c_int, &mut img.0) };
        if rc != sys::LIBRAW_SUCCESS {
            return Err(map_error(rc, "revelar RAW (linear)"));
        }
        let (w, h) = (img.0.width.max(0) as u32, img.0.height.max(0) as u32);
        let px = w as usize * h as usize;
        let data = img.bytes();
        if img.0.type_ != 2 || img.0.colors != 3 || img.0.bits != 16 || data.len() < px * 6 {
            return Err(CoreError::Corrupt("revelação linear inesperada".into()));
        }
        let rgb =
            data[..px * 6].chunks_exact(2).map(|b| u16::from_ne_bytes([b[0], b[1]])).collect();
        Ok(LinearBitmap { width: w, height: h, rgb })
    }
}

impl Drop for RawFile<'_> {
    fn drop(&mut self) {
        // SAFETY: handle obtido de mlr_open_buffer e fechado uma única vez.
        unsafe { sys::mlr_close(self.handle) };
    }
}
