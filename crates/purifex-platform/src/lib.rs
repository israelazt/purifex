//! The Windows adapters of Purifex: they implement the ports of
//! `purifex-domain`. Nothing here knows the application. The decoding of what
//! Windows returns is plain code (`decode`) and is tested everywhere; the calls
//! to the system are only compiled on Windows.

pub mod decode;
pub mod nt_layout;

#[cfg(windows)]
mod app_cleanup;
#[cfg(windows)]
mod apps_registry;
#[cfg(windows)]
mod browser;
#[cfg(windows)]
mod candidates_file;
#[cfg(windows)]
mod clock;
#[cfg(windows)]
mod elevation;
#[cfg(windows)]
mod fixes;
#[cfg(windows)]
mod gfx;
#[cfg(windows)]
mod inventory;
#[cfg(windows)]
mod log_console;
#[cfg(windows)]
mod nt_query;
#[cfg(windows)]
mod processes;
#[cfg(windows)]
mod registry;
#[cfg(windows)]
mod restore_points;
#[cfg(windows)]
mod sampler;
#[cfg(windows)]
mod session_archive;
#[cfg(windows)]
mod session_file;
#[cfg(windows)]
mod store_apps;
#[cfg(windows)]
mod window;
#[cfg(windows)]
mod windows_update;

#[cfg(windows)]
pub use app_cleanup::{
    LocalBackups, LocalCleanup, RegisteredUninstaller, WindowsSeedProbe, WindowsTraceFinder,
};
#[cfg(windows)]
pub use apps_registry::RegistryApps;
#[cfg(windows)]
pub use browser::{BrowserOpener, ExplorerOpener};
#[cfg(windows)]
pub use candidates_file::JsonlCandidates;
#[cfg(windows)]
pub use clock::SystemClock;
#[cfg(windows)]
pub use elevation::WindowsElevation;
#[cfg(windows)]
pub use fixes::{JsonlHistory, SystemFixes};
#[cfg(windows)]
pub use gfx::{render_png, Renderer, TextEngine};
#[cfg(windows)]
pub use inventory::SetupApiInventory;
#[cfg(windows)]
pub use log_console::install_stderr_log;
#[cfg(windows)]
pub use nt_query::running_programs;
#[cfg(windows)]
pub use processes::WindowsProcesses;
#[cfg(windows)]
pub use restore_points::WindowsRestorePoints;
#[cfg(windows)]
pub use sampler::WindowsSampler;
#[cfg(windows)]
pub use session_archive::LocalSessionArchive;
#[cfg(windows)]
pub use session_file::LocalSessionFile;
#[cfg(windows)]
pub use store_apps::{StorePackages, StoreUninstaller};
#[cfg(windows)]
pub use window::run as run_window;
#[cfg(windows)]
pub use windows_update::WindowsUpdateOffers;
