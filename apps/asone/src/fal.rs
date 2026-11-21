//! RMBG-3.0 online przez fal.ai (endpoint `bria/rmbg-3`).
//!
//! Zdjęcie (JPEG, dłuższy bok ≤ [`UPLOAD_SIDE`]) idzie jako data URI do [`ENDPOINT`]; wraca PNG
//! z kanałem alfa w rozmiarze wejścia – alfa staje się maską (wątek AI skaluje ją do zdjęcia).
//! Płatne za każde zdjęcie z przedpłaconych kredytów konta fal.ai; klucz API jest tylko lokalnie.
//! Licencja na fal.ai: „Commercial use” (model trenowany na licencjonowanych danych) – w odróżnieniu
//! od lokalnych RMBG-1.4/2.0 (CC BY-NC) także do zastosowań komercyjnych.
//! `sync_mode` – wynik wraca w odpowiedzi i nie zostaje w historii zapytań fal.ai.
//!
//! Desktop i Android: zapytanie blokujące (`ureq`) w wątku AI. Przeglądarka: `fetch` (fal.ai
//! pozwala na zapytania z przeglądarek – CORS), [`remove_background_async`].

#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use base64::Engine as _;
use image::{GrayImage, RgbaImage};

/// Model BRIA RMBG-3.0 na fal.ai (zapytanie synchroniczne).
pub const ENDPOINT: &str = "https://fal.run/bria/rmbg-3";
/// Dłuższy bok wysyłanego zdjęcia: drobne detale (włosy, futro) bez wysyłania pełnych megapikseli.
pub const UPLOAD_SIDE: u32 = crate::ai::FAL_UPLOAD_SIDE;
#[cfg(not(target_arch = "wasm32"))]
const TIMEOUT: Duration = Duration::from_secs(180);
/// Odpowiedź z PNG w data URI bywa duża (kilka–kilkanaście MB).
#[cfg(not(target_arch = "wasm32"))]
const MAX_RESPONSE: u64 = 128 * 1024 * 1024;

/// Treść zapytania (JSON) albo błąd braku klucza.
fn request_body(image: &RgbaImage, key: &str) -> Result<String, String> {
    if key.trim().is_empty() {
        return Err(t!("Brak klucza fal.ai – wpisz go w Ustawienia › Preferencje › Usuwanie tła (AI)").into());
    }
    Ok(serde_json::json!({ "image_url": jpeg_data_uri(image)?, "sync_mode": true }).to_string())
}

/// Adres wyniku z odpowiedzi (albo komunikat błędu z kodu HTTP).
fn output_url(status: u16, text: &str) -> Result<String, String> {
    if status != 200 {
        return Err(error_message(status, text));
    }
    let json: serde_json::Value = serde_json::from_str(text).map_err(|e| t!("fal.ai: nieczytelna odpowiedź ({e})", e = e))?;
    json.pointer("/image/url").and_then(|v| v.as_str()).map(String::from).ok_or_else(|| t!("fal.ai: odpowiedź bez obrazu").into())
}

/// Bajty z `data:…;base64,…` (`sync_mode`); `None`, gdy to zwykły adres.
fn data_uri_bytes(url: &str) -> Option<Result<Vec<u8>, String>> {
    let rest = url.strip_prefix("data:")?;
    Some((|| {
        let (head, data) = rest.split_once(',').ok_or(t!("fal.ai: uszkodzony data URI"))?;
        if !head.ends_with(";base64") {
            return Err(t!("fal.ai: nieobsługiwany data URI").to_string());
        }
        base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|e| t!("fal.ai: uszkodzony data URI ({e})", e = e))
    })())
}

/// Przeglądarka: maska obiektu (0 = tło, 255 = obiekt) w rozmiarze wysłanego zdjęcia.
#[cfg(target_arch = "wasm32")]
pub async fn remove_background_async(image: &RgbaImage, key: &str) -> Result<GrayImage, String> {
    let key = key.trim();
    let body = request_body(image, key)?;
    let auth = format!("Key {key}");
    let (status, bytes) =
        web_fetch("POST", ENDPOINT, Some(&body), &[("Authorization", auth.as_str()), ("Content-Type", "application/json")]).await?;
    let url = output_url(status, &String::from_utf8_lossy(&bytes))?;
    let png = match data_uri_bytes(&url) {
        Some(bytes) => bytes?,
        None => {
            let (status, bytes) = web_fetch("GET", &url, None, &[]).await?;
            if status != 200 {
                return Err(t!("fal.ai: pobieranie wyniku (HTTP {status})", status = status));
            }
            bytes
        }
    };
    matte(&png)
}

