# Wasm/LiveView Bridge

A two-way bridge between wasm-bindgen Rust and a mounted Phoenix LiveView.

Outbound, it wraps the `Phoenix.LiveView.JS` command set so Rust/wasm code can fire LV events, navigate, dispatch DOM events, run transitions, and manage focus - without trampolining through hidden `phx-*` trigger elements.

Inbound, it lets Rust subscribe to server-pushed events from `Phoenix.LiveView.push_event/3`.

Written for game code that renders in wasm but wants the server to own authentication, state, and persistence.

## Status

Early. Extracted from two wasm game projects that both use it today. The outbound side wraps the common `JS` commands; the inbound side covers server-pushed events via `window` `phx:<event>` listeners. Not yet implemented: client-to-server `pushEvent` with a reply callback, which needs a LiveView hook on the JS side.

## Install

```toml
[dependencies]
wasm_liveview = "0.3"
```

`wasm-bindgen` / `js-sys` / `web-sys` are only pulled in on `wasm32`. On other targets every call stubs to `Ok(())` so encoders can be unit-tested without a browser.

## Sending commands to LiveView

Every outbound function is a thin wrapper around one `Phoenix.LiveView.JS` command, dispatched via `window.liveSocket.execJS(rootEl, ...)`.

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

All outbound calls are **fire-and-forget**. `execJS` returns no reply; for server responses, use the hook-backed channel (not yet implemented).

## Receiving server-pushed events

`Phoenix.LiveView.push_event/3` dispatches `phx:<event>` `CustomEvent`s on `window` whose `detail` is the payload. `subscribe` turns that into a typed listener:

```rust
use wasm_liveview as lv;

#[derive(serde::Deserialize)]
struct Score { value: u32 }

let sub = lv::subscribe::<Score, _>("score_update", |s| {
    web_sys::console::log_1(&format!("score is now {}", s.value).into());
})?;

// `sub` removes the listener when dropped. To listen for the page lifetime:
sub.forget();
```

Deserialization failures are logged via `console.error` and skipped - malformed payloads never panic the wasm module.

## Reading and watching server state

LiveView templates often render authoritative state as `data-*` attributes on a hidden "bridge" element. `Bridge` reads those attributes with typed parsing and watches them for changes via a `MutationObserver` - no polling, no custom JS hook.

```html
<div id="my-bridge"
     phx-update="ignore"
     data-round-status="playing"
     data-remaining-seconds="42"></div>
```

```rust
use wasm_liveview::Bridge;

let bridge = Bridge::new("#my-bridge");

// One-shot reads. None if missing, empty, or unparseable.
let status: Option<String> = bridge.attr("data-round-status");
let remaining: Option<f32> = bridge.read("data-remaining-seconds");
let guesses: Option<std::collections::HashMap<String, Vec<usize>>>
    = bridge.read_json("data-saved-guesses");

// Watch for updates. Fires on each mutation that decodes cleanly, plus
// once on initial page ready and once per reconnect with the current value -
// no separate "read once" or "re-sync after disconnect" step needed.
let sub = bridge.watch::<f32, _>("data-remaining-seconds", |secs| {
    web_sys::console::log_1(&format!("{secs} seconds left").into());
})?;
sub.forget();
```

`phx-update="ignore"` is recommended so LV mutates the element's attributes in place; if the element is replaced, the `MutationObserver` silently stops firing (the `phx:page-loading-stop` re-delivery still works, since it re-queries by selector).

## How it works

- **Outbound.** Each command is encoded as `[[op, args]]` JSON and passed to `window.liveSocket.execJS(rootEl, commandJson)` - the same format LiveView's own `phx-click={JS.push(...)}` attributes use, so the server sees your events indistinguishably from clicks.
- **Inbound.** Phoenix broadcasts `push_event/3` payloads as `phx:<event>` window `CustomEvent`s; `subscribe` adds a typed `addEventListener` and JSON-decodes `event.detail` into your `T`.
- **Caching.** wasm32 is single-threaded and a page hosts a single `liveSocket`, so `window`, `document`, `liveSocket`, and `execJS` are cached in a `thread_local!`. The cache is cleared on every `phx:page-loading-stop` so reconnects pick up the fresh `liveSocket` rather than the pre-disconnect reference.
- **Bridge reads.** `Bridge::new(selector)` stores only the selector; the element is re-queried per call, so a `Bridge` survives LV navigations. `watch` wraps a `MutationObserver` with an attribute filter, and re-fires on every `phx:page-loading-stop` so initial-load and reconnect cases don't need separate handling.

## Documentation

Every public item has rustdoc. Build locally:

```sh
cargo doc --no-deps --open
```

To mirror docs.rs (feature-gate badges, etc.), build with the `docsrs` cfg on nightly:

```sh
RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --no-deps --open
```

Once published, docs.rs builds the same configuration automatically - the `[package.metadata.docs.rs]` block in `Cargo.toml` pins the target to `wasm32-unknown-unknown` and enables `--cfg docsrs`.
