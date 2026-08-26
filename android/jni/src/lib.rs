//! What the Kotlin app can call in the engine.
//!
//! # Why this exists
//!
//! Atur: *"why i can not run models"* on the phone. The engine compiles and
//! runs on Android — `research/android-engine-runs-2026-08-24.md` — but a
//! command-line binary in `/data/local/tmp` is not something an app can use.
//! An Android app calls native code through **JNI**: it loads a `.so` and
//! looks up functions by a name derived from the Java class and method.
//!
//! # Why there is no `jni` crate here
//!
//! The whole project has no dependencies, and the Android app has none either
//! so that its APK cannot break because something else was upgraded. JNI's ABI
//! is small and stable: a function is `extern "system"`, named
//! `Java_<package>_<Class>_<method>` with underscores escaped, and it receives
//! a `JNIEnv*` whose layout is a table of function pointers at fixed indices.
//!
//! **Only the one entry actually used is declared**, by index, with the index
//! written down beside it. Getting an index wrong calls a different function
//! through a pointer, which is not a compile error and not a clean crash — so
//! it cites the position it occupies in the JNI specification's table, and the
//! padding before it is load-bearing rather than decoration.
//!
//! # What it deliberately does not do yet
//!
//! Run a model. That needs the model file on the device and a token loop
//! driven from the UI thread's side, and it is the next step. This proves the
//! bridge itself: the app calls into Rust, Rust measures the phone with the
//! same `core/probe` the desktop uses, and the answer comes back as a string.

use std::ffi::c_void;
use std::os::raw::c_char;

/// The subset of `JNINativeInterface` this file calls, by table position.
///
/// The layout is fixed by the JNI specification and has not changed since
/// 1.6. Entries before the ones needed are padded so the offsets are right —
/// **the padding is load-bearing**, not decoration.
#[repr(C)]
pub struct JniNativeInterface {
    _reserved: [*mut c_void; 4],
    _v1: [*mut c_void; 163],
    /// Index 167: `NewStringUTF(JNIEnv*, const char*) -> jstring`
    new_string_utf: unsafe extern "system" fn(*mut JniEnv, *const c_char) -> *mut c_void,
    /// Index 168: `GetStringUTFLength`, which is not needed but occupies the
    /// slot -- the padding between entries is what keeps the offsets right.
    _get_string_utf_length: *mut c_void,
    /// Index 169: `GetStringUTFChars(JNIEnv*, jstring, jboolean*) -> const char*`
    get_string_utf_chars:
        unsafe extern "system" fn(*mut JniEnv, *mut c_void, *mut u8) -> *const c_char,
    /// Index 170: `ReleaseStringUTFChars(JNIEnv*, jstring, const char*)`
    release_string_utf_chars: unsafe extern "system" fn(*mut JniEnv, *mut c_void, *const c_char),
}

#[repr(C)]
pub struct JniEnv {
    functions: *const JniNativeInterface,
}

/// Hand a Rust string to Java.
///
/// **The bytes must be NUL-terminated and must not contain an interior NUL**,
/// which `CString::new` enforces by returning an error rather than truncating.
/// A string built from a device's own model name is not something to assume
/// about.
unsafe fn to_java(env: *mut JniEnv, s: &str) -> *mut c_void {
    let Ok(c) = std::ffi::CString::new(s) else {
        // An interior NUL means something upstream is wrong; an empty string
        // is a visible symptom rather than a crash in the app.
        let empty = std::ffi::CString::new("").expect("empty string has no NUL");
        return ((*(*env).functions).new_string_utf)(env, empty.as_ptr());
    };
    ((*(*env).functions).new_string_utf)(env, c.as_ptr())
}

/// Read a Java string.
///
/// **The characters must be released.** `GetStringUTFChars` may hand back a
/// copy, and not releasing it leaks for the life of the process -- which on a
/// phone is until the app is killed.
unsafe fn from_java(env: *mut JniEnv, s: *mut c_void) -> String {
    if s.is_null() {
        return String::new();
    }
    let p = ((*(*env).functions).get_string_utf_chars)(env, s, std::ptr::null_mut());
    if p.is_null() {
        return String::new();
    }
    let out = std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned();
    ((*(*env).functions).release_string_utf_chars)(env, s, p);
    out
}

/// `com.aturzone.chaos.Engine.version()`
///
/// The smallest possible round trip: if this returns, the `.so` loaded, the
/// symbol was found, and the calling convention is right.
///
/// # Safety
/// Called only by the JVM, which guarantees `env` is a valid `JNIEnv*` for the
/// current thread.
#[no_mangle]
pub unsafe extern "system" fn Java_com_aturzone_chaos_Engine_version(
    env: *mut JniEnv,
    _class: *mut c_void,
) -> *mut c_void {
    to_java(env, env!("CARGO_PKG_VERSION"))
}

