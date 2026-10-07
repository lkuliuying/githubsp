mod app;
pub mod catalog;
mod completion;
mod desktop;
pub mod engine;
pub mod error;
pub mod files;
pub mod history;
pub mod intake;
pub mod library;
pub mod limiter;
pub mod manager;
pub mod model;
pub mod network;
pub mod preflight;
mod route_policy;
pub mod source;
pub mod store;
pub mod updates;

pub use app::run;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_v2;
