//! Two-way bridge between wasm-bindgen Rust and a mounted Phoenix LiveView.
//!
//! Outbound, this crate wraps the [`Phoenix.LiveView.JS`] command set so
//! Rust/wasm code can fire LV events, navigate, dispatch DOM events, run
//! transitions, and manage focus -- without trampolining through hidden
//! `phx-*` trigger elements. Inbound, it lets Rust subscribe to server-pushed
//! events from `Phoenix.LiveView.push_event/3`.
//!
//! Written for game code that renders in wasm but wants the server to own
//! state, routing, and persistence.
//!
//! # Feature summary
//!
//! | Area        | Functions                                                          |
//! |-------------|--------------------------------------------------------------------|
//! | Events up   | [`push_event`], [`push_event_to`]                                  |
//! | Routing     | [`navigate`], [`patch`]                                            |
//! | DOM events  | [`dispatch`], [`dispatch_with`]                                    |
//! | Transitions | [`transition`] + [`TransitionClasses`]                             |
//! | Focus       | [`focus`], [`focus_first`], [`push_focus`], [`pop_focus`]          |
//! | JS attrs    | [`exec_attr`]                                                      |
//! | Events down | [`subscribe`] returning [`Subscription`]                           |
//!
//! # Payload types
//!
//! Commands that carry a payload ([`push_event`], [`push_event_to`],
//! [`dispatch_with`]) accept any `T: serde::Serialize`. Use
//! [`serde_json::json!`] for ad-hoc payloads, or define a
//! `#[derive(Serialize)]` struct for typed, compile-time-checked ones. Both
//! styles are shown on each function's page.
//!
//! # Outbound example
//!
//! ```no_run
//! use wasm_liveview as lv;
//!
//! #[derive(serde::Serialize)]
//! struct Submit<'a> { word: &'a str, route: &'a [usize] }
//!
//! lv::push_event("submit_word", &Submit { word: "TRY", route: &[0, 1, 2] })?;
//!
//! // Client-side routing.
//! lv::navigate("/room/42", false)?;
//!
//! // Run a CSS transition.
//! lv::transition(
//!     lv::TransitionClasses {
//!         transition: &["fade-in"],
//!         start: &["opacity-0"],
//!         end: &["opacity-100"],
//!     },
//!     Some("#board"),
//!     Some(150),
//! )?;
//! # Ok::<(), lv::Error>(())
//! ```
//!
//! # Inbound example
//!
//! ```no_run
//! use wasm_liveview as lv;
//!
//! #[derive(serde::Deserialize)]
//! struct Score { value: u32 }
//!
//! let sub = lv::subscribe::<Score, _>("score_update", |s| {
//!     // handler runs once per server push
//!     let _ = s.value;
//! })?;
//!
//! // Drop the subscription to unsubscribe, or:
//! sub.forget();
//! # Ok::<(), lv::Error>(())
//! ```
//!
//! # Target behavior
//!
//! On `wasm32-*` targets the crate pulls in `wasm-bindgen`, `js-sys`, and
//! `web-sys` and calls `window.liveSocket.execJS` for real. On any other
//! target every outbound function stubs to `Ok(())` and [`subscribe`] returns
//! an inert handle, so the JSON wire-format encoders can be unit-tested
//! without a browser.
//!
//! # Error model
//!
//! Every fallible call returns `Result<_, `[`Error`]`>`. See the [`Error`]
//! enum for the failure modes (missing `window`, uninitialized `liveSocket`,
//! JSON serialization failures, and JS exceptions thrown by `execJS`).
//!
//! [`Phoenix.LiveView.JS`]: https://hexdocs.pm/phoenix_live_view/Phoenix.LiveView.JS.html

#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(target_arch = "wasm32")]
mod cache;
mod commands;
mod error;
mod exec;
mod subscribe;

pub use commands::dispatch::{dispatch, dispatch_with};
pub use commands::exec_attr::exec_attr;
pub use commands::focus::{focus, focus_first, pop_focus, push_focus};
pub use commands::navigate::{navigate, patch};
pub use commands::push::{push_event, push_event_to};
pub use commands::transition::{transition, TransitionClasses};
pub use error::Error;
pub use subscribe::{subscribe, Subscription};
