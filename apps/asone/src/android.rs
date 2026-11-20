//! Android: most do powłoki Kotlin (`android/app/src/main/java/io/github/lucasterg/asone/AsOneActivity.kt`,
//! GameActivity).
//!
//! Kotlin → Rust (`nativeOn*`): marginesy pasków systemowych, wybrane zdjęcia (bajty z okna systemowego),
//! tekst ze schowka, tekst z okna edycji nazwy. Rust → Kotlin (metody aktywności przez JNI):
//! okno wyboru zdjęć, schowek, okno edycji tekstu, kolor ikon paska stanu, zapis JPG do Galerii.

use std::sync::{Mutex, MutexGuard, OnceLock};

use jni::JNIEnv;
use jni::objects::{JByteArray, JObject, JString, JValue};
use jni::sys::jint;

/// Zdarzenie z powłoki Kotlin, odbierane w pętli aplikacji.
pub enum Event {
    /// Jedno wybrane zdjęcie (nazwa pliku, zawartość).
    Picked(String, Vec<u8>),
    /// Koniec wyboru: ile wczytano, ilu nie udało się odczytać.
    PickFinished { ok: usize, failed: usize },
    /// Tekst ze schowka (przycisk „Wklej”).
    Clipboard(String),
    /// Tekst zatwierdzony w oknie edycji nazwy.
    Text(String),
}

static CTX: OnceLock<egui::Context> = OnceLock::new();
/// Marginesy w pikselach: lewo, góra, prawo, dół (paski systemowe + wycięcie aparatu).
static INSETS: Mutex<[i32; 4]> = Mutex::new([0; 4]);
static EVENTS: Mutex<Vec<Event>> = Mutex::new(Vec::new());

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Kontekst egui do budzenia pętli po zdarzeniu z Kotlina.
pub fn set_context(ctx: &egui::Context) {
    let _ = CTX.set(ctx.clone());
}

fn wake() {
    if let Some(ctx) = CTX.get() {
        ctx.request_repaint();
    }
}

fn push(event: Event) {
    lock(&EVENTS).push(event);
    wake();
}

/// Ostatnie marginesy pasków systemowych w pikselach `[lewo, góra, prawo, dół]`.
pub fn insets_px() -> [i32; 4] {
    *lock(&INSETS)
}

pub fn take_events() -> Vec<Event> {
    std::mem::take(&mut *lock(&EVENTS))
}

// ---- Kotlin → Rust -------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_lucasterg_asone_AsOneActivity_nativeOnInsets(
    _env: JNIEnv,
    _this: JObject,
    left: jint,
    top: jint,
    right: jint,
    bottom: jint,
) {
    *lock(&INSETS) = [left.max(0), top.max(0), right.max(0), bottom.max(0)];
    wake();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_lucasterg_asone_AsOneActivity_nativeOnImagePicked(
    mut env: JNIEnv,
    _this: JObject,
    name: JString,
    bytes: JByteArray,
) {
    let name: String = env.get_string(&name).map(Into::into).unwrap_or_else(|_| "zdjecie.jpg".into());
    if let Ok(data) = env.convert_byte_array(&bytes) {
        push(Event::Picked(name, data));
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_lucasterg_asone_AsOneActivity_nativeOnPickFinished(
    _env: JNIEnv,
    _this: JObject,
    ok: jint,
    failed: jint,
) {
    push(Event::PickFinished { ok: ok.max(0) as usize, failed: failed.max(0) as usize });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_lucasterg_asone_AsOneActivity_nativeOnClipboardText(
    mut env: JNIEnv,
    _this: JObject,
    text: JString,
) {
    let text: String = env.get_string(&text).map(Into::into).unwrap_or_default();
    push(Event::Clipboard(text));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_lucasterg_asone_AsOneActivity_nativeOnTextEntered(
    mut env: JNIEnv,
    _this: JObject,
    text: JString,
) {
    let text: String = env.get_string(&text).map(Into::into).unwrap_or_default();
    push(Event::Text(text));
}

// ---- Rust → Kotlin -------------------------------------------------------------------------

/// Wywołuje metodę aktywności z dowolnego wątku (w ramce lokalnych referencji).
fn with_activity<T>(f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<T>) -> Option<T> {
    let app = crate::ANDROID_APP.get()?;
    // SAFETY: wskaźniki VM i aktywności pochodzą od android-activity i żyją tyle co aplikacja.
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) }.ok()?;
    let mut env = vm.attach_current_thread_permanently().ok()?;
    let result = env.with_local_frame(32, |env| {
        // SAFETY: globalna referencja do aktywności – nie usuwamy jej.
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
        f(env, &activity)
    });
    if result.is_err() {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    result.ok()
}

/// Systemowe okno wyboru zdjęć (kilka naraz).
pub fn pick_images() {
    with_activity(|env, a| env.call_method(a, "pickImages", "()V", &[]).map(drop));
}

/// Prośba o tekst ze schowka – wynik przychodzi jako [`Event::Clipboard`].
pub fn request_clipboard() {
    with_activity(|env, a| env.call_method(a, "requestClipboardText", "()V", &[]).map(drop));
}

/// Okno edycji tekstu z klawiaturą systemową – wynik przychodzi jako [`Event::Text`].
pub fn edit_text(title: &str, current: &str) {
    with_activity(|env, a| {
        let t = env.new_string(title)?;
        let c = env.new_string(current)?;
        env.call_method(a, "editText", "(Ljava/lang/String;Ljava/lang/String;)V", &[JValue::Object(&t), JValue::Object(&c)])
            .map(drop)
    });
}

/// Ciemne ikony paska stanu i nawigacji na jasnym motywie (`true`), jasne na ciemnym.
pub fn set_light_system_bars(light: bool) {
    with_activity(|env, a| env.call_method(a, "setLightSystemBars", "(Z)V", &[JValue::Bool(u8::from(light))]).map(drop));
}

/// Zapis JPG do Galerii (Pictures/AsOne); zwraca miejsce zapisu.
pub fn save_image(name: &str, bytes: &[u8]) -> Result<String, String> {
    with_activity(|env, a| {
        let n = env.new_string(name)?;
        let b = env.byte_array_from_slice(bytes)?;
        let r = env
            .call_method(a, "saveImage", "(Ljava/lang/String;[B)Ljava/lang/String;", &[JValue::Object(&n), JValue::Object(&b)])?
            .l()?;
        if r.is_null() {
            Ok(None)
        } else {
            let s: String = env.get_string(&JString::from(r))?.into();
            Ok(Some(s))
        }
    })
    .flatten()
    .ok_or_else(|| format!("{name}: nie udało się zapisać w Galerii"))
}
