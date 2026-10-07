//! TELEVAULT Desktop Core library exporting Tauri application state, IPC DTOs, and command surface.

pub mod builder;
pub mod commands;
pub mod dto;
pub mod error;
pub mod state;

pub use builder::{create_ipc_builder, export_typescript_bindings};
pub use error::{IpcError, IpcResult};
pub use state::DesktopAppState;
