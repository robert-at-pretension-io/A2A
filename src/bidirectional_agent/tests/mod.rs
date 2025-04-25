//! Tests for the Bidirectional Agent module.

#![cfg(feature = "bidir-core")]

mod core; // Tests for Slice 1 components

#[cfg(feature = "bidir-local-exec")]
mod local_exec; // Keep this

#[cfg(feature = "bidir-delegate")]
mod delegation; // Keep this

#[cfg(feature = "bidir-delegate")]
mod property; // Property tests for Slice 3
