//! Metadane pliku obrazu do podglądu (zakładka „Metadane”): EXIF, XMP, IPTC, teksty PNG,
//! komentarze, profil ICC, Content Credentials (C2PA).
//!
//! Tylko odczyt – eksport i tak zapisuje czysty JPEG bez żadnych metadanych. Celem jest szybka
//! odpowiedź: skąd jest zdjęcie, czy jest zastrzeżone (prawa, licencja, bank zdjęć), czy powstało
//! w AI, czy zdradza lokalizację lub dane osobowe.

use std::io::Cursor;

/// Rodzaj uwagi w podsumowaniu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    /// Prawa autorskie, licencja, bank zdjęć.
    Rights,
    /// Wygenerowane lub edytowane przez AI.
    Ai,
    /// Lokalizacja GPS.
    Location,
    /// Autor, właściciel, numer seryjny.
    Personal,
    /// Aparat, program, C2PA, ICC…
    Info,
}

#[derive(Clone, Debug)]
pub struct Notice {
    pub kind: NoticeKind,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct MetaGroup {
    pub title: String,
    pub entries: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default)]
pub struct MetaReport {
    pub format: &'static str,
    pub notices: Vec<Notice>,
    pub groups: Vec<MetaGroup>,
}

impl MetaReport {
    pub fn is_empty(&self) -> bool {
        self.groups.iter().all(|g| g.entries.is_empty())
    }

    fn notice(&mut self, kind: NoticeKind, text: impl Into<String>) {
        let text = text.into();
        if !self.notices.iter().any(|n| n.kind == kind && n.text == text) {
            self.notices.push(Notice { kind, text });
        }
    }

    fn group(&mut self, title: &str, entries: Vec<(String, String)>) {
        if !entries.is_empty() {
            self.groups.push(MetaGroup { title: title.into(), entries });
        }
    }
}

/// Czy plik ma blok EXIF (dowolny kontener obsługiwany przez `kamadak-exif`).
pub fn has_exif(bytes: &[u8]) -> bool {
    exif::Reader::new().read_from_container(&mut Cursor::new(bytes)).is_ok()
}

fn format_name(b: &[u8]) -> &'static str {
    if b.starts_with(&[0xFF, 0xD8]) {
        "JPEG"
    } else if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        "PNG"
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        "WebP"
    } else if crate::imageio::is_avif(b) {
        "AVIF"
    } else if b.starts_with(b"II*\0") || b.starts_with(b"MM\0*") {
        "TIFF"
    } else if b.starts_with(b"BM") {
        "BMP"
    } else if b.len() > 12 && &b[4..8] == b"ftyp" {
        "HEIF"
    } else {
        "?"
    }
}

/// Banki zdjęć i agencje rozpoznawane w polach autora, źródła i praw.
const STOCK: &[(&str, &str)] = &[
    ("shutterstock", "Shutterstock"),
    ("getty", "Getty Images"),
    ("istock", "iStock"),
    ("adobe stock", "Adobe Stock"),
    ("stock.adobe", "Adobe Stock"),
    ("fotolia", "Fotolia / Adobe Stock"),
    ("depositphotos", "Depositphotos"),
    ("dreamstime", "Dreamstime"),
    ("123rf", "123RF"),
    ("alamy", "Alamy"),
    ("pond5", "Pond5"),
    ("bigstock", "Bigstock"),
    ("freepik", "Freepik"),
    ("envato", "Envato"),
    ("canva", "Canva"),
    ("unsplash", "Unsplash"),
    ("pexels", "Pexels"),
    ("pixabay", "Pixabay"),
    ("east news", "East News"),
    ("reuters", "Reuters"),
    ("associated press", "AP"),
    ("pap", "PAP"),
];

