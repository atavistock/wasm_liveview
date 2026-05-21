//! wasm32-only bridge plumbing: selector-based `data-*` reads and
//! `MutationObserver`-backed watchers.

#![cfg(target_arch = "wasm32")]

use std::rc::Rc as RefCount;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};

use crate::cache::ResetHookId;
use crate::error::Error;

/// Trimmed value of `attr_name` on the element at `selector`. `None` if
/// the element or attribute is missing, or the value trims to empty.
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

/// Installs a `MutationObserver` so `on_change` runs each time `attr_name`
/// updates (trimmed; empty/removed skips the handler). Also registers a
/// reset hook that re-invokes `on_change` with the current value on every
/// `phx:page-loading-stop` (initial ready and reconnect).
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

    // `RefCount<F>` (not `RefCount<dyn Fn(String)>`) keeps F monomorphized
    // for static dispatch on every fire.
    let attr_name: RefCount<str> = attr_name.into();
    let on_change = RefCount::new(on_change);

    let callback = {
        let attr_name = RefCount::clone(&attr_name);
        let on_change = RefCount::clone(&on_change);
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

    // Final use of `attr_name`/`on_change` -- move them in. Erasure happens
    // at the `dyn Fn()` registry boundary; the inner call stays monomorphic.
    let reset_hook: RefCount<dyn Fn()> = {
        let selector: RefCount<str> = selector.into();
        RefCount::new(move || {
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
