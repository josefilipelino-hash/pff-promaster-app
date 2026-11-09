//! # momento-raw-engine
//!
//! RAW através da LibRaw 0.22.3 (compilada do código-fonte em `libraw-sys`).
//!
//! - [`RawFile`]: abertura segura de um RAW em memória (todas as exceções C++ são
//!   convertidas em erros; cada instância é independente e pode ser usada em paralelo
//!   noutras threads);
//! - [`RawCodec`]: adaptador do registo de codecs — **preview embutida primeiro**,
//!   revelação a meia resolução ou completa só quando necessário;
//! - metadados (câmara, objetiva, exposição, data, orientação, profundidade de bits,
//!   cor) e nível de suporte por ficheiro (completo / parcial).

mod codec;
mod file;

pub use codec::{RawCodec, EMBEDDED_GOOD_ENOUGH_LONG_EDGE};
pub use file::{
    flip_to_exif_orientation, libraw_camera_count, libraw_version, Bitmap, EmbeddedPreview,
    LinearBitmap, RawFile, RawInfo,
};