/// Generatory i narzędzia AI rozpoznawane w polu „program”.
const AI_TOOLS: &[(&str, &str)] = &[
    ("midjourney", "Midjourney"),
    ("dall-e", "DALL·E"),
    ("dall·e", "DALL·E"),
    ("dalle", "DALL·E"),
    ("openai", "OpenAI"),
    ("chatgpt", "ChatGPT"),
    ("stable diffusion", "Stable Diffusion"),
    ("stablediffusion", "Stable Diffusion"),
    ("comfyui", "ComfyUI"),
    ("automatic1111", "Automatic1111"),
    ("novelai", "NovelAI"),
    ("firefly", "Adobe Firefly"),
    ("imagen", "Google Imagen"),
    ("gemini", "Google Gemini"),
    ("flux", "FLUX"),
    ("leonardo", "Leonardo.Ai"),
    ("ideogram", "Ideogram"),
    ("runway", "Runway"),
    ("bing image creator", "Bing Image Creator"),
    ("copilot", "Microsoft Copilot"),
    ("grok", "Grok"),
    ("recraft", "Recraft"),
];

fn find_known<'a>(text: &str, table: &'a [(&str, &str)]) -> Option<&'a str> {
    let lower = text.to_lowercase();
    table.iter().find(|(needle, _)| {
        // „pap” tylko jako osobne słowo (nie w środku innych wyrazów).
        if needle.len() <= 4 {
            lower.split(|c: char| !c.is_alphanumeric()).any(|w| w == *needle)
        } else {
            lower.contains(needle)
        }
    })
    .map(|(_, name)| *name)
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

fn latin1_or_utf8(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => b.iter().map(|&c| c as char).collect(),
    }
}

fn clean(s: &str) -> String {
    let s: String = s.chars().map(|c| if c.is_control() && c != '\n' { ' ' } else { c }).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() > 400 {
        format!("{}…", s.chars().take(400).collect::<String>())
    } else {
        s
    }
}

/// Czyta metadane z bajtów pliku.
pub fn read_metadata(bytes: &[u8]) -> MetaReport {
    let mut r = MetaReport { format: format_name(bytes), ..Default::default() };
    // Zebrane wartości do wniosków (prawa, AI, bank zdjęć).
    let mut rights: Vec<String> = Vec::new();
    let mut people: Vec<String> = Vec::new();
    let mut software: Vec<String> = Vec::new();

    read_exif(bytes, &mut r, &mut rights, &mut people, &mut software);
    if r.format == "JPEG" {
        read_jpeg_segments(bytes, &mut r, &mut rights, &mut people);
    }
    if r.format == "PNG" {
        read_png_chunks(bytes, &mut r, &mut software);
    }
    if let Some(xmp) = extract_xmp(bytes) {
        read_xmp(&xmp, &mut r, &mut rights, &mut people, &mut software);
    }
    if find(bytes, b"jumb").is_some() && find(bytes, b"c2pa").is_some() {
        // „trainedAlgorithmicMedia” i „compositeWithTrainedAlgorithmicMedia”.
        let ai = find(bytes, b"rainedAlgorithmicMedia").is_some();
        r.notice(
            NoticeKind::Info,
            "Content Credentials (C2PA) – podpisany zapis pochodzenia i edycji (aparaty, Adobe, generatory AI)",
        );
        if ai {
            r.notice(NoticeKind::Ai, "C2PA: obraz oznaczony jako wygenerowany przez AI (trainedAlgorithmicMedia)");
        }
    }

    // Wnioski.
    for text in rights.iter().chain(people.iter()) {
        if let Some(stock) = find_known(text, STOCK) {
            r.notice(NoticeKind::Rights, format!("Wygląda na zdjęcie z banku zdjęć / agencji: {stock} – sprawdź licencję"));
        }
    }
    if let Some(first) = rights.first() {
        r.notice(NoticeKind::Rights, format!("Prawa / licencja: {first}"));
    }
    for tool in &software {
        if let Some(ai) = find_known(tool, AI_TOOLS) {
            r.notice(NoticeKind::Ai, format!("Program wskazuje na AI: {ai} ({tool})"));
        }
    }
    if let Some(p) = people.first() {
        r.notice(NoticeKind::Personal, format!("Autor / właściciel: {p}"));
    }
    if r.is_empty() && r.notices.is_empty() {
        r.notice(NoticeKind::Info, "Brak metadanych – plik jest „czysty”");
    }
    // Kolejność: najważniejsze na górze.
    r.notices.sort_by_key(|n| match n.kind {
        NoticeKind::Rights => 0,
        NoticeKind::Ai => 1,
        NoticeKind::Location => 2,
        NoticeKind::Personal => 3,
        NoticeKind::Info => 4,
    });
    r
}

// ---- EXIF -----------------------------------------------------------------------------------

