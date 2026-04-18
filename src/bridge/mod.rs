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
//! [`Bridge`] wraps that pattern: one struct that knows the element's
//! selector, with typed reads ([`Bridge::read`], [`Bridge::read_json`]) and
//! a typed watcher ([`Bridge::watch`], [`Bridge::watch_json`]) backed by a
//! `MutationObserver`. When the server re-renders the template, the
//! attribute changes and registered watchers fire -- no polling, no custom
//! hook.
//!
//! # Example
//!
//! ```no_run
//! use wasm_liveview::Bridge;
//!
//! let bridge = Bridge::new("#my-bridge");
//!
//! // One-shot read; None if the attribute is missing or unparseable.
//! let remaining: Option<f32> = bridge.read("data-remaining-seconds");
//!
//! // Watch for updates.
//! let sub = bridge.watch::<f32, _>("data-remaining-seconds", |secs| {
//!     let _ = secs;
//! })?;
//! sub.forget();
//! # Ok::<(), wasm_liveview::Error>(())
//! ```
//!
//! # Element lifetime
//!
//! The bridge element must be present when [`Bridge::watch`] is called.
//! `phx-update="ignore"` is recommended so LiveView mutates the
//! attributes in place rather than replacing the element -- if the element
//! is replaced, the underlying `MutationObserver` silently stops firing.

use std::str::FromStr;

use serde::de::DeserializeOwned;

use crate::error::Error;
use crate::subscribe::Subscription;

#[cfg(target_arch = "wasm32")]
mod wasm;

/// Selector-keyed handle to a server-rendered bridge element.
///
/// Cloning is cheap; the struct only stores the selector string. Element
/// lookup happens on each call, so a `Bridge` is safe to keep across
/// LiveView navigations.
#[derive(Debug, Clone)]
pub struct Bridge {
    selector: String,
}

impl Bridge {
    /// Builds a [`Bridge`] for the element matching `selector`.
    ///
    /// `selector` is any CSS selector accepted by `document.querySelector`,
    /// for example `"#my-bridge"` or `"[data-bridge]"`.
    pub fn new(selector: impl Into<String>) -> Self {
        Self {
            selector: selector.into(),
        }
    }

    /// Returns the selector this bridge was built with.
    pub fn selector(&self) -> &str {
        &self.selector
    }

    /// Reads an attribute as a raw string.
    ///
    /// Returns `None` when the element or attribute is missing, or the
    /// attribute value is empty after trimming.
    pub fn attr(&self, name: &str) -> Option<String> {
        attr_impl(&self.selector, name)
    }

    /// Reads an attribute and parses it via [`FromStr`].
    ///
    /// Returns `None` when the attribute is missing, empty, or fails to
    /// parse. Parse errors are swallowed silently -- use [`Bridge::attr`]
    /// if you need to inspect the raw value.
    pub fn read<T>(&self, name: &str) -> Option<T>
    where
        T: FromStr,
    {
        self.attr(name).and_then(|raw| raw.parse::<T>().ok())
    }

    /// Reads an attribute and JSON-decodes it via [`serde::Deserialize`].
    ///
    /// Returns `None` when the attribute is missing, empty, or fails to
    /// decode.
    pub fn read_json<T>(&self, name: &str) -> Option<T>
    where
        T: DeserializeOwned,
    {
        self.attr(name)
            .and_then(|raw| serde_json::from_str::<T>(&raw).ok())
    }

    /// Watches `name` for changes, parsing each new value via [`FromStr`].
    ///
    /// The handler fires once per mutation that leaves the attribute with a
    /// parseable value. The initial value is *not* delivered -- call
    /// [`Bridge::read`] once at setup time if you need it.
    ///
    /// Parse failures during mutations are logged via `console.error` and
    /// dropped; the handler only runs on successful parse.
    ///
    /// The returned [`Subscription`] disconnects the underlying
    /// `MutationObserver` when dropped. Call [`Subscription::forget`] to
    /// watch for the rest of the page's lifetime.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoWindow`], [`Error::NoDocument`], or a not-found
    /// variant if the bridge element cannot be located.
    pub fn watch<T, F>(&self, name: &str, handler: F) -> Result<Subscription, Error>
    where
        T: FromStr + 'static,
        F: Fn(T) + 'static,
    {
        watch_impl(&self.selector, name, move |raw: String| {
            raw.parse::<T>().ok().map(|value| handler(value));
        })
    }

    /// Watches `name` for changes, JSON-decoding each new value.
    ///
    /// Same semantics as [`Bridge::watch`], but the attribute is decoded
    /// with `serde_json` instead of `FromStr`.
    ///
    /// # Errors
    ///
    /// See [`Bridge::watch`].
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
