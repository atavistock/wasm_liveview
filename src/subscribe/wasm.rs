//! wasm32-only listener plumbing: owns the `Closure`, registers and
//! unregisters it on `window`, and JSON-decodes `event.detail` into the
//! caller's `Event` type.

#![cfg(target_arch = "wasm32")]

use std::rc::Rc;

use serde::de::DeserializeOwned;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};

use crate::error::Error;

pub(super) struct Inner {
    event_name: Rc<str>,
    callback: Closure<dyn Fn(web_sys::CustomEvent)>,
}

impl super::Teardown for Inner {
    fn remove(self: Box<Self>) {
        if let Some(window) = web_sys::window() {
            let _ = window.remove_event_listener_with_callback(
                &self.event_name,
                self.callback.as_ref().unchecked_ref(),
            );
        }
    }

    fn forget(self: Box<Self>) {
        self.callback.forget();
    }
}

pub(super) fn subscribe<Event, Handler>(event: &str, handler: Handler) -> Result<Inner, Error>
where
    Event: DeserializeOwned + 'static,
    Handler: Fn(Event) + 'static,
{
    let window = crate::cache::window()?;
    let event_name: Rc<str> = format!("phx:{event}").into();
    let logged_name = Rc::clone(&event_name);

    let callback = Closure::<dyn Fn(web_sys::CustomEvent)>::new(
        move |custom_event: web_sys::CustomEvent| {
            deliver(&logged_name, custom_event.detail(), &handler);
        },
    );

    window
        .add_event_listener_with_callback(&event_name, callback.as_ref().unchecked_ref())
        .map_err(|error| Error::ExecFailed(format!("{error:?}")))?;

    Ok(Inner {
        event_name,
        callback,
    })
}

fn deliver<Event, Handler>(event_name: &str, detail: JsValue, handler: &Handler)
where
    Event: DeserializeOwned,
    Handler: Fn(Event),
{
    let json = match js_sys::JSON::stringify(&detail) {
        Ok(json_string) => json_string.as_string().unwrap_or_else(|| "null".into()),
        Err(_) => {
            web_sys::console::error_1(&JsValue::from_str(&format!(
                "wasm_liveview: could not stringify detail for {event_name}"
            )));
            return;
        }
    };

    match serde_json::from_str::<Event>(&json) {
        Ok(value) => handler(value),
        Err(error) => web_sys::console::error_1(&JsValue::from_str(&format!(
            "wasm_liveview: could not deserialize {event_name}: {error}"
        ))),
    }
}
