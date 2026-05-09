//! wasm32-only bridge plumbing: selector-based `data-*` reads and
//! `MutationObserver`-backed watchers.

#![cfg(target_arch = "wasm32")]

use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};

use crate::cache::ResetHookId;
use crate::error::Error;

/// Looks up the element matching `selector` and returns the trimmed value
/// of its `attr_name` attribute. Returns `None` when the element is missing,
/// the attribute is missing, or the value trims to empty.
pub(super) fn read_attribute(selector: &str, attr_name: &str) -> Option<String> {
    let document = crate::cache::document().ok()?;
    let element = document.query_selector(selector).ok().flatten()?;
    let raw = element.get_attribute(attr_name)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

pub(super) struct Inner {
    observer: web_sys::MutationObserver,
    callback: Closure<dyn Fn(js_sys::Array, web_sys::MutationObserver)>,
    reset_hook_id: ResetHookId,
}

impl super::super::subscribe::Teardown for Inner {
    fn remove(self: Box<Self>) {
        crate::cache::unregister_reset_hook(self.reset_hook_id);
        self.observer.disconnect();
    }

    fn forget(self: Box<Self>) {
        // Reset hook stays registered for the rest of the page's lifetime,
        // matching the leaked MutationObserver callback.
        self.callback.forget();
    }
}

/// Installs a `MutationObserver` on the bridge element so `on_change` runs
/// every time `attr_name` is updated. The handler receives the new raw
/// attribute string (after trimming); if the attribute is removed or trims
/// empty, the handler is skipped.
///
/// Also registers a reset hook so `on_change` is re-invoked with the
/// current attribute value on every `phx:page-loading-stop` (initial page
/// ready and reconnect after disconnect).
pub(super) fn watch<F>(selector: &str, attr_name: &str, on_change: F) -> Result<Inner, Error>
where
    F: Fn(String) + 'static,
{
    let document = crate::cache::document()?;
    let element = document
        .query_selector(selector)
        .ok()
        .flatten()
        .ok_or(Error::NoLiveViewRoot)?;

    // `Rc<F>` (not `Rc<dyn Fn(String)>`) keeps F monomorphized so the
    // call site dispatches statically. The outer reset-hook trait object
    // still erases below, but we save one vtable hop on every fire.
    let attr_name: Rc<str> = attr_name.into();
    let on_change = Rc::new(on_change);

    let callback = {
        let attr_name = Rc::clone(&attr_name);
        let on_change = Rc::clone(&on_change);
        let element = element.clone();
        Closure::<dyn Fn(js_sys::Array, web_sys::MutationObserver)>::new(
            move |_records: js_sys::Array, _observer: web_sys::MutationObserver| {
                if let Some(raw) = element.get_attribute(&attr_name) {
                    let trimmed = raw.trim();
                    if !trimmed.is_empty() {
                        on_change(trimmed.to_string());
                    }
                }
            },
        )
    };

    let observer = web_sys::MutationObserver::new(callback.as_ref().unchecked_ref())
        .map_err(|error| Error::ExecFailed(format!("MutationObserver::new: {error:?}")))?;

    let init = web_sys::MutationObserverInit::new();
    init.set_attributes(true);
    let filter = js_sys::Array::new();
    filter.push(&JsValue::from_str(&attr_name));
    init.set_attribute_filter(&filter);

    observer
        .observe_with_options(&element, &init)
        .map_err(|error| Error::ExecFailed(format!("MutationObserver.observe: {error:?}")))?;

    // Final use of `attr_name` and `on_change` -- move them in instead of
    // cloning. The `as Rc<dyn Fn()>` erases at the registry boundary; the
    // inner call to `on_change(raw)` stays monomorphic.
    let reset_hook: Rc<dyn Fn()> = {
        let selector: Rc<str> = selector.into();
        Rc::new(move || {
            if let Some(raw) = read_attribute(&selector, &attr_name) {
                on_change(raw);
            }
        })
    };
    let reset_hook_id = crate::cache::register_reset_hook(reset_hook);

    Ok(Inner {
        observer,
        callback,
        reset_hook_id,
    })
}
