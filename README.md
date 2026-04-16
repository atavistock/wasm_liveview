# Wasm/LiveView Bridge

A two-way bridge between wasm-bindgen Rust and a mounted Phoenix LiveView.

Outbound, it wraps the `Phoenix.LiveView.JS` command set so Rust/wasm code can fire LV events, navigate, dispatch DOM events, run transitions, and manage focus - without trampolining through hidden `phx-*` trigger elements.

Inbound, it lets Rust subscribe to server-pushed events from `Phoenix.LiveView.push_event/3`.

Written for game code that renders in wasm but wants the server to own authentication, state, and persistence.

## Status

Early. I originally built this inside a wasm game project, then needed the same bridge in a second one, so I extracted it into a shared crate. Both projects use it today, and the docs were cleaned up ahead of a first release. The outbound side wraps the common `JS` commands. The inbound side covers server-pushed events via `window` `phx:<event>` listeners. Not yet implemented: client-to-server `pushEvent` with a reply callback, which needs a LiveView hook on the JS side.

## Install

Until this lands on [crates.io](https://crates.io), pull it from GitHub:

```toml
[dependencies]
wasm_liveview = { git = "https://github.com/atavistock/wasm_liveview" }
```

The crate only pulls in `wasm-bindgen` / `js-sys` / `web-sys` on the `wasm32` target. On non-wasm targets every call stubs to `Ok(())` so the command encoders can be unit-tested without a browser.

## Sending commands to LiveView

Every outbound function is a thin wrapper around one `Phoenix.LiveView.JS` command, ultimately dispatched via `window.liveSocket.execJS(rootEl, ...)`.

```rust
use wasm_liveview as lv;

// Push an event to the root LiveView (ad-hoc JSON).
lv::push_event("submit_word", &serde_json::json!({
    "word": "TRY",
    "route": [0, 1, 2],
}))?;

// Or with a typed payload - no json! allocation, field names checked at compile time.
#[derive(serde::Serialize)]
struct Submit<'a> { word: &'a str, route: &'a [usize] }

lv::push_event("submit_word", &Submit { word: "TRY", route: &[0, 1, 2] })?;

// Push to a component by CID or selector.
lv::push_event_to("#chat", "send", &payload)?;

// Client-side routing.
lv::navigate("/room/42", false)?;  // pushes history
lv::patch("/room/42?tab=chat", true)?;  // replaces history, same LV

// Dispatch a CustomEvent on the LV root (or a selector).
lv::dispatch("wasm:tick", None)?;
lv::dispatch_with("wasm:score", Some("#score"), &serde_json::json!({ "delta": 5 }))?;

// Run a CSS transition.
lv::transition(
    lv::TransitionClasses {
        transition: &["fade-in"],
        start: &["opacity-0"],
        end: &["opacity-100"],
    },
    Some("#board"),
    Some(150),
)?;

// Focus management (uses LV's focus stack).
lv::focus(Some("#first-name"))?;
lv::push_focus(None)?;
lv::pop_focus()?;

// Execute a JS command chain stored in a data-* attribute.
lv::exec_attr("data-show", Some("#modal"))?;
```

All outbound calls are **fire-and-forget**. `execJS` returns no reply, so if you need the server's response, use the hook-backed channel (not yet implemented).

## Receiving server-pushed events

`Phoenix.LiveView.push_event/3` dispatches `phx:<event>` `CustomEvent`s on `window` whose `detail` is the payload. `subscribe` turns that into a typed listener:

```rust
use wasm_liveview as lv;

#[derive(serde::Deserialize)]
struct Score { value: u32 }

let sub = lv::subscribe::<Score, _>("score_update", |s| {
    web_sys::console::log_1(&format!("score is now {}", s.value).into());
})?;

// `sub` removes the listener when dropped. To listen for the lifetime of the page:
sub.forget();
```

Deserialization failures are logged via `console.error` and the handler is skipped. Malformed payloads will never panic your wasm module.

## How it works

- **Outbound.** Each command is encoded as `[[op, args]]` JSON and passed to `window.liveSocket.execJS(rootEl, commandJson)`. This is exactly the format LiveView's own `phx-click={JS.push(...)}` attributes use, so the server sees your events indistinguishably from clicks.
- **Inbound.** Phoenix already broadcasts `push_event/3` payloads as `phx:<event>` window `CustomEvent`s; `subscribe` just adds a typed `addEventListener` and JSON-decodes `event.detail` into your `T`.
- **Caching.** wasm32 is single-threaded and a page hosts a single `liveSocket`, so `window`, `document`, `liveSocket`, and its `execJS` function are cached in a `thread_local!` for the page's lifetime. The `[data-phx-session]` root element is re-queried per call, because LV navigation can swap it out.
