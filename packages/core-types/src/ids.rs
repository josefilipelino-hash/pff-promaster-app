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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_serialize_as_plain_numbers() {
        assert_eq!(serde_json::to_string(&AssetId(42)).unwrap(), "42");
        let v: VariantId = serde_json::from_str("7").unwrap();
        assert_eq!(v, VariantId(7));
    }
}
