//! The apps of the Microsoft Store (packages) installed for the current user. Read only:
//! the package manager lists them without administrator rights.

use purifex_domain::apps::{AppKind, InstalledApp};
use purifex_domain::ports::{AppInventory, Uninstaller};
use windows::core::HSTRING;
use windows::ApplicationModel::{Package, PackageSignatureKind};
use windows::Management::Deployment::{PackageManager, RemovalOptions};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

pub struct StorePackages;

impl AppInventory for StorePackages {
    /// There is no cheap question for a package: the caller reads the list for these.
    fn still_registered(&self, _: &str) -> Result<bool, String> {
        Err("un paquete de la Tienda se busca en la lista".into())
    }

    fn apps(&self) -> Result<Vec<InstalledApp>, String> {
        init_com();
        let manager = PackageManager::new().map_err(|e| format!("paquetes de la Tienda: {e}"))?;
        let packages = manager
            .FindPackagesByUserSecurityId(&HSTRING::new())
            .map_err(|e| format!("paquetes de la Tienda: {e}"))?;
        Ok(packages.into_iter().filter_map(|p| app_of(&p)).collect())
    }
}

/// The package manager is a Windows Runtime object: the thread that uses it needs COM. It is
/// harmless if the thread already has it (the answer is ignored on purpose).
fn init_com() {
    // SAFETY: initialises COM for the calling thread before the first call that needs it.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
}

/// Removes the packages of the Store for the current user, the way Windows' own list of apps
/// does. It does not wait: removing takes seconds and the window must not freeze, so it
/// runs on its own thread and a failure goes to the log. The apps page notices that the
/// app is gone, as it does for a classic uninstaller.
pub struct StoreUninstaller;

impl Uninstaller for StoreUninstaller {
    fn launch(&self, app: &InstalledApp) -> Result<(), String> {
        if app.kind != AppKind::Store || app.id.is_empty() {
            return Err("esto no es un paquete de la Tienda".into());
        }
        let full_name = app.id.clone();
        std::thread::Builder::new()
            .name("store-remove".into())
            .spawn(move || {
                if let Err(error) = remove(&full_name) {
                    purifex_domain::log_error!("{full_name}: {error}");
                }
            })
            .map(|_| ())
            .map_err(|e| format!("no se pudo iniciar la desinstalación: {e}"))
    }
}

/// Removes the package for the current user and waits for the system to finish.
fn remove(full_name: &str) -> Result<(), String> {
    init_com();
    let manager = PackageManager::new().map_err(|e| e.to_string())?;
    let result = manager
        .RemovePackageWithOptionsAsync(&HSTRING::from(full_name), RemovalOptions::None)
        .and_then(|operation| operation.join())
        .map_err(|e| e.to_string())?;
    match result.ExtendedErrorCode() {
        Ok(code) if code.is_ok() => Ok(()),
        _ => Err(result
            .ErrorText()
            .map(|t| t.to_string())
            .unwrap_or_else(|_| "Windows no quitó el paquete".into())),
    }
}

/// The app a package is, or `None` when it has no name to show. Frameworks, resource
/// packages and the ones Windows ships are listed but hidden, as the registry hides the
/// parts of other programs.
fn app_of(package: &Package) -> Option<InstalledApp> {
    let id = package.Id().ok()?;
    let full_name = id.FullName().ok()?.to_string();
    let name = package
        .DisplayName()
        .ok()
        .map(|n| n.to_string())
        .filter(|n| !n.is_empty())
        .or_else(|| id.Name().ok().map(|n| n.to_string()))?;
    let v = id.Version().ok()?;
    let part_of_the_system = package.IsFramework().unwrap_or(false)
        || package.IsResourcePackage().unwrap_or(false)
        || package.SignatureKind().ok() == Some(PackageSignatureKind::System);
    Some(InstalledApp {
        id: full_name,
        name,
        publisher: package.PublisherDisplayName().ok().map(|p| p.to_string()),
        version: Some(format!(
            "{}.{}.{}.{}",
            v.Major, v.Minor, v.Build, v.Revision
        )),
        install_location: package
            .InstalledPath()
            .ok()
            .map(|p| p.to_string())
            .filter(|p| !p.is_empty()),
        kind: AppKind::Store,
        hidden: part_of_the_system,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "uses the real package manager: tools/check.ps1 runs it on this computer"]
    fn the_store_packages_of_this_computer_can_be_listed() {
        // It depends on the computer: what matters is that the call works and every
        // package it returns is a store app with a name and an id.
        let apps = StorePackages.apps().unwrap();
        assert!(apps
            .iter()
            .all(|a| a.kind == AppKind::Store && !a.name.is_empty() && !a.id.is_empty()));
    }

    #[test]
    fn only_a_package_of_the_store_is_removed_this_way() {
        let classic = InstalledApp {
            id: "HKLM64:x".into(),
            name: "x".into(),
            ..Default::default()
        };
        assert!(StoreUninstaller.launch(&classic).is_err());
    }

    #[test]
    #[ignore = "uses the real package manager: tools/check.ps1 runs it on this computer"]
    fn removing_a_package_that_is_not_installed_fails_with_windows_own_reason() {
        // Nothing of this computer is touched: the package does not exist.
        let error = remove("Purifex.NoExiste_1.0.0.0_x64__0000000000000").unwrap_err();
        assert!(!error.is_empty());
    }
}
