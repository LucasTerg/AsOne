//! Edycja zdjęcia poleceniem przez Replicate (FLUX.1 Kontext Pro / Max).
//!
//! Zdjęcie (JPEG jako data URI, do ~1 MB) i polecenie idą do
//! `POST https://api.replicate.com/v1/models/{model}/predictions` z `Prefer: wait`; gdy model
//! nie skończy w minutę, odpytujemy `urls.get` co 2 s. Wynik to adres obrazu na
//! `replicate.delivery` (pobierany od razu – Replicate kasuje dane zapytań z API po godzinie).
//! Płatne za każdy obraz z kredytu konta Replicate; klucz API jest tylko lokalnie.

use std::time::{Duration, Instant};

use base64::Engine as _;
use image::RgbaImage;
use serde_json::Value;

pub const API: &str = "https://api.replicate.com/v1";
/// Dłuższy bok wysyłanego zdjęcia (FLUX Kontext i tak tworzy obraz ok. 1 Mpx).
const UPLOAD_SIDE: u32 = 1536;
/// Replicate zaleca data URI do ok. 1 MB.
const MAX_DATA_URI: usize = 1_000_000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_RESPONSE: u64 = 64 * 1024 * 1024;

/// Edytuje zdjęcie według polecenia; zwraca nowy obraz (rozmiar wybiera model, ok. 1 Mpx).
pub fn edit_image(image: &RgbaImage, prompt: &str, model: &str, key: &str) -> Result<RgbaImage, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("Brak klucza Replicate – Ustawienia › Preferencje › Replicate".into());
    }
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err("Wpisz polecenie dla AI (co zrobić ze zdjęciem)".into());
    }
    let body = serde_json::json!({
        "input": {
            "prompt": prompt,
            "input_image": jpeg_data_uri(image)?,
            "aspect_ratio": "match_input_image",
            "output_format": "png",
            "safety_tolerance": 2,
        }
    })
    .to_string();
    let mut prediction = send(Some(&body), &format!("{API}/models/{model}/predictions"), key)?;
    let started = Instant::now();
    loop {
        match prediction.get("status").and_then(Value::as_str) {
            Some("succeeded") => break,
            Some("failed") | Some("canceled") | Some("aborted") => {
                let why = prediction.get("error").and_then(Value::as_str).unwrap_or("model zgłosił błąd");
                return Err(format!("Replicate: {why}"));
            }
            _ => {}
        }
        if started.elapsed() > TOTAL_TIMEOUT {
            return Err("Replicate: model nie skończył w 5 minut".into());
        }
        std::thread::sleep(Duration::from_secs(2));
        let url = prediction
            .pointer("/urls/get")
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| prediction.get("id").and_then(Value::as_str).map(|id| format!("{API}/predictions/{id}")))
            .ok_or("Replicate: odpowiedź bez identyfikatora zadania")?;
        prediction = send(None, &url, key)?;
    }
    let output = match prediction.get("output") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a.first().and_then(Value::as_str).map(String::from).ok_or("Replicate: pusty wynik")?,
        _ => return Err("Replicate: odpowiedź bez obrazu".into()),
    };
    let bytes = download(&output, key)?;
    image::load_from_memory(&bytes)
        .map(|img| img.to_rgba8())
        .map_err(|e| format!("Replicate: nieczytelny wynik ({e})"))
}

/// POST (z `body`) albo GET zadania; zwraca JSON odpowiedzi albo czytelny błąd.
fn send(body: Option<&str>, url: &str, key: &str) -> Result<Value, String> {
    let auth = format!("Bearer {key}");
    let result = match body {
        Some(body) => ureq::post(url)
            .config()
            .http_status_as_error(false)
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build()
            .header("Authorization", &auth)
            .header("Content-Type", "application/json")
            .header("Prefer", "wait=60")
            .send(body.as_bytes()),
        None => ureq::get(url)
            .config()
            .http_status_as_error(false)
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build()
            .header("Authorization", &auth)
            .call(),
    };
    let mut resp = result.map_err(|e| format!("Replicate: brak połączenia ({e})"))?;
    let status = resp.status().as_u16();
    let text = resp
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE)
        .read_to_string()
        .map_err(|e| format!("Replicate: przerwana odpowiedź ({e})"))?;
    if !(200..300).contains(&status) {
        return Err(error_message(status, &text));
    }
    serde_json::from_str(&text).map_err(|e| format!("Replicate: nieczytelna odpowiedź ({e})"))
}

