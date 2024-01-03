// Re-exports the implementations

mod combustion_macos;
#[cfg(target_os="macos")]
pub use self::combustion_macos::macos::*;

mod combustion_linux;
#[cfg(target_os="linux")]
pub use self::combustion_linux::linux::*;
