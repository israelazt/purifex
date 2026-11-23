//! The install monitor on Windows: the picture of the places where installers leave things,
//! the logs kept on disk, running an installer and asking for a file.

mod logs;
mod picker;
mod picture;
mod runner;

pub use logs::LocalInstallLogs;
pub use picker::WindowsFilePicker;
pub use picture::WindowsPicture;
pub use runner::ShellInstaller;