/// `fetch` przeglądarki: kod HTTP i treść odpowiedzi.
#[cfg(target_arch = "wasm32")]
async fn web_fetch(method: &str, url: &str, body: Option<&str>, headers: &[(&str, &str)]) -> Result<(u16, Vec<u8>), String> {
    use wasm_bindgen::JsCast;
    let js = |e: wasm_bindgen::JsValue| format!("fal.ai: brak połączenia ({e:?})");
    let init = web_sys::RequestInit::new();
    init.set_method(method);
    if let Some(body) = body {
        init.set_body(&wasm_bindgen::JsValue::from_str(body));
    }
    let request = web_sys::Request::new_with_str_and_init(url, &init).map_err(js)?;
    for (k, v) in headers {
        request.headers().set(k, v).map_err(js)?;
    }
    let window = web_sys::window().ok_or(t!("fal.ai: brak okna przeglądarki"))?;
    let response: web_sys::Response =
        wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request)).await.map_err(js)?.dyn_into().map_err(js)?;
    let buffer = wasm_bindgen_futures::JsFuture::from(response.array_buffer().map_err(js)?).await.map_err(js)?;
    Ok((response.status(), js_sys::Uint8Array::new(&buffer).to_vec()))
}

/// Maska obiektu (0 = tło, 255 = obiekt) w rozmiarze wysłanego zdjęcia.
#[cfg(not(target_arch = "wasm32"))]
pub fn remove_background(image: &RgbaImage, key: &str) -> Result<GrayImage, String> {
    let key = key.trim();
    let body = request_body(image, key)?;
    let mut resp = ureq::post(ENDPOINT)
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(TIMEOUT))
        .build()
        .header("Authorization", &format!("Key {key}"))
        .header("Content-Type", "application/json")
        .send(body.as_bytes())
        .map_err(|e| t!("fal.ai: brak połączenia ({e})", e = e))?;
    let status = resp.status().as_u16();
    let text = resp
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE)
        .read_to_string()
        .map_err(|e| t!("fal.ai: przerwana odpowiedź ({e})", e = e))?;
    let url = output_url(status, &text)?;
    let png = fetch(&url)?;
    matte(&png)
}

/// Zdjęcie jako `data:image/jpeg;base64,…` (przezroczystość na białym tle, JPEG 92).
fn jpeg_data_uri(image: &RgbaImage) -> Result<String, String> {
    let (w, h) = image.dimensions();
    let size = asone_core::imageio::fit_within([w, h], UPLOAD_SIDE);
    let scaled;
    let img = if size != [w, h] {
        scaled = asone_core::imageio::resize_rgba(image, size, asone_core::imageio::lanczos3())?;
        &scaled
    } else {
        image
    };
    let mut rgb = Vec::with_capacity(img.as_raw().len() / 4 * 3);
    for p in img.pixels() {
        let a = p[3] as u32;
        for c in 0..3 {
            rgb.push(((p[c] as u32 * a + 255 * (255 - a) + 127) / 255) as u8);
        }
    }
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 92)
        .encode(&rgb, img.width(), img.height(), image::ExtendedColorType::Rgb8)
        .map_err(|e| t!("fal.ai: kodowanie zdjęcia ({e})", e = e))?;
    Ok(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(jpeg)))
}

/// Wynik: data URI (`sync_mode`) albo adres pliku na serwerze fal.ai.
#[cfg(not(target_arch = "wasm32"))]
fn fetch(url: &str) -> Result<Vec<u8>, String> {
    if let Some(bytes) = data_uri_bytes(url) {
        return bytes;
    }
    let mut resp = ureq::get(url)
        .config()
        .timeout_global(Some(TIMEOUT))
        .build()
        .call()
        .map_err(|e| t!("fal.ai: pobieranie wyniku ({e})", e = e))?;
    resp.body_mut()
        .with_config()
        .limit(MAX_RESPONSE)
        .read_to_vec()
        .map_err(|e| t!("fal.ai: pobieranie wyniku ({e})", e = e))
}