fn download(url: &str, key: &str) -> Result<Vec<u8>, String> {
    let mut resp = ureq::get(url)
        .config()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build()
        .header("Authorization", &format!("Bearer {key}"))
        .call()
        .map_err(|e| format!("Replicate: pobieranie wyniku ({e})"))?;
    resp.body_mut()
        .with_config()
        .limit(MAX_RESPONSE)
        .read_to_vec()
        .map_err(|e| format!("Replicate: pobieranie wyniku ({e})"))
}

/// Zdjęcie jako `data:image/jpeg;base64,…` mieszczące się w [`MAX_DATA_URI`]
/// (w razie potrzeby niższa jakość, potem mniejszy obraz).
fn jpeg_data_uri(image: &RgbaImage) -> Result<String, String> {
    use asone_core::imageio::{fit_within, lanczos3, resize_rgba};
    let (w, h) = image.dimensions();
    let mut side = UPLOAD_SIDE.min(w.max(h));
    loop {
        let size = fit_within([w, h], side);
        let scaled = resize_rgba(image, size, lanczos3())?;
        let mut rgb = Vec::with_capacity(scaled.as_raw().len() / 4 * 3);
        for p in scaled.pixels() {
            let a = p[3] as u32;
            for c in 0..3 {
                rgb.push(((p[c] as u32 * a + 255 * (255 - a) + 127) / 255) as u8);
            }
        }
        for quality in [90u8, 80, 70] {
            let mut jpeg = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality)
                .encode(&rgb, size[0], size[1], image::ExtendedColorType::Rgb8)
                .map_err(|e| format!("Replicate: kodowanie zdjęcia ({e})"))?;
            let uri = format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(&jpeg));
            if uri.len() <= MAX_DATA_URI {
                return Ok(uri);
            }
        }
        if side <= 512 {
            return Err("Replicate: zdjęcie nie mieści się w limicie przesyłania".into());
        }
        side = side * 4 / 5;
    }
}

/// Komunikat błędu z kodu HTTP i pól `detail` / `title` odpowiedzi.
fn error_message(status: u16, body: &str) -> String {
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("detail").or_else(|| v.get("title")).and_then(Value::as_str).map(String::from))
        .unwrap_or_else(|| body.trim().to_string());
    let detail: String = detail.chars().take(300).collect();
    match status {
        401 => "Replicate: nieprawidłowy klucz API (401)".into(),
        402 => format!("Replicate: brak środków na koncie (402) – doładuj kredyt na replicate.com/account/billing. {detail}"),
        422 => format!("Replicate: odrzucone dane (422). {detail}"),
        429 => "Replicate: za dużo zapytań naraz (429) – spróbuj za chwilę".into(),
        _ => format!("Replicate: błąd {status}. {detail}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn komunikaty_bledow() {
        assert!(error_message(401, "{}").contains("klucz"));
        assert!(error_message(402, r#"{"title":"Insufficient credit"}"#).contains("Insufficient credit"));
        assert!(error_message(422, r#"{"detail":"- input.prompt: Invalid type"}"#).contains("input.prompt"));
    }

    #[test]
    fn duze_zdjecie_miesci_sie_w_limicie() {
        // Szum słabo się kompresuje – wymusza niższą jakość / mniejszy obraz.
        let mut x = 0x1234_5678u32;
        let img = RgbaImage::from_fn(3000, 2000, |_, _| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            image::Rgba([(x >> 24) as u8, (x >> 16) as u8, (x >> 8) as u8, 255])
        });
        let uri = jpeg_data_uri(&img).unwrap();
        assert!(uri.len() <= MAX_DATA_URI && uri.starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn bez_klucza_i_bez_polecenia() {
        let img = RgbaImage::new(8, 8);
        assert!(edit_image(&img, "x", "a/b", " ").unwrap_err().contains("klucza"));
        assert!(edit_image(&img, "  ", "a/b", "r8_x").unwrap_err().contains("polecenie"));
    }

    /// Prawdziwa edycja (płatna, ok. 0,04 USD): klucz z `~/.config/asone/replicate-token`.
    /// `cargo test -p asone replicate_na_zywo -- --ignored --nocapture`
    #[test]
    #[ignore = "płatne zapytanie do Replicate"]
    fn replicate_na_zywo() {
        let key = crate::settings::Settings::default().replicate_api_key();
        let sample = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sample/produkt-1.jpg");
        let img = asone_core::imageio::decode_path(&sample).unwrap();
        let t = Instant::now();
        let out = edit_image(&img, "Make the mug blue. Keep everything else the same.", "black-forest-labs/flux-kontext-pro", &key)
            .unwrap();
        eprintln!("Replicate: {:?}, wynik {:?}", t.elapsed(), out.dimensions());
        if let Ok(path) = std::env::var("ASONE_AI_OUT") {
            out.save(path).unwrap();
        }
        assert!(out.width() > 100 && out.height() > 100);
    }
}