fn exif_label(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "Make" => "Producent aparatu",
        "Model" => "Model aparatu",
        "LensModel" => "Obiektyw",
        "LensMake" => "Producent obiektywu",
        "Software" => "Program",
        "DateTimeOriginal" => "Data zrobienia",
        "DateTime" => "Data zmiany",
        "Artist" => "Autor",
        "Copyright" => "Prawa autorskie",
        "ImageDescription" => "Opis",
        "UserComment" => "Komentarz",
        "BodySerialNumber" => "Numer seryjny aparatu",
        "LensSerialNumber" => "Numer seryjny obiektywu",
        "CameraOwnerName" => "Właściciel aparatu",
        "Orientation" => "Orientacja",
        "PixelXDimension" => "Szerokość (EXIF)",
        "PixelYDimension" => "Wysokość (EXIF)",
        "ExposureTime" => "Czas naświetlania",
        "FNumber" => "Przysłona",
        "PhotographicSensitivity" => "ISO",
        "FocalLength" => "Ogniskowa",
        "ColorSpace" => "Przestrzeń barw",
        "ImageUniqueID" => "ID zdjęcia",
        _ => return None,
    })
}

fn gps_decimal(exif: &exif::Exif, value: exif::Tag, reference: exif::Tag) -> Option<f64> {
    let field = exif.get_field(value, exif::In::PRIMARY)?;
    let exif::Value::Rational(ref v) = field.value else { return None };
    if v.len() < 3 {
        return None;
    }
    let deg = v[0].to_f64() + v[1].to_f64() / 60.0 + v[2].to_f64() / 3600.0;
    let negative = exif
        .get_field(reference, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string())
        .is_some_and(|s| s.contains('S') || s.contains('W'));
    Some(if negative { -deg } else { deg })
}

fn read_exif(bytes: &[u8], r: &mut MetaReport, rights: &mut Vec<String>, people: &mut Vec<String>, software: &mut Vec<String>) {
    let Ok(exif) = exif::Reader::new().read_from_container(&mut Cursor::new(bytes)) else { return };
    let mut entries = Vec::new();
    for f in exif.fields() {
        if f.ifd_num != exif::In::PRIMARY || f.tag == exif::Tag::MakerNote {
            continue;
        }
        let name = f.tag.to_string();
        let value = clean(f.display_value().with_unit(&exif).to_string().trim_matches('"'));
        if value.is_empty() {
            continue;
        }
        match name.as_str() {
            "Copyright" => rights.push(value.clone()),
            "Artist" | "CameraOwnerName" => people.push(value.clone()),
            "Software" => software.push(value.clone()),
            "BodySerialNumber" => r.notice(NoticeKind::Personal, format!("Numer seryjny aparatu: {value}")),
            _ => {}
        }
        let label = exif_label(&name).map_or(name.clone(), |l| format!("{l} ({name})"));
        entries.push((label, value));
    }
    if let (Some(lat), Some(lon)) = (
        gps_decimal(&exif, exif::Tag::GPSLatitude, exif::Tag::GPSLatitudeRef),
        gps_decimal(&exif, exif::Tag::GPSLongitude, exif::Tag::GPSLongitudeRef),
    ) {
        r.notice(NoticeKind::Location, format!("Lokalizacja GPS: {lat:.5}, {lon:.5}"));
        entries.insert(0, ("Lokalizacja GPS".into(), format!("{lat:.6}, {lon:.6}")));
    }
    let camera: Vec<String> = ["Make", "Model"]
        .iter()
        .filter_map(|t| exif.fields().find(|f| f.tag.to_string() == *t && f.ifd_num == exif::In::PRIMARY))
        .map(|f| clean(f.display_value().to_string().trim_matches('"')))
        .collect();
    if !camera.is_empty() {
        r.notice(NoticeKind::Info, format!("Aparat: {}", camera.join(" ")));
    }
    r.group("EXIF", entries);
}

// ---- JPEG: IPTC, komentarze, ICC, MPF -------------------------------------------------------

