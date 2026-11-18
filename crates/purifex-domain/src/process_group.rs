//! Families of programs that matter when measuring a game: the same program
//! runs as several processes (the League client is three), and what counts is
//! the family's total.

use serde::Serialize;

/// A family of programs. The game itself is in none of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    /// The League of Legends client (not the game).
    LeagueClient,
    RiotClient,
    /// Riot Vanguard's program.
    AntiCheat,
    Browser,
    /// Windows itself and the antivirus.
    System,
}

pub const ALL_GROUPS: [Group; 5] = [
    Group::LeagueClient,
    Group::RiotClient,
    Group::AntiCheat,
    Group::Browser,
    Group::System,
];

impl Group {
    /// The name used in files (`league_client`).
    pub fn key(self) -> &'static str {
        match self {
            Self::LeagueClient => "league_client",
            Self::RiotClient => "riot_client",
            Self::AntiCheat => "anti_cheat",
            Self::Browser => "browser",
            Self::System => "system",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        ALL_GROUPS.into_iter().find(|g| g.key() == key)
    }

    /// The name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::LeagueClient => "Cliente de League",
            Self::RiotClient => "Riot Client",
            Self::AntiCheat => "Vanguard",
            Self::Browser => "Navegadores",
            Self::System => "Windows y antivirus",
        }
    }
}

/// The family a program belongs to, from its executable name.
pub fn group_of(process: &str) -> Option<Group> {
    let name = process.to_ascii_lowercase();
    Some(match name.as_str() {
        "leagueclient.exe" | "leagueclientux.exe" | "leagueclientuxrender.exe" => {
            Group::LeagueClient
        }
        "riotclientservices.exe"
        | "riot client.exe"
        | "riotclientux.exe"
        | "riotclientuxrender.exe"
        | "riotclientcrashhandler.exe" => Group::RiotClient,
        "vgc.exe" | "vgtray.exe" => Group::AntiCheat,
        "msedge.exe" | "chrome.exe" | "firefox.exe" | "opera.exe" | "brave.exe" | "vivaldi.exe" => {
            Group::Browser
        }
        "system" | "dwm.exe" | "csrss.exe" | "audiodg.exe" | "svchost.exe" | "msmpeng.exe" => {
            Group::System
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_processes_of_the_league_client_are_one_group() {
        for name in [
            "LeagueClient.exe",
            "LeagueClientUx.exe",
            "LeagueClientUxRender.exe",
        ] {
            assert_eq!(group_of(name), Some(Group::LeagueClient), "{name}");
        }
    }

    #[test]
    fn the_game_itself_and_unknown_programs_are_in_no_group() {
        assert_eq!(group_of("League of Legends.exe"), None);
        assert_eq!(group_of("notepad.exe"), None);
        assert_eq!(group_of(""), None);
    }

    #[test]
    fn names_are_matched_without_regard_to_case() {
        assert_eq!(group_of("MSEDGE.EXE"), Some(Group::Browser));
        assert_eq!(group_of("vgc.exe"), Some(Group::AntiCheat));
        assert_eq!(group_of("System"), Some(Group::System));
    }

    #[test]
    fn every_group_has_a_unique_key_that_reads_back() {
        for group in ALL_GROUPS {
            assert_eq!(Group::from_key(group.key()), Some(group));
        }
        assert_eq!(Group::from_key("games"), None);
    }
}
