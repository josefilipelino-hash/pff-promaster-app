//! Identificadores fortemente tipados. Correspondem a `INTEGER PRIMARY KEY` no SQLite.
//! Tipos distintos impedem, em tempo de compilação, trocar um `AssetId` por um `VariantId`.

use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($($name:ident),* $(,)?) => {$(
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub i64);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    )*};
}

id_type!(
    AssetId,
    VariantId,
    SourceRootId,
    CameraId,
    LensId,
    JobId,
    ImportSessionId,
    MomentId,
    PersonId,
    GroupId,
    PresetId
);

/// Chave de uma fotografia na interface: o `AssetId` para a versão principal e
/// `VIRTUAL_KEY_BASE + VariantId` para as cópias virtuais (gamas que nunca se cruzam).
pub const VIRTUAL_KEY_BASE: i64 = 1 << 40;

/// O que uma chave da interface designa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoRef {
    Master(AssetId),
    Variant(VariantId),
}

pub fn decode_key(key: i64) -> PhotoRef {
    if key >= VIRTUAL_KEY_BASE {
        PhotoRef::Variant(VariantId(key - VIRTUAL_KEY_BASE))
    } else {
        PhotoRef::Master(AssetId(key))
    }
}

pub fn virtual_key(v: VariantId) -> i64 {
    VIRTUAL_KEY_BASE + v.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_serialize_as_plain_numbers() {
        assert_eq!(serde_json::to_string(&AssetId(42)).unwrap(), "42");
        let v: VariantId = serde_json::from_str("7").unwrap();
        assert_eq!(v, VariantId(7));
    }

    #[test]
    fn photo_keys_never_collide() {
        assert_eq!(decode_key(42), PhotoRef::Master(AssetId(42)));
        assert_eq!(decode_key(virtual_key(VariantId(42))), PhotoRef::Variant(VariantId(42)));
    }
}
