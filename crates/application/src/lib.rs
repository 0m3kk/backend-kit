//! Generic application-layer primitives.

pub mod context;
pub mod state;

pub use context::ExecutionContext;
pub use state::{Decision, State, StateLoadError, StateLoader, StateSet};