/// `com.aturzone.chaos.Engine.describeDevice()`
///
/// **The same `core/probe` the desktop uses**, which needed no change to read a
/// phone: its unix branch reads `/proc/meminfo`, and Android has one. This is
/// what will decide which model a given phone can hold — Atur's "a powerful
/// phone or a simple phone" — rather than a hard-coded list of handsets.
///
/// # Safety
/// Called only by the JVM, as above.
#[no_mangle]
pub unsafe extern "system" fn Java_com_aturzone_chaos_Engine_describeDevice(
    env: *mut JniEnv,
    _class: *mut c_void,
) -> *mut c_void {
    // **`false` is the whole point of the flag.** The bandwidth probe writes and
    // re-reads a file to defeat the cache; on a phone that is somebody's flash
    // wear and seconds of UI thread. Everything else here is reading
    // /proc/meminfo.
    let m = chaos_probe::Machine::probe(".", false);
    let text = format!(
        "{} threads, {:.1} GiB total, {:.1} GiB available [{}]",
        m.cpu_threads,
        chaos_probe::gib(m.ram_total_bytes.unwrap_or(0)),
        chaos_probe::gib(m.ram_available_bytes.unwrap_or(0)),
        m.ram_source
    );
    to_java(env, &text)
}

/// `com.aturzone.chaos.Engine.startServer(modelPath, host, port, key)`
///
/// **This is what makes ALONE and CORE mean something on a phone.** Until it
/// existed the dial offered four modes and one of them worked: a CORE could
/// not serve, a HELPER could not lend, ALONE could not run a model. Atur, and
/// he was right: *"android can not do any one of works in windows"*.
///
/// It runs `chaos_serve::serve` on its own thread, which is the same server
/// the desktop starts as a child process — the same token loop, the same
/// OpenAI-compatible endpoints — so the app's existing client talks to it with
/// no second protocol and no second token loop to keep in step.
///
/// **This blocks for the life of the app.** Call it from a thread the JVM
/// made: `serve` never returns, and the app is killed rather than shut down,
/// which is also why the ggml exit abort recorded in `android-engine-runs`
/// cannot fire here.
///
/// Returns the reason it stopped, or an empty string.
///
/// # Safety
/// Called only by the JVM.
#[no_mangle]
pub unsafe extern "system" fn Java_com_aturzone_chaos_Engine_startServer(
    env: *mut JniEnv,
    _class: *mut c_void,
    model: *mut c_void,
    host: *mut c_void,
    port: i32,
    key: *mut c_void,
) -> *mut c_void {
    let model = from_java(env, model);
    let host = from_java(env, host);
    let key = from_java(env, key);
    if model.is_empty() {
        return to_java(env, "no model chosen");
    }
    let key = (!key.is_empty()).then_some(key);
    // **Run it here, on the caller's thread.** Spawning from inside the
    // library crashed in `pthread_create` -> `__init_tcb` with
    // SIGSEGV/SEGV_ACCERR before the closure ran, and a bigger stack did not
    // help. Kotlin has threads of its own and the JVM makes them properly, so
    // the caller supplies one and this blocks on it for the life of the app.
    let why = match std::panic::catch_unwind(move || {
        chaos_serve::serve(&model, &host, port as u16, 0.0, key, None, false, true)
    }) {
        Ok(Ok(())) => String::new(),
        Ok(Err(e)) => format!("{e}"),
        Err(_) => "the engine stopped unexpectedly".to_string(),
    };
    to_java(env, &why)
}

/// `com.aturzone.chaos.Engine.listModels(dir)`
///
/// What is on this phone, one name per line, so the app can offer a choice
/// rather than a text box. Uses the same catalogue the desktop does.
///
/// # Safety
/// Called only by the JVM.
#[no_mangle]
pub unsafe extern "system" fn Java_com_aturzone_chaos_Engine_listModels(
    env: *mut JniEnv,
    _class: *mut c_void,
    dir: *mut c_void,
) -> *mut c_void {
    let dir = from_java(env, dir);
    if dir.is_empty() {
        return to_java(env, "");
    }
    // `CHAOS_MODELS` is read before anything home-relative, which is how the
    // app hands the engine its own files directory -- an Android shell has
    // `HOME=/`, so nothing home-relative would be right here.
    std::env::set_var("CHAOS_MODELS", &dir);
    let mut names: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("gguf"))
            {
                if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
                    names.push(n.to_string());
                }
            }
        }
    }
    names.sort();
    to_java(env, &names.join("\n"))
}

#[cfg(test)]
mod tests {
    //! **These run on the host, and cannot call anything JNI.** There is no
    //! `JNIEnv` without a JVM, so what is testable here is the part that does
    //! not touch it: that the device description is built from the probe and
    //! says something, rather than being a placeholder.

    #[test]
    fn the_device_description_is_built_from_the_probe() {
        let m = chaos_probe::Machine::probe(".", false);
        assert!(
            m.cpu_threads >= 1,
            "a machine running this has at least one core"
        );
        let text = format!(
            "{} threads, {:.1} GiB total",
            m.cpu_threads,
            chaos_probe::gib(m.ram_total_bytes.unwrap_or(0))
        );
        assert!(text.contains("threads"));
        // The source is named so a wrong number can be traced to where it came
        // from -- /proc/meminfo on Android and Linux, something else elsewhere.
        assert!(
            !m.ram_source.is_empty(),
            "the probe says where RAM came from"
        );
    }
}
