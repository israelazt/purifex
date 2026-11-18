//! The rules and the data of Purifex. No Windows, no threads, no disk, no
//! network: the system is reached only through the traits in [`ports`].
//! See `ARCHITECTURE.md`.

#![forbid(unsafe_code)]

pub mod date;
pub mod device;
pub mod diagnosis;
pub mod log;
pub mod ports;
pub mod process_group;
pub mod report;
pub mod sample;
pub mod session;
pub mod session_csv;
pub mod session_header;
pub mod session_id;
pub mod session_list;
pub mod text;
pub mod version;