fn read_jpeg_segments(b: &[u8], r: &mut MetaReport, rights: &mut Vec<String>, people: &mut Vec<String>) {
    let mut i = 2;
    let mut other = Vec::new();
    while i + 4 <= b.len() && b[i] == 0xFF {
        let marker = b[i + 1];
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 || marker == 0xFF {
            i += if marker == 0xFF { 1 } else { 2 };
            continue;
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        let end = (i + 2 + len).min(b.len());
        let data = &b[(i + 4).min(end)..end];
        match marker {
            0xED if data.starts_with(b"Photoshop 3.0\0") => read_photoshop_irb(&data[14..], r, rights, people),
            0xE2 if data.starts_with(b"ICC_PROFILE\0") => {
                r.notice(NoticeKind::Info, "Profil kolorów ICC (eksport zapisuje sRGB bez profilu)");
            }
            0xE2 if data.starts_with(b"MPF\0") => other.push(("MPF".into(), "dodatkowe obrazy (np. podgląd, głębia)".into())),
            0xEB => {} // JUMBF / C2PA – sprawdzane w całym pliku
            0xFE => other.push(("Komentarz JPEG".into(), clean(&latin1_or_utf8(data)))),
            0xEC if data.starts_with(b"Ducky") => other.push(("Ducky".into(), "„Zapisz dla Web” z Photoshopa".into())),
            0xEE if data.starts_with(b"Adobe") => other.push(("Adobe".into(), "znacznik Adobe (Photoshop / Illustrator)".into())),
            _ => {}
        }
        i = end;
    }
    r.group("Inne (JPEG)", other);
}

fn iptc_label(dataset: u8) -> Option<&'static str> {
    Some(match dataset {
        5 => "Tytuł",
        25 => "Słowa kluczowe",
        40 => "Instrukcje",
        55 => "Data utworzenia",
        80 => "Autor (By-line)",
        85 => "Stanowisko autora",
        90 => "Miasto",
        95 => "Region",
        101 => "Kraj",
        103 => "Numer zlecenia",
        105 => "Nagłówek",
        110 => "Kredyt (Credit)",
        115 => "Źródło (Source)",
        116 => "Prawa autorskie (Copyright Notice)",
        118 => "Kontakt",
        120 => "Opis (Caption)",
        122 => "Autor opisu",
        _ => return None,
    })
}

fn read_photoshop_irb(mut d: &[u8], r: &mut MetaReport, rights: &mut Vec<String>, people: &mut Vec<String>) {
    let mut entries = Vec::new();
    while d.len() >= 12 && d.starts_with(b"8BIM") {
        let id = u16::from_be_bytes([d[4], d[5]]);
        let name_len = d[6] as usize;
        let mut p = 7 + name_len;
        if p % 2 == 1 {
            p += 1;
        }
        if p + 4 > d.len() {
            break;
        }
        let size = u32::from_be_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]]) as usize;
        let start = p + 4;
        let end = (start + size).min(d.len());
        if id == 0x0404 {
            let mut iim = &d[start..end];
            while iim.len() >= 5 && iim[0] == 0x1C {
                let (record, dataset) = (iim[1], iim[2]);
                let len = u16::from_be_bytes([iim[3], iim[4]]) as usize;
                if len & 0x8000 != 0 || 5 + len > iim.len() {
                    break;
                }
                let value = clean(&latin1_or_utf8(&iim[5..5 + len]));
                if record == 2 && !value.is_empty() {
                    match dataset {
                        110 | 115 | 116 => rights.push(value.clone()),
                        80 => people.push(value.clone()),
                        _ => {}
                    }
                    if let Some(label) = iptc_label(dataset) {
                        entries.push((label.to_string(), value));
                    }
                }
                iim = &iim[5 + len..];
            }
        }
        let next = end + (size % 2);
        if next >= d.len() {
            break;
        }
        d = &d[next..];
    }
    r.group("IPTC", entries);
}

// ---- PNG --------------------------------------------------------------------------------------

fn inflate(data: &[u8]) -> Option<String> {
    miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(data, 4 << 20).ok().map(|v| latin1_or_utf8(&v))
}

