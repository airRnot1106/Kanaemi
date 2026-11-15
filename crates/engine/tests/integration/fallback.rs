use std::cell::RefCell;
use std::rc::Rc;

use kanaemi_config::default_romaji_table;
use kanaemi_core::{
    Candidate, Config, Converter, Core, Event, Key, KeyEvent, KeyKind, Modifiers, Output,
};
use kanaemi_engine::{Engine, Fallback, Slot, TextDictionary};

use crate::common::{Discard, Learn, dictionary};

/// Gives the same surfaces for every reading, and keeps what it was asked.
#[derive(Default)]
struct Fake {
    surfaces: Vec<&'static str>,
    /// The reading, the context and the limit of each call.
    asked: RefCell<Vec<(String, String, usize)>>,
}

impl Fake {
    fn giving(surfaces: &[&'static str]) -> Rc<Self> {
        Rc::new(Self {
            surfaces: surfaces.to_vec(),
            asked: RefCell::default(),
        })
    }

    fn times_asked(&self) -> usize {
        self.asked.borrow().len()
    }
}

impl Fallback for Fake {
    fn name(&self) -> &str {
        "fake"
    }

    fn convert(&self, reading: &str, context: &str, limit: usize) -> Vec<String> {
        self.asked
            .borrow_mut()
            .push((reading.to_owned(), context.to_owned(), limit));
        self.surfaces.iter().map(|s| (*s).to_owned()).collect()
    }
}

/// An engine whose only word is 記者 for きしゃ, falling back on `fake`.
fn engine(fake: &Rc<Fake>) -> Engine {
    let mut e = Engine::new(
        [
            Slot::UserCustom,
            Slot::Dictionary(dictionary("きしゃ\t記者\nか*k\t書\n")),
        ],
        TextDictionary::parse_user_custom("").0,
        Discard,
    );
    e.set_fallback(Some(fake.clone() as Rc<dyn Fallback>));
    e
}

fn surfaces(e: &Engine, reading: &str) -> Vec<String> {
    e.convert(reading, None)
        .into_iter()
        .map(|c| c.surface)
        .collect()
}

#[test]
fn a_reading_no_dictionary_has_converts_to_what_the_fallback_gives_best_first() {
    let fake = Fake::giving(&["機械学習", "機会学習"]);
    assert_eq!(
        surfaces(&engine(&fake), "きかいがくしゅう"),
        ["機械学習", "機会学習"]
    );
}

#[test]
fn the_fallback_is_not_asked_for_a_reading_a_dictionary_has() {
    let fake = Fake::giving(&["汽車"]);
    assert_eq!(surfaces(&engine(&fake), "きしゃ"), ["記者"]);
    assert_eq!(fake.times_asked(), 0);
}

#[test]
fn the_fallback_is_asked_for_three_and_what_it_gives_past_three_is_cut() {
    let fake = Fake::giving(&["一", "二", "三", "四"]);
    assert_eq!(surfaces(&engine(&fake), "かず"), ["一", "二", "三"]);
    assert_eq!(fake.asked.borrow()[0].2, 3);
}

#[test]
fn a_candidate_from_the_fallback_carries_its_name_as_the_source() {
    let fake = Fake::giving(&["機械学習"]);
    let candidates = engine(&fake).convert("きかいがくしゅう", None);
    assert_eq!(
        candidates,
        [Candidate {
            surface: "機械学習".to_owned(),
            source: Some("fake".to_owned()),
            preview: None,
        }]
    );
}

#[test]
fn the_fallback_is_not_asked_when_converting_with_okurigana() {
    let fake = Fake::giving(&["泳ぐ"]);
    assert_eq!(engine(&fake).convert("およぐ", Some("ぐ")), []);
    assert_eq!(fake.times_asked(), 0);
}

#[test]
fn the_fallback_is_not_asked_while_a_key_is_only_tried() {
    let fake = Fake::giving(&["機械学習"]);
    let e = engine(&fake);
    assert_eq!(
        e.without_functions(|| e.convert("きかいがくしゅう", None)),
        []
    );
    assert_eq!(fake.times_asked(), 0);
}

#[test]
fn the_fallback_is_not_asked_for_what_a_reading_to_complete_with_converts_to() {
    let fake = Fake::giving(&["機械学習"]);
    let e = engine(&fake);
    assert_eq!(e.preview("きかいがくしゅう", 3), Vec::<String>::new());
    assert_eq!(fake.times_asked(), 0);
}

#[test]
fn the_fallback_is_given_the_reading_as_nfc_and_the_end_of_the_field_s_text() {
    let fake = Fake::giving(&["学習"]);
    let mut e = engine(&fake);
    e.type_text("機械");
    e.convert("か\u{3099}くしゅう", None);
    assert_eq!(
        fake.asked.borrow()[0],
        ("がくしゅう".to_owned(), "機械".to_owned(), 3)
    );
}

#[test]
fn a_surface_that_is_empty_given_twice_or_the_reading_itself_is_left_out() {
    let fake = Fake::giving(&["", "かず"]);
    assert_eq!(surfaces(&engine(&fake), "かず"), Vec::<String>::new());
    let fake = Fake::giving(&["数", "数", "和"]);
    assert_eq!(surfaces(&engine(&fake), "かず"), ["数", "和"]);
}

#[test]
fn a_surface_the_user_forgot_is_left_out() {
    let fake = Fake::giving(&["機会学習", "機械学習"]);
    let mut e = engine(&fake);
    e.convert("きかいがくしゅう", None);
    e.delete("きかいがくしゅう", "機会学習");
    assert_eq!(surfaces(&e, "きかいがくしゅう"), ["機械学習"]);
}

#[test]
fn committing_a_candidate_from_the_fallback_records_the_pair_without_asking_again() {
    let fake = Fake::giving(&["機械学習"]);
    let mut e = engine(&fake);
    e.convert("きかいがくしゅう", None);
    e.commit("きかいがくしゅう", "機械学習");
    assert_eq!(fake.times_asked(), 1);
    let record = e.take_selections().expect("a pick was recorded").to_text();
    assert!(record.contains("きかいがくしゅう\t機械学習\t"), "{record}");
}

#[test]
fn a_candidate_from_the_fallback_picked_again_and_again_comes_first() {
    let fake = Fake::giving(&["機会学習", "機械学習", "器械学習"]);
    let mut e = engine(&fake);
    for _ in 0..3 {
        e.commit("きかいがくしゅう", "機械学習");
        e.move_focus();
    }
    assert_eq!(
        surfaces(&e, "きかいがくしゅう"),
        ["機械学習", "機会学習", "器械学習"]
    );
}

/// The engine as one field's converter.
struct Shared(Rc<RefCell<Engine>>);

impl Converter for Shared {
    fn convert(&self, reading: &str, okurigana: Option<&str>) -> Vec<Candidate> {
        self.0.borrow().convert(reading, okurigana)
    }
}

/// A field in kana mode, teaching its engine what each key did.
struct Field {
    core: Core<Shared>,
    engine: Rc<RefCell<Engine>>,
    now: u64,
}

impl Field {
    fn new(engine: Engine) -> Self {
        let engine = Rc::new(RefCell::new(engine));
        let config = Config {
            romaji: default_romaji_table(),
            ..Config::default()
        };
        let mut field = Self {
            core: Core::new(Shared(engine.clone()), config),
            engine,
            now: 0,
        };
        field.handle(Event::FocusIn { password: false });
        field.key(Key::ShiftRight, false);
        field
    }

