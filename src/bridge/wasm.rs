//! wasm32-only bridge plumbing: selector-based `data-*` reads and
//! `MutationObserver`-backed watchers.

#![cfg(target_arch = "wasm32")]

use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};

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
}

impl super::super::subscribe::Teardown for Inner {
    fn remove(self: Box<Self>) {
        self.observer.disconnect();
    }

    fn forget(self: Box<Self>) {
        self.callback.forget();
    }
}

/// Installs a `MutationObserver` on the bridge element so `on_change` runs
/// every time `attr_name` is updated. The handler receives the new raw
/// attribute string (after trimming); if the attribute is removed or trims
/// empty, the handler is skipped.
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

    let attr_name_owned: Rc<str> = attr_name.into();
    let observed_attr = Rc::clone(&attr_name_owned);
    let observed_element = element.clone();

    let callback = Closure::<dyn Fn(js_sys::Array, web_sys::MutationObserver)>::new(
        move |_records: js_sys::Array, _observer: web_sys::MutationObserver| {
            if let Some(raw) = observed_element.get_attribute(&observed_attr) {
                let trimmed = raw.trim();
                if !trimmed.is_empty() {
                    on_change(trimmed.to_string());
                }
            }
        },
    );

    let observer = web_sys::MutationObserver::new(callback.as_ref().unchecked_ref())
        .map_err(|error| Error::ExecFailed(format!("MutationObserver::new: {error:?}")))?;

    let init = web_sys::MutationObserverInit::new();
    init.set_attributes(true);
    let filter = js_sys::Array::new();
    filter.push(&JsValue::from_str(attr_name));
    init.set_attribute_filter(&filter);

    observer
        .observe_with_options(&element, &init)
        .map_err(|error| Error::ExecFailed(format!("MutationObserver.observe: {error:?}")))?;

    Ok(Inner { observer, callback })
}
