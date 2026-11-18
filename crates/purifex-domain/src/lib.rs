//! The rules and the data of Purifex. No Windows, no threads, no disk, no
//! network: the system is reached only through the traits in [`ports`].
//! See `ARCHITECTURE.md`.

pub mod date;
pub mod device;
pub mod diagnosis;
pub mod log;
pub mod ports;
pub mod report;
pub mod sample;
pub mod session;
pub mod text;
pub mod version;
