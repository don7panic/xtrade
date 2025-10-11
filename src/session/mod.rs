//! Interactive session management module
//!
//! This module provides the core components for managing interactive terminal sessions,
//! including session lifecycle, command routing, and event processing.

pub mod action_channel;
pub mod command_router;
pub mod session_manager;

pub use action_channel::{ActionChannel, SessionEvent, StatusInfo};
pub use command_router::CommandRouter;
pub use session_manager::{SessionConfig, SessionManager, SessionState, SessionStats};
