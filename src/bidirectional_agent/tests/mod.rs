//! Tests for the Bidirectional Agent module.

#![cfg(feature = "bidir-core")]

mod core; // Tests for Slice 1 components

#[cfg(feature = "bidir-local-exec")]
mod local_exec; // Keep this

#[cfg(feature = "bidir-delegate")]
mod delegation; // Keep this

#[cfg(feature = "bidir-delegate")]
mod property; // Property tests for Slice 3
//! Tests for the Bidirectional Agent module.

#![cfg(feature = "bidir-core")] // Guard entire test module

mod core; // Tests for Slice 1 components

#[cfg(feature = "bidir-local-exec")]
mod local_exec;

#[cfg(feature = "bidir-delegate")]
mod delegation;

#[cfg(feature = "bidir-delegate")]
mod property;

// Add integration tests module conditionally
#[cfg(all(feature = "bidir-core", feature = "bidir-local-exec"))]
mod integration;
