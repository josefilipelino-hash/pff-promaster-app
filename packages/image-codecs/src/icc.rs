//! Leitura mínima de perfis ICC: descrição do perfil (tag `desc`), para mostrar ao
//! utilizador em que espaço de cor está a fotografia. A conversão de cor em si é feita
//! pela LittleCMS numa fase posterior.

fn be32(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

/// Descrição legível de um perfil ICC (v2 `desc` ou v4 `mluc`), se existir.
pub fn icc_description(icc: &[u8]) -> Option<String> {
    if icc.len() < 132 || icc.get(36..40)? != b"acsp" {
        return None;
    }
    let count = be32(icc, 128)? as usize;
    for i in 0..count.min(256) {
        let base = 132 + i * 12;
        let sig = icc.get(base..base + 4)?;
        if sig != b"desc" {
            continue;
        }
        let off = be32(icc, base + 4)? as usize;
        let len = be32(icc, base + 8)? as usize;
        let tag = icc.get(off..off.checked_add(len)?)?;
        return match tag.get(0..4)? {
            b"desc" => {
                let n = be32(tag, 8)? as usize;
                let s = tag.get(12..12 + n.min(tag.len().saturating_sub(12)))?;
                let text = String::from_utf8_lossy(s).trim_end_matches('\0').trim().to_string();
                (!text.is_empty()).then_some(text)
            }
            b"mluc" => {
                let records = be32(tag, 8)? as usize;
                if records == 0 {
                    return None;
                }
                let rlen = be32(tag, 20)? as usize;
                let roff = be32(tag, 24)? as usize;
                let raw = tag.get(roff..roff.checked_add(rlen)?)?;
                let units: Vec<u16> =
                    raw.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                let text =
                    String::from_utf16_lossy(&units).trim_end_matches('\0').trim().to_string();
                (!text.is_empty()).then_some(text)
            }
            _ => None,
        };
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_with(tag: &[u8]) -> Vec<u8> {
        let mut p = vec![0u8; 132 + 12];
        p[36..40].copy_from_slice(b"acsp");
        p[128..132].copy_from_slice(&1u32.to_be_bytes());
        p[132..136].copy_from_slice(b"desc");
        p[136..140].copy_from_slice(&(144u32).to_be_bytes());
        p[140..144].copy_from_slice(&(tag.len() as u32).to_be_bytes());
        p.extend_from_slice(tag);
        p
    }

    #[test]
    fn reads_v2_and_v4_descriptions() {
        let mut v2 = b"desc\0\0\0\0".to_vec();
        let name = b"Adobe RGB (1998)\0";
        v2.extend_from_slice(&(name.len() as u32).to_be_bytes());
        v2.extend_from_slice(name);
        assert_eq!(icc_description(&profile_with(&v2)).as_deref(), Some("Adobe RGB (1998)"));

        let text: Vec<u8> = "Display P3".encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
        let mut v4 = b"mluc\0\0\0\0".to_vec();
        v4.extend_from_slice(&1u32.to_be_bytes()); // registos
        v4.extend_from_slice(&12u32.to_be_bytes()); // tamanho do registo
        v4.extend_from_slice(b"enUS");
        v4.extend_from_slice(&(text.len() as u32).to_be_bytes());
        v4.extend_from_slice(&28u32.to_be_bytes());
        v4.extend_from_slice(&text);
        assert_eq!(icc_description(&profile_with(&v4)).as_deref(), Some("Display P3"));
    }

    #[test]
    fn garbage_is_ignored() {
        assert_eq!(icc_description(b"nope"), None);
        assert_eq!(icc_description(&vec![0xFF; 500]), None);
    }
}
