//! Typed reader and watcher for server-rendered `data-*` attributes.
//!
//! Phoenix LiveView templates often carry authoritative state as `data-*`
//! attributes on a hidden "bridge" element, e.g.
//!
//! ```html
//! <div id="my-bridge"
//!      phx-update="ignore"
//!      data-round-status="playing"
//!      data-remaining-seconds="42"></div>
//! ```
//!
//! [`Bridge`] wraps that pattern: typed reads ([`Bridge::read`],
//! [`Bridge::read_json`]) and a `MutationObserver`-backed watcher
//! ([`Bridge::watch`], [`Bridge::watch_json`]). When the server re-renders,
//! watchers fire -- no polling, no custom hook.
//!
//! # Example
//!
//! ```no_run
//! use wasm_liveview::Bridge;
//!
//! let bridge = Bridge::new("#my-bridge");
//!
//! let remaining: Option<f32> = bridge.read("data-remaining-seconds");
//!
//! let sub = bridge.watch::<f32, _>("data-remaining-seconds", |secs| {
//!     let _ = secs;
//! })?;
//! sub.forget();
//! # Ok::<(), wasm_liveview::Error>(())
//! ```
//!
//! # Element lifetime
//!
//! The bridge element must exist when [`Bridge::watch`] is called.
//! `phx-update="ignore"` is recommended so LiveView mutates the attributes
//! in place -- if the element is replaced, the `MutationObserver` silently
//! stops firing.

use std::str::FromStr;

use serde::de::DeserializeOwned;

use crate::error::Error;
use crate::subscribe::Subscription;

#[cfg(target_arch = "wasm32")]
mod wasm;

/// Selector-keyed handle to a server-rendered bridge element.
///
/// Cloning is cheap; only the selector string is stored. Element lookup
/// happens on each call, so a `Bridge` survives LiveView navigations.
#[derive(Debug, Clone)]
pub struct Bridge {
    selector: String,
}

impl Bridge {
    /// Builds a [`Bridge`] for the element matching `selector` (any CSS
    /// selector accepted by `document.querySelector`).
    pub fn new(selector: impl Into<String>) -> Self {
        Self {
            selector: selector.into(),
        }
    }

    /// Returns the selector this bridge was built with.
    pub fn selector(&self) -> &str {
        &self.selector
    }

    /// Reads an attribute as a raw string. `None` if the element or
    /// attribute is missing, or the value is empty after trimming.
    pub fn attr(&self, name: &str) -> Option<String> {
        attr_impl(&self.selector, name)
    }

    /// Reads an attribute and parses it via [`FromStr`]. `None` if missing,
    /// empty, or unparseable. Parse errors are swallowed silently; use
    /// [`Bridge::attr`] to inspect the raw value.
    pub fn read<T>(&self, name: &str) -> Option<T>
    where
        T: FromStr,
    {
        self.attr(name).and_then(|raw| raw.parse::<T>().ok())
    }

    /// Reads an attribute and JSON-decodes it. `None` if missing, empty, or
    /// undecodable.
    pub fn read_json<T>(&self, name: &str) -> Option<T>
    where
        T: DeserializeOwned,
    {
        self.attr(name)
            .and_then(|raw| serde_json::from_str::<T>(&raw).ok())
    }

    /// Watches `name` for changes, parsing each new value via [`FromStr`].
    ///
    /// Fires once per mutation that leaves the attribute with a parseable
    /// value, plus once per `phx:page-loading-stop` (initial page ready and
    /// reconnect) with the current value -- callers don't need a separate
    /// initial read or post-disconnect re-sync.
    ///
    /// Parse failures during mutations are logged via `console.error` and
    /// dropped.
    ///
    /// The returned [`Subscription`] disconnects the `MutationObserver`
    /// when dropped; call [`Subscription::forget`] for the page lifetime.
    ///
    /// # Errors
    ///
    /// [`Error::NoWindow`], [`Error::NoDocument`], or not-found if the
    /// bridge element can't be located.
    pub fn watch<T, F>(&self, name: &str, handler: F) -> Result<Subscription, Error>
    where
        T: FromStr + 'static,
        F: Fn(T) + 'static,
    {
        watch_impl(&self.selector, name, move |raw: String| {
            raw.parse::<T>().ok().map(|value| handler(value));
        })
    }

    /// Same as [`Bridge::watch`], but JSON-decodes each new value.
    pub fn watch_json<T, F>(&self, name: &str, handler: F) -> Result<Subscription, Error>
    where
        T: DeserializeOwned + 'static,
        F: Fn(T) + 'static,
    {
        watch_impl(&self.selector, name, move |raw: String| {
            match serde_json::from_str::<T>(&raw) {
                Ok(value) => handler(value),
                Err(error) => {
                    log_decode_failure(&error.to_string());
                }
            }
        })
    }
}

#[cfg(target_arch = "wasm32")]
fn attr_impl(selector: &str, name: &str) -> Option<String> {
    wasm::read_attribute(selector, name)
}

#[cfg(not(target_arch = "wasm32"))]
fn attr_impl(_selector: &str, _name: &str) -> Option<String> {
    None
}

#[cfg(target_arch = "wasm32")]
fn watch_impl<F>(selector: &str, name: &str, on_change: F) -> Result<Subscription, Error>
where
    F: Fn(String) + 'static,
{
    wasm::watch(selector, name, on_change).map(|inner| Subscription::from_inner(Box::new(inner)))
}

#[cfg(not(target_arch = "wasm32"))]
fn watch_impl<F>(_selector: &str, _name: &str, _on_change: F) -> Result<Subscription, Error>
where
    F: Fn(String) + 'static,
{
    Ok(Subscription::inert())
}

#[cfg(target_arch = "wasm32")]
fn log_decode_failure(message: &str) {
    use wasm_bindgen::JsValue;
    web_sys::console::error_1(&JsValue::from_str(&format!(
        "wasm_liveview::bridge: decode failed: {message}"
    )));
}

#[cfg(not(target_arch = "wasm32"))]
fn log_decode_failure(_message: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_is_preserved() {
        let bridge = Bridge::new("#my-bridge");
        assert_eq!(bridge.selector(), "#my-bridge");
    }

    #[test]
    fn non_wasm_attr_is_none() {
        let bridge = Bridge::new("#my-bridge");
        assert!(bridge.attr("data-anything").is_none());
        assert!(bridge.read::<f32>("data-anything").is_none());
        assert!(bridge
            .read_json::<serde_json::Value>("data-anything")
            .is_none());
    }

    #[test]
    fn non_wasm_watch_stubs_to_ok() {
        let bridge = Bridge::new("#my-bridge");
        let sub = bridge
            .watch::<f32, _>("data-remaining-seconds", |_| {})
            .unwrap();
        drop(sub);
    }

    #[test]
    fn non_wasm_watch_json_stubs_to_ok() {
        let bridge = Bridge::new("#my-bridge");
        let sub = bridge
            .watch_json::<serde_json::Value, _>("data-payload", |_| {})
            .unwrap();
        sub.forget();
    }
}
