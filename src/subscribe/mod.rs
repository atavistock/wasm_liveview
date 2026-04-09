//! Typed `addEventListener` for Phoenix `push_event/3` payloads.
//!
//! `Phoenix.LiveView.push_event/3` dispatches `phx:<event>` `CustomEvent`s on
//! `window`, with the payload as `event.detail`. [`subscribe`] wraps that
//! with JSON decoding into a caller-chosen type, so handlers receive a
//! strongly typed value instead of a raw JS object.

use serde::de::DeserializeOwned;

use crate::error::Error;

mod wasm;

/// Subscribes to a server-pushed LiveView event.
///
/// The `phx:` prefix is added automatically, so
/// `subscribe("score_update", ...)` listens for `phx:score_update`.
///
/// `handler` is invoked once per matching event with `event.detail`
/// deserialized into `Event`. Deserialization failures are logged via
/// `console.error` and dropped: the handler only runs on successful decode,
/// and malformed payloads will never panic your wasm module.
///
/// The returned [`Subscription`] removes the listener when dropped. Call
/// [`Subscription::forget`] to let the listener live for the remainder of
/// the page's lifetime.
///
/// # Errors
///
/// Returns [`Error::NoWindow`] on non-browser environments. Never returns an
/// error on non-wasm targets (the call stubs out).
///
/// # Example
///
/// ```no_run
/// use wasm_liveview as lv;
///
/// #[derive(serde::Deserialize)]
/// struct Score { value: u32 }
///
/// let sub = lv::subscribe::<Score, _>("score_update", |s| {
///     let _ = s.value;
/// })?;
/// sub.forget();
/// # Ok::<(), lv::Error>(())
/// ```
pub fn subscribe<Event, Handler>(event: &str, handler: Handler) -> Result<Subscription, Error>
where
    Event: DeserializeOwned + 'static,
    Handler: Fn(Event) + 'static,
{
    subscribe_impl(event, handler)
}

/// Handle returned by [`subscribe`].
///
/// Dropping the handle removes the underlying `addEventListener` callback.
/// Call [`Subscription::forget`] to keep the listener alive for the rest of
/// the page's lifetime (the usual choice for permanent subscriptions set up
/// at startup).
pub struct Subscription {
    #[cfg(target_arch = "wasm32")]
    inner: Option<wasm::Inner>,
}

#[cfg(target_arch = "wasm32")]
impl Subscription {
    /// Consumes the handle, leaking the listener so it lives for the rest
    /// of the page's lifetime.
    ///
    /// After calling `forget`, the handler can no longer be removed. Use
    /// this for subscriptions that should persist across LiveView
    /// navigations and live as long as the page is loaded.
    pub fn forget(mut self) {
        if let Some(inner) = self.inner.take() {
            inner.forget();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Subscription {
    /// No-op on non-wasm targets. See the wasm implementation for behavior.
    pub fn forget(self) {}
}

impl Drop for Subscription {
    fn drop(&mut self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(inner) = self.inner.take() {
            inner.remove();
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn subscribe_impl<Event, Handler>(event: &str, handler: Handler) -> Result<Subscription, Error>
where
    Event: DeserializeOwned + 'static,
    Handler: Fn(Event) + 'static,
{
    wasm::subscribe(event, handler).map(|inner| Subscription { inner: Some(inner) })
}

#[cfg(not(target_arch = "wasm32"))]
fn subscribe_impl<Event, Handler>(_event: &str, _handler: Handler) -> Result<Subscription, Error>
where
    Event: DeserializeOwned + 'static,
    Handler: Fn(Event) + 'static,
{
    Ok(Subscription {})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_wasm_subscribe_stubs_to_ok() {
        let subscription = subscribe::<serde_json::Value, _>("score_update", |_| {}).unwrap();
        drop(subscription);
    }

    #[test]
    fn non_wasm_forget_is_noop() {
        let subscription = subscribe::<serde_json::Value, _>("score_update", |_| {}).unwrap();
        subscription.forget();
    }
}