    fn handle(&mut self, event: Event) -> Output {
        let out = self.core.handle(event);
        for effect in &out.effects {
            self.engine.borrow_mut().learn(effect);
        }
        out
    }

    /// Presses `key` and lets it go; what the press gave.
    fn key(&mut self, key: Key, shift: bool) -> Output {
        let mut event = |kind, shift| {
            self.now += 10;
            self.handle(Event::Key(KeyEvent {
                key,
                mods: Modifiers {
                    shift,
                    ..Modifiers::default()
                },
                kind,
                time_ms: self.now,
            }))
        };
        let out = event(KeyKind::Press, shift);
        event(KeyKind::Release, false);
        out
    }

    /// Types `input` and converts it; the candidates shown.
    fn convert(&mut self, input: &str) -> Vec<Candidate> {
        for c in input.chars() {
            self.key(Key::Char(c), false);
        }
        let out = self.key(Key::Space, false);
        out.candidates.expect("candidates are shown").items
    }
}

#[test]
fn candidates_from_the_fallback_come_before_the_katakana_and_can_be_forgotten() {
    let fake = Fake::giving(&["機械", "機会"]);
    let mut field = Field::new(engine(&fake));
    let shown = field.convert(";kikai");
    let shown: Vec<&str> = shown.iter().map(|c| c.surface.as_str()).collect();
    assert_eq!(shown[..3], ["機械", "機会", "キカイ"]);
    field.key(Key::Delete, true);
    field.key(Key::Delete, true);
    field.key(Key::Esc, false);
    field.key(Key::Esc, false);
    let shown = field.convert(";kikai");
    let shown: Vec<&str> = shown.iter().map(|c| c.surface.as_str()).collect();
    assert_eq!(shown[..2], ["機会", "キカイ"]);
}
