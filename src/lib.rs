//! Scenar's core, free of any browser: the story tree, the dependency engine
//! that checks it, the shared document that lets several people edit it at
//! once, and the example story a first visit opens on.

pub mod engine;
pub mod example;
pub mod model;
pub mod rank;
pub mod shared;
