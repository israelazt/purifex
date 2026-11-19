//! The rules and the data of Purifex. No Windows, no threads, no disk, no
//! network: the system is reached only through the traits in [`ports`].
//! See `ARCHITECTURE.md`.

#![forbid(unsafe_code)]

pub mod analysis;
pub mod app_traces;
pub mod apps;
pub mod cleanup_log;
pub mod compare;
pub mod date;
pub mod device;
pub mod device_category;
pub mod diagnosis;
pub mod draw;
pub mod driver_candidate;
pub mod driver_offers;
pub mod driver_tiers;
pub mod drivers_view;
pub mod fix;
pub mod icons;
pub mod log;
pub mod overview;
pub mod ports;
pub mod process_group;
pub mod report;
pub mod sample;
pub mod seeds;
pub mod session;
pub mod session_csv;
pub mod session_header;
pub mod session_id;
pub mod session_list;
pub mod text;
pub mod ui_event;
pub mod version;
pub mod view;
pub mod yaml_key;
