//! # asone-core
//!
//! Rdzeń AsOne bez zależności od GUI – wspólny dla aplikacji okienkowej (desktop, Android, wasm),
//! `asone-cli` i serwera MCP:
//!
//! * [`naming`] – nazwy plików przyjazne dla URL (`123-marka model` → `123-marka-model-1.jpg`),
//! * [`edits`] – niedestrukcyjne parametry (kadr, marginesy, tło, maska) i edycja maski,
//! * [`imageio`] – dekodowanie z orientacją EXIF, skalowanie Lanczos3 (`fast_image_resize`),
//! * [`compose`] – kompozycja wyniku na CPU (ta sama semantyka co shader GPU),
//! * [`jpeg`] – Progressive JPEG bez metadanych (MozJPEG albo `jpeg-encoder`),
//! * [`finish`] – limity „na sztywno” (3000×3600, 3 MB) i kompresja do rozmiaru,
//! * [`autocrop`] – kadr do obiektu na jasnym tle,
//! * [`archive`] – ZIP i PDF,
//! * [`batch`] – eksport wsadowy z ochroną oryginałów.

pub mod archive;
pub mod autocrop;
pub mod batch;
pub mod compose;
pub mod edits;
pub mod finish;
pub mod imageio;
pub mod jpeg;
pub mod naming;
