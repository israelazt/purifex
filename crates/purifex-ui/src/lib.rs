//! The look of Purifex: the page that the program serves to its own window. The
//! files are included in the program (nothing is read from disk or downloaded),
//! and there is no logic of decision in them: what the page shows is decided in
//! `purifex-domain`. See `docs/DESIGN.md`.

#![forbid(unsafe_code)]

pub mod canvas;
pub mod drivers_page;
mod overview_cards;
pub mod overview_page;
mod overview_panels;
pub mod page;
pub mod sessions_page;
pub mod shell;
pub mod theme;
pub mod ui;
pub mod widgets;

pub use ui::{Ui, UiOutput};

/// A file of the page.
#[derive(Debug, Clone, Copy)]
pub struct Asset {
    pub path: &'static str,
    pub content_type: &'static str,
    pub body: &'static [u8],
}

const ASSETS: [Asset; 5] = [
    Asset {
        path: "/index.html",
        content_type: "text/html; charset=utf-8",
        body: include_bytes!("../assets/index.html"),
    },
    Asset {
        path: "/app.css",
        content_type: "text/css; charset=utf-8",
        body: include_bytes!("../assets/app.css"),
    },
    Asset {
        path: "/app.js",
        content_type: "text/javascript; charset=utf-8",
        body: include_bytes!("../assets/app.js"),
    },
    Asset {
        path: "/icons.js",
        content_type: "text/javascript; charset=utf-8",
        body: include_bytes!("../assets/icons.js"),
    },
    Asset {
        path: "/LICENSE-lucide.txt",
        content_type: "text/plain; charset=utf-8",
        body: include_bytes!("../assets/LICENSE-lucide.txt"),
    },
];

/// The file served at `path`. `/` is the page. Anything not in the list (a path
/// that climbs, another file) is `None`: nothing is ever read from disk.
pub fn asset(path: &str) -> Option<&'static Asset> {
    let path = if path == "/" { "/index.html" } else { path };
    ASSETS.iter().find(|a| a.path == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(path: &str) -> &'static str {
        std::str::from_utf8(asset(path).unwrap().body).unwrap()
    }

    #[test]
    fn the_root_is_the_page_and_nothing_else_is_served() {
        assert_eq!(asset("/").unwrap().path, "/index.html");
        for path in [
            "/nope",
            "/../Cargo.toml",
            "/app.js/",
            "app.js",
            "/%2e%2e/x",
            "",
            "/index.html?x",
        ] {
            assert!(asset(path).is_none(), "{path}");
        }
    }

    #[test]
    fn every_file_has_content_and_the_right_type() {
        for a in ASSETS {
            assert!(!a.body.is_empty(), "{}", a.path);
            assert!(
                std::str::from_utf8(a.body).is_ok(),
                "{} is not UTF-8",
                a.path
            );
        }
        assert!(asset("/app.js")
            .unwrap()
            .content_type
            .starts_with("text/javascript"));
        assert!(asset("/app.css")
            .unwrap()
            .content_type
            .starts_with("text/css"));
    }

    #[test]
    fn the_page_loads_only_files_that_exist_and_no_inline_script() {
        let html = text("/");
        for reference in ["/app.css", "/icons.js", "/app.js"] {
            assert!(html.contains(reference), "{reference}");
            assert!(asset(reference).is_some());
        }
        assert!(
            !html.contains("http://") && !html.contains("https://"),
            "nothing external"
        );
        // The policy of the server only allows scripts from the page itself.
        let scripts = html.matches("<script").count();
        assert_eq!(
            scripts,
            html.matches("<script src=").count(),
            "no inline script"
        );
    }

    /// The icon names drawn by the page: those given to `ic(...)`, the second
    /// argument of `navItem(...)` and the values of the category table.
    fn icons_used() -> Vec<String> {
        let js = text("/app.js");
        let quoted = |segment: &str| -> Vec<String> {
            segment
                .split('\'')
                .skip(1)
                .step_by(2)
                .map(str::to_string)
                .collect()
        };
        let mut used = Vec::new();
        for call in js.split("ic(").skip(1) {
            if let Some(args) = call.split(')').next() {
                used.extend(quoted(args));
            }
        }
        for call in js.split("navItem(").skip(1) {
            if let Some(args) = call.split(')').next() {
                used.extend(quoted(args).into_iter().skip(1).take(1));
            }
        }
        let table = js
            .split("CATEGORY_ICON = {")
            .nth(1)
            .and_then(|t| t.split('}').next())
            .unwrap_or("");
        for value in table.split(':').skip(1) {
            used.extend(quoted(value.split(',').next().unwrap_or("")));
        }
        used.sort();
        used.dedup();
        used
    }

    #[test]
    fn no_icon_the_page_asks_for_is_missing() {
        let icons = text("/icons.js");
        let used = icons_used();
        assert!(used.len() >= 20, "the scan found too few icons: {used:?}");
        for name in &used {
            assert!(
                icons.contains(&format!("\"{name}\":")),
                "falta el icono «{name}»"
            );
        }
    }

    #[test]
    fn the_icons_keep_their_license() {
        assert!(text("/icons.js").contains("ISC"));
        assert!(text("/LICENSE-lucide.txt").contains("ISC License"));
    }

    #[test]
    fn the_page_escapes_what_comes_from_the_system() {
        // Every `innerHTML` is built through `esc`; the helper exists and escapes the five characters.
        let js = text("/app.js");
        assert!(js.contains("function esc("));
        for entity in ["&amp;", "&lt;", "&gt;", "&quot;", "&#39;"] {
            assert!(js.contains(entity), "{entity}");
        }
    }
}
