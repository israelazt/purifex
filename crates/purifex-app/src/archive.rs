//! The recordings made so far: listing them and reading one back.

use purifex_domain::analysis::Analysis;
use purifex_domain::compare::ComparisonReport;
use purifex_domain::ports::SessionArchive;
use purifex_domain::session_csv::{self, ParsedSession};
use purifex_domain::session_id::{sort_newest_first, stamp_of};
use purifex_domain::session_list::SessionRow;

pub struct Archive<A> {
    archive: A,
}

impl<A: SessionArchive> Archive<A> {
    pub fn new(archive: A) -> Self {
        Self { archive }
    }

    /// Every recording, the newest first, each read to know how long it is. One
    /// that cannot be read is in the list with the reason, not left out.
    pub fn rows(&self) -> Result<Vec<SessionRow>, String> {
        let mut infos = self.archive.list()?;
        sort_newest_first(&mut infos);
        Ok(infos
            .into_iter()
            .map(|info| {
                let parsed = self.read(&info.id);
                let (seconds, samples, game, problem) = match parsed {
                    Ok(session) => (
                        session.samples.last().map_or(0, |s| s.at_ms / 1000),
                        session.samples.len(),
                        session.header.map(|h| h.game),
                        None,
                    ),
                    Err(error) => (0, 0, None, Some(error)),
                };
                SessionRow {
                    started: stamp_of(&info.id),
                    id: info.id,
                    seconds,
                    samples,
                    has_summary: info.has_summary,
                    game,
                    problem,
                }
            })
            .collect())
    }

    /// The analysis of a recording; with no name, of the newest one.
    pub fn analyze(&self, id: Option<&str>) -> Result<Analysis, String> {
        let id =
            match id {
                Some(id) => id.to_string(),
                None => {
                    let mut infos = self.archive.list()?;
                    sort_newest_first(&mut infos);
                    infos.into_iter().next().map(|info| info.id).ok_or(
                        "todavía no hay grabaciones: graba una partida con `purifex watch`",
                    )?
                }
            };
        Analysis::of(&id, self.read(&id)?)
    }

    /// The newest recording that is a game (see `Analysis::is_game`). A broken
    /// one, or a short test, is skipped so it does not hide the one before it.
    pub fn latest(&self) -> Option<Analysis> {
        let mut infos = self.archive.list().ok()?;
        sort_newest_first(&mut infos);
        infos
            .into_iter()
            .filter_map(|info| self.analyze(Some(&info.id)).ok())
            .find(Analysis::is_game)
    }

    /// The recordings that are games, the newest first (a broken one or a test
    /// is skipped).
    fn games(&self) -> Vec<Analysis> {
        let Ok(mut infos) = self.archive.list() else {
            return Vec::new();
        };
        sort_newest_first(&mut infos);
        infos
            .into_iter()
            .filter_map(|info| self.analyze(Some(&info.id)).ok())
            .filter(Analysis::is_game)
            .collect()
    }

    /// Two recordings compared. With no names, the two newest games.
    pub fn compare(
        &self,
        before: Option<&str>,
        after: Option<&str>,
    ) -> Result<ComparisonReport, String> {
        if let (Some(before), Some(after)) = (before, after) {
            return Ok(ComparisonReport::of(
                &self.analyze(Some(before))?,
                &self.analyze(Some(after))?,
            ));
        }
        let games = self.games();
        match games.as_slice() {
            [newest, previous, ..] => Ok(ComparisonReport::of(previous, newest)),
            _ => Err(format!(
                "hacen falta al menos dos partidas grabadas (de 2 minutos o más, con el juego abierto); hay {}",
                games.len()
            )),
        }
    }

