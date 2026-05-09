//! Thread-local cache for page-stable JS handles (window, document,
//! liveSocket, execJS). The `[data-phx-session]` root is re-queried per
//! call since LV navigation can swap it out. See the crate README.
//!
//! On `phx:page-loading-stop` (initial page ready and reconnect after a
//! transport drop) the cache is cleared so subsequent calls re-resolve
//! `window`, `document`, `liveSocket`, and `execJS` from scratch -- the
//! pre-disconnect `liveSocket` reference may no longer be the active one.
//! Bridge watchers register reset hooks via [`register_reset_hook`] so
//! they re-fire with the current attribute value on every reset.
//!
//! `mod cache;` in `lib.rs` is already gated on `target_arch = "wasm32"`,
//! so no further cfg is needed here.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};

use crate::error::Error;

struct Handles {
    window: web_sys::Window,
    document: web_sys::Document,
    live_socket: JsValue,
    exec_js: js_sys::Function,
}

/// Identifier returned by [`register_reset_hook`] and accepted by
/// [`unregister_reset_hook`].
pub(crate) type ResetHookId = u64;

thread_local! {
    static HANDLES: RefCell<Option<Handles>> = const { RefCell::new(None) };
    static RESET_HOOKS: RefCell<Vec<(ResetHookId, Rc<dyn Fn()>)>> =
        const { RefCell::new(Vec::new()) };
    static NEXT_HOOK_ID: Cell<ResetHookId> = const { Cell::new(0) };
    static RESET_LISTENER_INSTALLED: Cell<bool> = const { Cell::new(false) };
    static RESETTING: Cell<bool> = const { Cell::new(false) };
}

/// RAII guard that flips [`RESETTING`] for the duration of [`reset`].
///
/// Acquisition fails if a reset is already in progress on this thread,
/// which is how we shed re-entrant `phx:page-loading-stop` dispatches: a
/// hook callback that synchronously triggers another reset would otherwise
/// fire every registered hook a second time. The Drop impl restores the
/// flag even if a hook panics, so a panicking hook (under unwinding
/// profiles) doesn't permanently lock out future resets.
struct ResetGuard;

impl ResetGuard {
    fn try_acquire() -> Option<Self> {
        RESETTING.with(|cell| {
            if cell.replace(true) { None } else { Some(Self) }
        })
    }
}

impl Drop for ResetGuard {
    fn drop(&mut self) {
        RESETTING.with(|cell| cell.set(false));
    }
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

    install_reset_listener(&slot.as_ref().unwrap().window);
    Ok(())
}

/// Installs the `phx:page-loading-stop` listener on `window` exactly once
/// per thread. The listener clears [`HANDLES`] and runs every registered
/// reset hook. On success the closure is intentionally leaked (`forget`)
/// since it must live for the rest of the page's lifetime; on failure the
/// closure is dropped (tearing down the JS-side trampoline cleanly, since
/// JS never registered it) and the failure is logged. Either way the
/// install flag is set so we don't retry and spam the console.
fn install_reset_listener(window: &web_sys::Window) {
    RESET_LISTENER_INSTALLED.with(|installed| {
        if installed.get() {
            return;
        }
        let closure = Closure::<dyn Fn()>::new(reset);
        match window.add_event_listener_with_callback(
            "phx:page-loading-stop",
            closure.as_ref().unchecked_ref(),
        ) {
            Ok(()) => {
                closure.forget();
            }
            Err(error) => {
                web_sys::console::error_1(&JsValue::from_str(&format!(
                    "wasm_liveview::cache: could not install phx:page-loading-stop listener: {error:?}"
                )));
            }
        }
        installed.set(true);
    });
}

/// Clears the cached handles and invokes every registered reset hook.
/// Called from the `phx:page-loading-stop` listener. Re-entrant calls
/// (e.g. a hook that synchronously dispatches another page-loading-stop)
/// short-circuit via [`ResetGuard`] to avoid double-firing every hook.
fn reset() {
    let _guard = match ResetGuard::try_acquire() {
        Some(guard) => guard,
        None => return,
    };

    HANDLES.with(|cell| {
        cell.borrow_mut().take();
    });

    let hooks: Vec<Rc<dyn Fn()>> = RESET_HOOKS.with(|registry| {
        registry
            .borrow()
            .iter()
            .map(|(_, hook)| Rc::clone(hook))
            .collect()
    });

    for hook in hooks {
        hook();
    }
}

/// Registers a callback to be invoked whenever the cache is reset (i.e.
/// on `phx:page-loading-stop`). Returns an id that can be passed to
/// [`unregister_reset_hook`] to remove the hook.
pub(crate) fn register_reset_hook(hook: Rc<dyn Fn()>) -> ResetHookId {
    let id = NEXT_HOOK_ID.with(|cell| {
        let id = cell.get();
        cell.set(id.wrapping_add(1));
        id
    });
    RESET_HOOKS.with(|registry| registry.borrow_mut().push((id, hook)));
    id
}

/// Removes a reset hook previously registered with [`register_reset_hook`].
/// No-op if the id is not present.
pub(crate) fn unregister_reset_hook(id: ResetHookId) {
    RESET_HOOKS.with(|registry| {
        registry.borrow_mut().retain(|(hook_id, _)| *hook_id != id);
    });
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

/// Returns a clone of the cached `document` handle.
pub fn document() -> Result<web_sys::Document, Error> {
    HANDLES.with(|cell| {
        let mut slot = cell.borrow_mut();
        ensure_handles(&mut slot)?;
        Ok(slot.as_ref().unwrap().document.clone())
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
