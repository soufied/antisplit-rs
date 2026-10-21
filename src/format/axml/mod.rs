pub mod model;
pub mod patch;
pub mod reader;
pub mod writer;

#[cfg(test)]
mod tests;

pub use model::Document;
pub use patch::make_standalone;
pub use reader::parse;
pub use writer::write;
