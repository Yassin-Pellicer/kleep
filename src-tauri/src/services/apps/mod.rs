mod catalog;
mod desktop;
mod icons;
mod launch;
mod search;

pub use catalog::preload;
pub use launch::{launch, launch_by_name};
pub use search::search;
