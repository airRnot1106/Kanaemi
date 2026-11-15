//! Candidates for a reading no dictionary has, from a converter that is not
//! a dictionary.

use std::rc::Rc;

use kanaemi_core::Candidate;

use super::{Engine, Found};
use crate::{CandidateFacts, nfc};

/// How many candidates the fallback is asked for.
const MAX_FALLBACK_CANDIDATES: usize = 3;

/// A converter asked for a reading no dictionary has.
pub trait Fallback {
    /// What candidates from it carry as their source.
    fn name(&self) -> &str;

    /// At most `limit` surfaces for `reading` typed after `context`, best
    /// first; none when it has none in time.
    fn convert(&self, reading: &str, context: &str, limit: usize) -> Vec<String>;
}

impl Engine {
    /// Asks `fallback` from now on for a reading no dictionary has. The
    /// engines of one process may share it.
    pub fn set_fallback(&mut self, fallback: Option<Rc<dyn Fallback>>) {
        self.fallback = fallback;
    }

    /// Runs `run` without asking the fallback, as for a reading that is only
    /// shown to complete with.
    pub(super) fn without_fallback<T>(&self, run: impl FnOnce() -> T) -> T {
        let before = self.without_fallback.replace(true);
        let result = run();
        self.without_fallback.set(before);
        result
    }

    /// The fallback's candidates for `reading`, which no dictionary has,
    /// those picked again and again first. None with okurigana, which it is
    /// not told of, nor while a key is only tried: it may take as long as a
    /// key can wait, and the key itself asks again.
    pub(super) fn fall_back(&self, reading: &str, okurigana: Option<&str>) -> Vec<Candidate> {
        let asked =
            okurigana.is_none() && !self.without_functions.get() && !self.without_fallback.get();
        let Some(fallback) = self.fallback.as_deref().filter(|_| asked) else {
            return Vec::new();
        };
        let user = self.user.dictionary();
        let mut found: Vec<Found> = Vec::new();
        for surface in fallback
            .convert(reading, &self.field.context, MAX_FALLBACK_CANDIDATES)
            .into_iter()
            .take(MAX_FALLBACK_CANDIDATES)
        {
            let surface = nfc(&surface);
            let left_out = surface.is_empty()
                || surface == reading
                || user.is_hidden(reading, &surface)
                || found.iter().any(|f| f.facts.surface == surface);
            if !left_out {
                found.push(Found {
                    facts: CandidateFacts::plain(surface),
                    okuri: false,
                    value: 0,
                });
            }
        }
        self.favor(reading, found, None)
            .into_iter()
            .map(|f| Candidate {
                surface: f.facts.surface,
                source: Some(fallback.name().to_owned()),
                preview: None,
            })
            .collect()
    }
}
