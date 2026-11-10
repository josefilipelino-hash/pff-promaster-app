//! PDF de várias páginas (uma imagem JPEG por página): `MediaBox`/`BleedBox` com a
//! sangria e `TrimBox` no corte, perfil ICC (`ICCBased`) — o que os laboratórios usam
//! para posicionar o corte.

pub struct PdfPage {
    pub jpeg: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
    /// Tamanho com sangria, em pontos (1/72 in).
    pub width_pt: f32,
    pub height_pt: f32,
    /// Sangria em pontos (a `TrimBox` fica para dentro).
    pub bleed_pt: f32,
}

fn text(s: &str) -> String {
    if s.is_ascii() {
        format!("({})", s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)"))
    } else {
        let mut h = String::from("<FEFF");
        for u in s.encode_utf16() {
            h += &format!("{u:04X}");
        }
        h + ">"
    }
}

pub fn write(pages: &[PdfPage], icc: &[u8], title: &str) -> Vec<u8> {
    // Objetos: 1 catálogo, 2 páginas, 3 ICC, 4 info; por página: página, conteúdo, imagem.
    let mut objs: Vec<Vec<u8>> = Vec::new();
    let page_ids: Vec<usize> = (0..pages.len()).map(|i| 5 + i * 3).collect();
    objs.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objs.push(
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            page_ids.iter().map(|i| format!("{i} 0 R")).collect::<Vec<_>>().join(" "),
            pages.len()
        )
        .into_bytes(),
    );
    objs.push(
        [
            format!("<< /N 3 /Alternate /DeviceRGB /Length {} >>\nstream\n", icc.len()).as_bytes(),
            icc,
            b"\nendstream",
        ]
        .concat(),
    );
    objs.push(format!("<< /Producer (PFF ProMaster) /Title {} >>", text(title)).into_bytes());
    for (i, p) in pages.iter().enumerate() {
        let (pid, cid, iid) = (page_ids[i], page_ids[i] + 1, page_ids[i] + 2);
        let (w, h, b) = (p.width_pt, p.height_pt, p.bleed_pt);
        objs.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.3} {h:.3}] /BleedBox [0 0 {w:.3} {h:.3}] /TrimBox [{b:.3} {b:.3} {:.3} {:.3}] /Resources << /XObject << /Im0 {iid} 0 R >> >> /Contents {cid} 0 R >>",
                w - b,
                h - b
            )
            .into_bytes(),
        );
        debug_assert_eq!(objs.len(), pid);
        let content = format!("q\n{w:.3} 0 0 {h:.3} 0 0 cm\n/Im0 Do\nQ\n");
        objs.push(
            [
                format!("<< /Length {} >>\nstream\n", content.len()).as_bytes(),
                content.as_bytes(),
                b"\nendstream",
            ]
            .concat(),
        );
        objs.push(
            [
                format!(
                    "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace [/ICCBased 3 0 R] /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
                    p.width_px,
                    p.height_px,
                    p.jpeg.len()
                )
                .as_bytes(),
                &p.jpeg,
                b"\nendstream",
            ]
            .concat(),
        );
    }
    let mut out = b"%PDF-1.6\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend(o);
        out.extend(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offsets {
        out.extend(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 4 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}