fn read_png_chunks(b: &[u8], r: &mut MetaReport, software: &mut Vec<String>) {
    let mut i = 8;
    let mut entries = Vec::new();
    while i + 12 <= b.len() {
        let len = u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
        let kind = &b[i + 4..i + 8];
        let start = i + 8;
        let end = start.saturating_add(len).min(b.len());
        let data = &b[start..end];
        let text = match kind {
            b"tEXt" => data.iter().position(|&c| c == 0).map(|z| (latin1_or_utf8(&data[..z]), latin1_or_utf8(&data[z + 1..]))),
            b"zTXt" => data.iter().position(|&c| c == 0).and_then(|z| {
                inflate(data.get(z + 2..)?).map(|t| (latin1_or_utf8(&data[..z]), t))
            }),
            b"iTXt" => data.iter().position(|&c| c == 0).and_then(|z| {
                let key = latin1_or_utf8(&data[..z]);
                let compressed = *data.get(z + 1)? == 1;
                let rest = data.get(z + 3..)?;
                let lang_end = rest.iter().position(|&c| c == 0)?;
                let rest = &rest[lang_end + 1..];
                let tkey_end = rest.iter().position(|&c| c == 0)?;
                let body = &rest[tkey_end + 1..];
                let text = if compressed { inflate(body)? } else { latin1_or_utf8(body) };
                Some((key, text))
            }),
            b"iCCP" => {
                r.notice(NoticeKind::Info, "Profil kolorów ICC (eksport zapisuje sRGB bez profilu)");
                None
            }
            b"IEND" => break,
            _ => None,
        };
        if let Some((key, value)) = text
            && key != "XML:com.adobe.xmp"
        {
            let k = key.to_lowercase();
            if ["parameters", "prompt", "workflow", "negative_prompt", "sd-metadata", "invokeai_metadata", "dream"]
                .contains(&k.as_str())
            {
                r.notice(NoticeKind::Ai, format!("PNG zawiera parametry generowania AI („{key}”)"));
            }
            if k == "software" {
                software.push(value.clone());
            }
            entries.push((key, clean(&value)));
        }
        i = end + 4; // CRC
    }
    r.group("Teksty PNG", entries);
}

// ---- XMP --------------------------------------------------------------------------------------

/// Pakiet XMP z dowolnego kontenera (JPEG APP1, PNG iTXt, WebP, AVIF, TIFF) – po znaczniku `x:xmpmeta`.
pub fn extract_xmp(b: &[u8]) -> Option<String> {
    let start = find(b, b"<x:xmpmeta").or_else(|| find(b, b"<x:xapmeta"))?;
    let rest = &b[start..];
    let end = find(rest, b"</x:xmpmeta>").map(|e| e + 12).or_else(|| find(rest, b"</x:xapmeta>").map(|e| e + 12))?;
    Some(String::from_utf8_lossy(&rest[..end]).into_owned())
}

fn xml_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        let tail = &rest[p..];
        let Some(semi) = tail.find(';').filter(|&i| i < 10) else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let ent = &tail[1..semi];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => out.push(c),
            None => out.push_str(&tail[..=semi]),
        }
        rest = &tail[semi + 1..];
    }
    out.push_str(rest);
    out
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// Wartość właściwości XMP: atrybut `prop="…"` albo element `<prop>…</prop>` (z rdf:Alt/Seq/Bag).
fn xmp_prop(xmp: &str, prop: &str) -> Option<String> {
    let attr = format!("{prop}=\"");
    if let Some(p) = xmp.find(&attr) {
        let v = &xmp[p + attr.len()..];
        let v = &v[..v.find('"')?];
        return Some(clean(&xml_unescape(v))).filter(|s| !s.is_empty());
    }
    let open = format!("<{prop}");
    let close = format!("</{prop}>");
    let p = xmp.find(&open)?;
    let after = &xmp[p + open.len()..];
    // Musi to być ten element (np. „<dc:rights>”, a nie „<dc:rightsHolder>”).
    if !after.starts_with(['>', ' ', '\n', '\r', '\t', '/']) {
        return None;
    }
    let gt = after.find('>')?;
    if after[..gt].ends_with('/') {
        return None;
    }
    let body = &after[gt + 1..];
    let body = &body[..body.find(&close)?];
    let items: Vec<String> = strip_tags(body).split("  ").map(|s| clean(&xml_unescape(s))).filter(|s| !s.is_empty()).collect();
    Some(items.join(", ")).filter(|s| !s.is_empty())
}