/// Maska z PNG: kanał alfa, a gdy go nie ma (albo jest pełny) – jasność (biały = obiekt).
fn matte(bytes: &[u8]) -> Result<GrayImage, String> {
    let img = image::load_from_memory(bytes).map_err(|e| t!("fal.ai: nieczytelny wynik ({e})", e = e))?;
    if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        if rgba.pixels().any(|p| p[3] < 255) {
            let (w, h) = rgba.dimensions();
            let alpha = rgba.pixels().map(|p| p[3]).collect();
            return GrayImage::from_raw(w, h, alpha).ok_or_else(|| t!("fal.ai: zły rozmiar maski").to_string());
        }
    }
    Ok(img.to_luma8())
}

/// Komunikat błędu z kodu HTTP i pola `detail` odpowiedzi.
fn error_message(status: u16, body: &str) -> String {
    let detail = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| match v.get("detail") {
            Some(serde_json::Value::String(s)) => Some(s.clone()),
            Some(serde_json::Value::Array(a)) => Some(
                a.iter()
                    .filter_map(|e| e.get("msg").and_then(|m| m.as_str()))
                    .collect::<Vec<_>>()
                    .join("; "),
            ),
            _ => None,
        })
        .unwrap_or_else(|| body.trim().to_string());
    let detail: String = detail.chars().take(300).collect();
    match status {
        401 => t!("fal.ai: nieprawidłowy klucz API (401)").into(),
        403 => t!("fal.ai: brak dostępu (403) – sprawdź saldo konta fal.ai. {detail}", detail = detail),
        422 => t!("fal.ai: zdjęcie odrzucone (422). {detail}", detail = detail),
        429 => t!("fal.ai: za dużo zapytań naraz (429) – spróbuj za chwilę").into(),
        _ => t!("fal.ai: błąd {status}. {detail}", status = status, detail = detail),
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn png(img: image::DynamicImage) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn maska_z_kanalu_alfa() {
        let mut img = RgbaImage::from_pixel(2, 1, image::Rgba([10, 20, 30, 255]));
        img.put_pixel(1, 0, image::Rgba([10, 20, 30, 0]));
        let m = matte(&png(img.into())).unwrap();
        assert_eq!(m.as_raw(), &vec![255, 0]);
    }

    #[test]
    fn maska_z_jasnosci() {
        let img = GrayImage::from_raw(2, 1, vec![0, 255]).unwrap();
        let m = matte(&png(img.into())).unwrap();
        assert_eq!(m.as_raw(), &vec![0, 255]);
    }

    #[test]
    fn data_uri_tam_i_z_powrotem() {
        let img = RgbaImage::from_pixel(3000, 1500, image::Rgba([200, 100, 50, 255]));
        let uri = jpeg_data_uri(&img).unwrap();
        let bytes = fetch(&uri).unwrap();
        let back = image::load_from_memory(&bytes).unwrap();
        assert_eq!((back.width(), back.height()), (UPLOAD_SIDE, UPLOAD_SIDE / 2));
    }

    #[test]
    fn komunikaty_bledow() {
        assert!(error_message(401, "{}").contains("klucz"));
        let e = error_message(403, r#"{"detail":"User is locked. Reason: Exhausted balance."}"#);
        assert!(e.contains("saldo") && e.contains("Exhausted balance"));
        let e = error_message(422, r#"{"detail":[{"msg":"bad image"},{"msg":"too big"}]}"#);
        assert!(e.contains("bad image; too big"));
    }

    /// Prawdziwe zapytanie (płatne, ok. 0,018 USD): `FAL_KEY=… cargo test -p asone fal_na_zywo -- --ignored`.
    #[test]
    #[ignore = "płatne zapytanie do fal.ai, wymaga FAL_KEY"]
    fn fal_na_zywo() {
        let key = std::env::var("FAL_KEY").expect("FAL_KEY");
        let sample = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sample/produkt-1.jpg");
        let img = asone_core::imageio::decode_path(&sample).unwrap();
        let t = std::time::Instant::now();
        let mask = remove_background(&img, &key).unwrap();
        eprintln!("RMBG-3.0 (fal.ai): {:?}, maska {:?}", t.elapsed(), mask.dimensions());
        if let Ok(out) = std::env::var("ASONE_MASK_OUT") {
            mask.save(out).unwrap();
        }
        let (w, h) = mask.dimensions();
        assert_eq!((w, h), img.dimensions());
        assert!(mask.get_pixel(w / 2, h / 2)[0] > 200);
        assert!(mask.get_pixel(5, 5)[0] < 40);
    }
}
