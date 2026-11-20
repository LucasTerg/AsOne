//! „Zapisz do ZIP” i „Eksportuj do PDF”.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Pakuje pliki do ZIP (bez ponownej kompresji – zdjęcia są już skompresowane).
/// Powtarzające się nazwy dostają przyrostek `-2`, `-3`…
pub fn zip_files(files: &[PathBuf], out: &Path) -> Result<usize, String> {
    let tmp = out.with_file_name(format!(
        ".{}.part",
        out.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
    ));
    let result = (|| -> Result<usize, String> {
        let file = std::fs::File::create(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let mut used = std::collections::HashSet::new();
        for f in files {
            let bytes = std::fs::read(f).map_err(|e| format!("{}: {e}", f.display()))?;
            let name = unique_name(f, &mut used);
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
        }
        zip.finish().map_err(|e| e.to_string())?;
        Ok(files.len())
    })();
    match result {
        Ok(n) => std::fs::rename(&tmp, out).map(|()| n).map_err(|e| e.to_string()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn unique_name(path: &Path, used: &mut std::collections::HashSet<String>) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "plik".into());
    if used.insert(name.clone()) {
        return name;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (name.clone(), String::new()),
    };
    (2..)
        .map(|i| format!("{stem}-{i}{ext}"))
        .find(|n| used.insert(n.clone()))
        .unwrap_or(name)
}

/// Strona PDF: obraz JPEG (baseline) o podanym rozmiarze w pikselach.
pub struct PdfPage {
    pub size: [u32; 2],
    pub jpeg: Vec<u8>,
}

/// Składa wielostronicowy PDF – każda strona to jedno zdjęcie (96 DPI, strona = rozmiar zdjęcia).
pub fn images_to_pdf(pages: &[PdfPage]) -> Vec<u8> {
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets: Vec<usize> = Vec::new();
    let n = pages.len();
    // Obiekty: 1 – katalog, 2 – drzewo stron, potem dla każdej strony: strona, treść, obraz.
    let page_id = |i: usize| 3 + i * 3;
    let obj = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
        offsets.push(out.len());
        let id = offsets.len();
        out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    obj(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", page_id(i))).collect();
    obj(&mut out, &mut offsets, format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")).as_bytes());
    for (i, p) in pages.iter().enumerate() {
        let (pw, ph) = (p.size[0] as f64 * 0.75, p.size[1] as f64 * 0.75);
        let (pid, cid, iid) = (page_id(i), page_id(i) + 1, page_id(i) + 2);
        obj(
            &mut out,
            &mut offsets,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.2} {ph:.2}] /Resources << /XObject << /Im{i} {iid} 0 R >> >> /Contents {cid} 0 R >>"
            )
            .as_bytes(),
        );
        debug_assert_eq!(offsets.len(), pid);
        let content = format!("q {pw:.2} 0 0 {ph:.2} 0 0 cm /Im{i} Do Q");
        obj(
            &mut out,
            &mut offsets,
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).as_bytes(),
        );
        let mut image = format!(
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            p.size[0],
            p.size[1],
            p.jpeg.len()
        )
        .into_bytes();
        image.extend_from_slice(&p.jpeg);
        image.extend_from_slice(b"\nendstream");
        obj(&mut out, &mut offsets, &image);
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes());
    for off in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len() + 1).as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_ma_poprawne_przesuniecia() {
        let jpeg = crate::jpeg::encode_jpeg_baseline(&[200u8; 4 * 3 * 3], [4, 3], 90).unwrap();
        let pdf = images_to_pdf(&[PdfPage { size: [4, 3], jpeg: jpeg.clone() }, PdfPage { size: [4, 3], jpeg }]);
        let find = |needle: &[u8]| pdf.windows(needle.len()).rposition(|w| w == needle);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(find(b"/Count 2").is_some());
        // Tabela xref (po bajtach – dane JPEG nie są UTF-8).
        let xref = find(b"\nxref\n").unwrap() + 1;
        let table = std::str::from_utf8(&pdf[xref..]).unwrap();
        let entries: Vec<usize> = table.lines().skip(3).take_while(|l| l.ends_with(" n ")).map(|l| l[..10].parse().unwrap()).collect();
        assert_eq!(entries.len(), 8);
        for (i, off) in entries.iter().enumerate() {
            let head = format!("{} 0 obj", i + 1);
            assert!(pdf[*off..].starts_with(head.as_bytes()), "obiekt {}", i + 1);
        }
        let startxref: usize = table.lines().rev().nth(1).unwrap().parse().unwrap();
        assert_eq!(startxref, xref);
    }

    #[test]
    fn zip_z_powtorzonymi_nazwami() {
        let dir = std::env::temp_dir().join(format!("asone-zip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::create_dir_all(dir.join("b")).unwrap();
        std::fs::write(dir.join("a/x.jpg"), b"1").unwrap();
        std::fs::write(dir.join("b/x.jpg"), b"22").unwrap();
        let out = dir.join("paczka.zip");
        assert_eq!(zip_files(&[dir.join("a/x.jpg"), dir.join("b/x.jpg")], &out).unwrap(), 2);
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(&bytes[..2], b"PK");
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("x.jpg") && text.contains("x-2.jpg"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