const XMP_PROPS: &[(&str, &str)] = &[
    ("dc:title", "Tytuł"),
    ("dc:description", "Opis"),
    ("dc:creator", "Autor"),
    ("dc:rights", "Prawa autorskie"),
    ("xmpRights:UsageTerms", "Warunki użycia"),
    ("xmpRights:WebStatement", "Strona z licencją"),
    ("xmpRights:Marked", "Zastrzeżone (Marked)"),
    ("xmpRights:Owner", "Właściciel praw"),
    ("plus:LicensorName", "Licencjodawca"),
    ("plus:LicensorURL", "Licencjodawca – strona"),
    ("photoshop:Credit", "Kredyt (Credit)"),
    ("photoshop:Source", "Źródło (Source)"),
    ("photoshop:AuthorsPosition", "Stanowisko autora"),
    ("photoshop:Headline", "Nagłówek"),
    ("photoshop:City", "Miasto"),
    ("photoshop:Country", "Kraj"),
    ("Iptc4xmpCore:Location", "Miejsce"),
    ("Iptc4xmpExt:DigitalSourceType", "Rodzaj źródła cyfrowego"),
    ("Iptc4xmpExt:AISystemUsed", "System AI"),
    ("Iptc4xmpExt:AIPromptInformation", "Prompt AI"),
    ("xmp:CreatorTool", "Program"),
    ("xmp:CreateDate", "Utworzono"),
    ("xmp:ModifyDate", "Zmieniono"),
    ("xmp:MetadataDate", "Metadane zmienione"),
    ("xmpMM:DocumentID", "ID dokumentu"),
    ("xmpMM:OriginalDocumentID", "ID oryginału"),
    ("aux:SerialNumber", "Numer seryjny aparatu"),
    ("exifEX:BodySerialNumber", "Numer seryjny aparatu"),
    ("exif:GPSLatitude", "GPS – szerokość"),
    ("exif:GPSLongitude", "GPS – długość"),
    ("dc:subject", "Słowa kluczowe"),
];

