pub(crate) mod board;
mod detail;
mod gallery;
mod tickets;
pub use board::ApiBoard;
pub use detail::*;
pub use gallery::*;
pub use tickets::*;

pub(crate) mod api_detail;
pub(crate) mod list;
