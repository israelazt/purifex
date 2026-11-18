//! The kinds of hardware the interface filters by. Windows names a device's
//! class with its own words (`MEDIA`, `HIDClass`, `Net`…); people look for "audio"
//! or "network".

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Gpu,
    Audio,
    Network,
    Input,
    Bluetooth,
    Storage,
    Monitor,
    Camera,
    System,
    Other,
}

pub const ALL_CATEGORIES: [Category; 10] = [
    Category::Gpu,
    Category::Audio,
    Category::Network,
    Category::Input,
    Category::Bluetooth,
    Category::Storage,
    Category::Monitor,
    Category::Camera,
    Category::System,
    Category::Other,
];

impl Category {
    /// The name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Gpu => "GPU",
            Self::Audio => "Audio",
            Self::Network => "Red",
            Self::Input => "Entrada",
            Self::Bluetooth => "Bluetooth",
            Self::Storage => "Almacenamiento",
            Self::Monitor => "Monitores",
            Self::Camera => "Cámaras",
            Self::System => "Sistema",
            Self::Other => "Otros",
        }
    }
}

/// The category of a device from its Windows class, whatever its case.
pub fn category_of(class: &str) -> Category {
    match class.to_ascii_lowercase().as_str() {
        "display" => Category::Gpu,
        "media" | "audioendpoint" | "audioprocessingobject" => Category::Audio,
        "net" | "netservice" | "nettrans" | "netclient" => Category::Network,
        "hidclass" | "keyboard" | "mouse" => Category::Input,
        "bluetooth" => Category::Bluetooth,
        "diskdrive" | "scsiadapter" | "hdc" | "volume" | "cdrom" => Category::Storage,
        "monitor" => Category::Monitor,
        "camera" | "image" => Category::Camera,
        "system" | "firmware" | "processor" | "computer" | "usb" | "pnpprinters" | "battery" => {
            Category::System
        }
        _ => Category::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_classes_map_to_what_people_look_for() {
        for (class, category) in [
            ("Display", Category::Gpu),
            ("MEDIA", Category::Audio),
            ("media", Category::Audio),
            ("Net", Category::Network),
            ("HIDClass", Category::Input),
            ("Keyboard", Category::Input),
            ("Bluetooth", Category::Bluetooth),
            ("DiskDrive", Category::Storage),
            ("HDC", Category::Storage),
            ("Monitor", Category::Monitor),
            ("Camera", Category::Camera),
            ("Firmware", Category::System),
            ("USB", Category::System),
            ("SomethingNew", Category::Other),
            ("", Category::Other),
        ] {
            assert_eq!(category_of(class), category, "{class}");
        }
    }

    #[test]
    fn every_category_has_a_label_and_a_place_in_the_list() {
        for category in ALL_CATEGORIES {
            assert!(!category.label().is_empty());
        }
        let mut unique = ALL_CATEGORIES.to_vec();
        unique.dedup();
        assert_eq!(unique.len(), 10);
    }
}
