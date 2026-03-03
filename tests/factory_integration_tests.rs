//! Integration tests for factory modules
//!
//! Run with: cargo test --test factory_integration_tests

#[path = "factory/pool_tests.rs"]
mod pool_tests;
#[path = "factory/protocol_tests.rs"]
mod protocol_tests;
#[path = "factory/workspace_tests.rs"]
mod workspace_tests;
