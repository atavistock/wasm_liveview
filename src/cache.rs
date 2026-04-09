//! Thread-local cache for page-stable JS handles (window, document,
//! liveSocket, execJS). The `[data-phx-session]` root is re-queried per
//! call since LV navigation can swap it out. See the crate README.
//!
//! `mod cache;` in `lib.rs` is already gated on `target_arch = "wasm32"`,
//! so no further cfg is needed here.

use std::cell::RefCell;

use wasm_bindgen::{JsCast, JsValue};

use crate::error::Error;

struct Handles {
    window: web_sys::Window,
    document: web_sys::Document,
    live_socket: JsValue,
    exec_js: js_sys::Function,
}

thread_local! {
    static HANDLES: RefCell<Option<Handles>> = const { RefCell::new(None) };
}

fn ensure_handles(slot: &mut Option<Handles>) -> Result<(), Error> {
    if slot.is_some() {
        return Ok(());
    }

    let window = web_sys::window().ok_or(Error::NoWindow)?;
    let document = window.document().ok_or(Error::NoDocument)?;

    let live_socket = js_sys::Reflect::get(&window, &JsValue::from_str("liveSocket"))
        .map_err(|_| Error::NoLiveSocket)?;
    if live_socket.is_undefined() || live_socket.is_null() {
        return Err(Error::NoLiveSocket);
    }

    let exec_js_val = js_sys::Reflect::get(&live_socket, &JsValue::from_str("execJS"))
        .map_err(|_| Error::NoLiveSocket)?;
    let exec_js: js_sys::Function = exec_js_val.dyn_into().map_err(|_| Error::NoLiveSocket)?;

    *slot = Some(Handles {
        window,
        document,
        live_socket,
        exec_js,
    });
    Ok(())
}

/// Returns a clone of the cached `window` handle (cheap - a `JsValue` is a
/// reference-counted handle into JS).
pub fn window() -> Result<web_sys::Window, Error> {
    HANDLES.with(|cell| {
        let mut slot = cell.borrow_mut();
        ensure_handles(&mut slot)?;
        Ok(slot.as_ref().unwrap().window.clone())
    })
}

/// Runs `callback` with references to the cached `execJS` function, the
/// `liveSocket` it binds to as `this`, and the current LiveView root
/// element (re-queried from the cached `document` each call).
pub fn with_live_view<Return, Callback>(callback: Callback) -> Result<Return, Error>
where
    Callback: FnOnce(&js_sys::Function, &JsValue, &web_sys::Element) -> Result<Return, Error>,
{
    HANDLES.with(|cell| {
        let mut slot = cell.borrow_mut();
        ensure_handles(&mut slot)?;
        let handles = slot.as_ref().unwrap();

        let root = handles
            .document
            .query_selector("[data-phx-session]")
            .ok()
            .flatten()
            .ok_or(Error::NoLiveViewRoot)?;

        callback(&handles.exec_js, &handles.live_socket, &root)
    })
}
