//! # Bridge between sync and async
//!
//! `stackful` is a minimalistic library that allows you to convert between sync code and async
//! freely.
//!
//! A major issue of Rust's `async` is that it is very intrusive: it pretty much requires a whole
//! program rewrite to convert synchronous code to async, and it would require many libraries to
//! support both variants.
//!
//! Currently, we could use `async_std::task::spawn_blocking` or equivalents to convert blocking
//! IO into async IO using thread pools, or use `async_std::task::block_on` to use an async library
//! in blocking code. But it is still quite problematic if you want to use a synchronous middleware
//! inside an generally asynchronous program.
//!
//! `stackful` aims to blur the boundary by using stackful coroutines to allow arbitary yield
//! points within other-wise synchronous code.
//!
//! ## Usage
//! Use `wait` to convert an async value to a sync value, similar to `.await` or `block_on`.
//!
//! ```
//! # use std::time::Duration;
//! use stackful::wait;
//! # async_std::task::block_on(stackful::stackful(|| {
//! wait(async_std::task::sleep(Duration::from_secs(1)));
//! # }));
//! ```
//!
//! Use `stackful` to convert a synchronous function into a `Future`:
//! ```
//! use stackful::stackful;
//! # async_std::task::spawn(async {
//! async_std::task::spawn_local(stackful(|| {
//!     // Synchronous code
//!     // This shouldn't block, however
//! }));
//! # });
//! ```
//! You can combine these functions, note that we seamlessly handle nested functions:
//! ```
//! # use std::time::Duration;
//! use stackful::{stackful, wait};
//!
//! fn maybe_sleep(dur: Option<Duration>) {
//!     // This is not possible under `async`!
//!     dur.map(|x| wait(async_std::task::sleep(x)));
//! }
//!
//! # async_std::task::spawn(async {
//! async_std::task::spawn_local(async {
//!     stackful(|| maybe_sleep(Some(Duration::from_secs(1)))).await
//! });
//! # });
//! ```

#![cfg_attr(feature = "nightly", feature(generator_trait))]

mod fiber;
pub mod generator;
mod page_size;

#[cfg(feature = "future")]
pub mod future;
#[cfg(feature = "future")]
#[doc(inline)]
pub use future::{stackful, wait};