fn read_xmp(xmp: &str, r: &mut MetaReport, rights: &mut Vec<String>, people: &mut Vec<String>, software: &mut Vec<String>) {
    let mut entries = Vec::new();
    for (prop, label) in XMP_PROPS {
        let Some(value) = xmp_prop(xmp, prop) else { continue };
        match *prop {
            "dc:rights" | "xmpRights:UsageTerms" | "xmpRights:WebStatement" | "xmpRights:Owner" | "plus:LicensorName"
            | "plus:LicensorURL" | "photoshop:Credit" | "photoshop:Source" => rights.push(value.clone()),
            "dc:creator" => people.push(value.clone()),
            "xmp:CreatorTool" | "Iptc4xmpExt:AISystemUsed" => software.push(value.clone()),
            "xmpRights:Marked" if value.eq_ignore_ascii_case("true") => {
                r.notice(NoticeKind::Rights, "Oznaczone jako zastrzeżone prawem autorskim (xmpRights:Marked)");
            }
            "Iptc4xmpExt:DigitalSourceType" if value.to_lowercase().contains("trainedalgorithmicmedia") => {
                r.notice(NoticeKind::Ai, format!("IPTC: obraz wygenerowany lub zmieniony przez AI ({})", value.rsplit('/').next().unwrap_or(&value)));
            }
            "exif:GPSLatitude" => r.notice(NoticeKind::Location, "Lokalizacja GPS w XMP"),
            "aux:SerialNumber" | "exifEX:BodySerialNumber" => r.notice(NoticeKind::Personal, format!("Numer seryjny aparatu: {value}")),
            _ => {}
        }
        entries.push((label.to_string(), value));
    }
    // Historia edycji (xmpMM:History) – programy, którymi zapisywano plik.
    let mut agents: Vec<String> = Vec::new();
    let mut rest = xmp;
    while let Some(p) = rest.find("stEvt:softwareAgent") {
        rest = &rest[p + 19..];
        let v = if let Some(v) = rest.strip_prefix("=\"") {
            v.split('"').next()
        } else {
            rest.strip_prefix('>').and_then(|v| v.split('<').next())
        };
        if let Some(v) = v.map(|v| clean(&xml_unescape(v))).filter(|v| !v.is_empty())
            && !agents.contains(&v)
        {
            agents.push(v);
        }
    }
    if !agents.is_empty() {
        software.extend(agents.iter().cloned());
        entries.push(("Historia – programy".into(), agents.join(", ")));
    }
    if !entries.is_empty() {
        entries.push(("Rozmiar XMP".into(), format!("{} B", xmp.len())));
    }
    r.group("XMP", entries);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xmp_atrybut_i_element() {
        let xmp = r#"<x:xmpmeta><rdf:RDF><rdf:Description xmp:CreatorTool="Adobe Photoshop 26.0 (Windows)"
            xmpRights:Marked="True" photoshop:Credit="Getty Images/iStockphoto">
            <dc:rights><rdf:Alt><rdf:li xml:lang="x-default">© Jan &amp; Spółka</rdf:li></rdf:Alt></dc:rights>
            <dc:creator><rdf:Seq><rdf:li>Anna</rdf:li><rdf:li>Piotr</rdf:li></rdf:Seq></dc:creator>
            <Iptc4xmpExt:DigitalSourceType>http://cv.iptc.org/newscodes/digitalsourcetype/compositeWithTrainedAlgorithmicMedia</Iptc4xmpExt:DigitalSourceType>
            </rdf:Description></rdf:RDF></x:xmpmeta>"#;
        assert_eq!(xmp_prop(xmp, "dc:rights").as_deref(), Some("© Jan & Spółka"));
        assert_eq!(xmp_prop(xmp, "dc:creator").as_deref(), Some("Anna, Piotr"));
        assert_eq!(xmp_prop(xmp, "xmp:CreatorTool").as_deref(), Some("Adobe Photoshop 26.0 (Windows)"));

        let mut bytes = vec![0xFF, 0xD8];
        bytes.extend_from_slice(xmp.as_bytes());
        let r = read_metadata(&bytes);
        let kinds: Vec<_> = r.notices.iter().map(|n| n.kind).collect();
        assert!(kinds.contains(&NoticeKind::Rights));
        assert!(kinds.contains(&NoticeKind::Ai));
        assert!(r.notices.iter().any(|n| n.text.contains("Getty")), "{:?}", r.notices);
        assert!(r.notices.iter().any(|n| n.text.contains("zastrzeżone")));
    }

    #[test]
    fn iptc_w_jpeg() {
        // APP13: Photoshop IRB z IIM 2:116 (Copyright) i 2:80 (By-line).
        let mut iim = Vec::new();
        for (ds, val) in [(116u8, "Shutterstock"), (80, "Jan Kowalski")] {
            iim.extend_from_slice(&[0x1C, 2, ds]);
            iim.extend_from_slice(&(val.len() as u16).to_be_bytes());
            iim.extend_from_slice(val.as_bytes());
        }
        let mut irb = b"Photoshop 3.0\0".to_vec();
        irb.extend_from_slice(b"8BIM\x04\x04\0\0");
        irb.extend_from_slice(&(iim.len() as u32).to_be_bytes());
        irb.extend_from_slice(&iim);
        if iim.len() % 2 == 1 {
            irb.push(0);
        }
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xED];
        jpeg.extend_from_slice(&((irb.len() + 2) as u16).to_be_bytes());
        jpeg.extend_from_slice(&irb);
        jpeg.extend_from_slice(&[0xFF, 0xD9]);
        let r = read_metadata(&jpeg);
        let iptc = r.groups.iter().find(|g| g.title == "IPTC").expect("grupa IPTC");
        assert!(iptc.entries.iter().any(|(k, v)| k.contains("Copyright") && v == "Shutterstock"));
        assert!(r.notices.iter().any(|n| n.kind == NoticeKind::Rights && n.text.contains("Shutterstock")));
        assert!(r.notices.iter().any(|n| n.kind == NoticeKind::Personal && n.text.contains("Jan Kowalski")));
    }

    #[test]
    fn png_parametry_ai() {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let data = b"parameters\0a cat, Steps: 20, Sampler: Euler";
        png.extend_from_slice(&(data.len() as u32).to_be_bytes());
        png.extend_from_slice(b"tEXt");
        png.extend_from_slice(data);
        png.extend_from_slice(&[0; 4]);
        png.extend_from_slice(&[0, 0, 0, 0]);
        png.extend_from_slice(b"IEND");
        png.extend_from_slice(&[0; 4]);
        let r = read_metadata(&png);
        assert_eq!(r.format, "PNG");
        assert!(r.notices.iter().any(|n| n.kind == NoticeKind::Ai));
    }

    #[test]
    fn exif_gps_prawa_numer_seryjny() {
        use exif::experimental::Writer;
        use exif::{Field, In, Rational, Tag, Value};
        let ascii = |s: &str| Value::Ascii(vec![s.as_bytes().to_vec()]);
        let dms = |d: u32, m: u32, s100: u32| {
            Value::Rational(vec![Rational { num: d, denom: 1 }, Rational { num: m, denom: 1 }, Rational { num: s100, denom: 100 }])
        };
        let fields = [
            Field { tag: Tag::Copyright, ifd_num: In::PRIMARY, value: ascii("(c) Getty Images") },
            Field { tag: Tag::Artist, ifd_num: In::PRIMARY, value: ascii("Anna Nowak") },
            Field { tag: Tag::Software, ifd_num: In::PRIMARY, value: ascii("Midjourney v7") },
            Field { tag: Tag::BodySerialNumber, ifd_num: In::PRIMARY, value: ascii("SN-42") },
            Field { tag: Tag::GPSLatitudeRef, ifd_num: In::PRIMARY, value: ascii("N") },
            Field { tag: Tag::GPSLatitude, ifd_num: In::PRIMARY, value: dms(52, 13, 4692) },
            Field { tag: Tag::GPSLongitudeRef, ifd_num: In::PRIMARY, value: ascii("W") },
            Field { tag: Tag::GPSLongitude, ifd_num: In::PRIMARY, value: dms(21, 0, 4392) },
        ];
        let mut w = Writer::new();
        for f in &fields {
            w.push_field(f);
        }
        let mut tiff = Cursor::new(Vec::new());
        w.write(&mut tiff, false).unwrap();
        let tiff = tiff.into_inner();
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
        jpeg.extend_from_slice(&((tiff.len() + 8) as u16).to_be_bytes());
        jpeg.extend_from_slice(b"Exif\0\0");
        jpeg.extend_from_slice(&tiff);
        jpeg.extend_from_slice(&[0xFF, 0xD9]);

        assert!(has_exif(&jpeg));
        let r = read_metadata(&jpeg);
        let has = |kind: NoticeKind, needle: &str| r.notices.iter().any(|n| n.kind == kind && n.text.contains(needle));
        assert!(has(NoticeKind::Rights, "Getty Images"), "{:?}", r.notices);
        assert!(has(NoticeKind::Ai, "Midjourney"), "{:?}", r.notices);
        assert!(has(NoticeKind::Personal, "SN-42"), "{:?}", r.notices);
        assert!(has(NoticeKind::Personal, "Anna Nowak"), "{:?}", r.notices);
        // 52°13'46.92" N = 52.22970, 21°0'43.92" W = -21.01220
        assert!(has(NoticeKind::Location, "52.22970, -21.01220"), "{:?}", r.notices);
        assert_eq!(r.notices[0].kind, NoticeKind::Rights);
    }

    #[test]
    fn czysty_plik() {
        let r = read_metadata(&[0xFF, 0xD8, 0xFF, 0xD9]);
        assert!(r.is_empty());
        assert!(r.notices[0].text.contains("Brak metadanych"));
    }

    #[test]
    fn pap_tylko_jako_slowo() {
        assert_eq!(find_known("Foto: PAP/Jan", STOCK), Some("PAP"));
        assert_eq!(find_known("papier", STOCK), None);
    }
}

/// Podgląd raportu dla pliku: `ASONE_META_FILE=zdjęcie.jpg cargo test -p asone-core metadane_pliku -- --ignored --nocapture`.
#[cfg(test)]
#[test]
#[ignore]
fn metadane_pliku() {
    let path = std::env::var("ASONE_META_FILE").expect("ASONE_META_FILE");
    let r = read_metadata(&std::fs::read(path).unwrap());
    eprintln!("format: {}", r.format);
    for n in &r.notices {
        eprintln!("[{:?}] {}", n.kind, n.text);
    }
    for g in &r.groups {
        eprintln!("== {}", g.title);
        for (k, v) in &g.entries {
            eprintln!("  {k}: {v}");
        }
    }
}