    /// One recording, read back.
    pub fn read(&self, id: &str) -> Result<ParsedSession, String> {
        session_csv::parse(&self.archive.read_csv(id)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeArchive;

    const GAME: &str = "t_s,cpu_total,cpu_max_core,game_cpu,game_mb
1.0,1.0,1.0,5.0,900
300.0,1.0,1.0,5.0,900
";
    const SHORT: &str = "t_s,cpu_total,cpu_max_core\n1.0,10.0,20.0\n2.0,11.0,21.0\n";
    const WITH_HEADER: &str = "# purifex-session v1 game=\"League of Legends.exe\" cores=12 started=2026-11-18\nt_s,cpu_total,cpu_max_core\n1.0,1.0,1.0\n61.0,1.0,1.0\n";

    #[test]
    fn the_list_is_newest_first_with_duration_and_game() {
        let archive = Archive::new(FakeArchive::with(&[
            ("lol-20261118-145915", SHORT),
            ("lol-20261118-165329", WITH_HEADER),
        ]));
        let rows = archive.rows().unwrap();
        assert_eq!(rows[0].id, "lol-20261118-165329");
        assert_eq!((rows[0].seconds, rows[0].samples), (61, 2));
        assert_eq!(rows[0].game.as_deref(), Some("League of Legends.exe"));
        assert_eq!((rows[1].seconds, rows[1].game.clone()), (2, None));
    }

    #[test]
    fn a_recording_that_cannot_be_read_is_listed_with_its_reason() {
        let archive = Archive::new(FakeArchive::with(&[
            ("lol-20261118-145915", SHORT),
            ("lol-20261118-120000", "esto no es un csv"),
        ]));
        let rows = archive.rows().unwrap();
        assert_eq!(rows.len(), 2);
        let broken = rows.iter().find(|r| r.id == "lol-20261118-120000").unwrap();
        assert!(broken.problem.as_deref().unwrap().contains("t_s"));
        assert!(rows
            .iter()
            .find(|r| r.id == "lol-20261118-145915")
            .unwrap()
            .problem
            .is_none());
    }

    #[test]
    fn with_no_name_the_newest_recording_is_analyzed() {
        let archive = Archive::new(FakeArchive::with(&[
            ("lol-20261118-145915", SHORT),
            ("lol-20261118-165329", WITH_HEADER),
        ]));
        assert_eq!(archive.analyze(None).unwrap().id, "lol-20261118-165329");
        assert_eq!(
            archive
                .analyze(Some("lol-20261118-145915"))
                .unwrap()
                .summary
                .samples,
            2
        );
    }

    #[test]
    fn the_latest_skips_a_broken_recording_and_a_short_test() {
        let archive = Archive::new(FakeArchive::with(&[
            ("lol-20261118-145915", GAME),
            ("lol-20261118-165329", "roto"),
            ("lol-20261118-170557", SHORT),
        ]));
        assert_eq!(archive.latest().unwrap().id, "lol-20261118-145915");
        assert!(Archive::new(FakeArchive::default()).latest().is_none());
        let only_tests = Archive::new(FakeArchive::with(&[("lol-20261118-170557", SHORT)]));
        assert!(only_tests.latest().is_none(), "a test is not a game");
    }

    #[test]
    fn with_no_names_the_two_newest_games_are_compared_oldest_first() {
        let archive = Archive::new(FakeArchive::with(&[
            ("lol-20261118-100000", GAME),
            ("lol-20261118-120000", GAME),
            ("lol-20261118-130000", SHORT),
            ("lol-20261118-140000", "roto"),
        ]));
        let report = archive.compare(None, None).unwrap();
        assert_eq!(
            (report.before.as_str(), report.after.as_str()),
            ("lol-20261118-100000", "lol-20261118-120000")
        );
        let named = archive
            .compare(Some("lol-20261118-120000"), Some("lol-20261118-100000"))
            .unwrap();
        assert_eq!(named.before, "lol-20261118-120000");
    }

    #[test]
    fn comparing_with_fewer_than_two_games_says_how_many_there_are() {
        let one = Archive::new(FakeArchive::with(&[
            ("lol-20261118-100000", GAME),
            ("lol-20261118-110000", SHORT),
        ]));
        let error = one.compare(None, None).unwrap_err();
        assert!(
            error.contains("al menos dos partidas") && error.contains("hay 1"),
            "{error}"
        );
        assert!(one
            .compare(Some("lol-20261118-100000"), Some("zzz"))
            .unwrap_err()
            .contains("no existe"));
    }

    #[test]
    fn analyzing_says_why_it_cannot() {
        let archive = Archive::new(FakeArchive::default());
        assert!(archive.analyze(None).unwrap_err().contains("purifex watch"));
        assert!(archive
            .analyze(Some("nope"))
            .unwrap_err()
            .contains("no existe"));
    }

    #[test]
    fn a_failing_archive_is_an_error_and_no_recordings_is_an_empty_list() {
        let failing = FakeArchive {
            fails: Some("disco no disponible".into()),
            ..FakeArchive::default()
        };
        assert_eq!(
            Archive::new(failing).rows().unwrap_err(),
            "disco no disponible"
        );
        assert!(Archive::new(FakeArchive::default())
            .rows()
            .unwrap()
            .is_empty());
    }
}
