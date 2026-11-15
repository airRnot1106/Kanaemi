#![cfg(target_os = "macos")]

use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use kanaemi_engine::Fallback;
use kanaemi_neural::{NeuralConverter, Status};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kanaemi-neural-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Opens the model at `path` and waits until it is read or cannot be.
fn opened(path: &Path) -> NeuralConverter {
    let converter = NeuralConverter::open(path);
    let started = Instant::now();
    while converter.status() == Status::Loading {
        assert!(started.elapsed() < Duration::from_secs(60), "still loading");
        thread::sleep(Duration::from_millis(10));
    }
    converter
}

#[test]
fn opening_returns_before_the_model_is_read_and_converts_to_nothing_until_then() {
    let started = Instant::now();
    let converter = NeuralConverter::open(&temp_dir("missing").join("model.gguf"));
    let surfaces = converter.convert("きかいがくしゅう", "", 3);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(surfaces, Vec::<String>::new());
}

#[test]
fn a_missing_model_file_converts_to_nothing() {
    let converter = opened(&temp_dir("absent").join("model.gguf"));
    assert_eq!(converter.status(), Status::Failed);
    assert_eq!(
        converter.convert("きかいがくしゅう", "", 3),
        Vec::<String>::new()
    );
}

#[test]
fn a_model_file_without_its_tokenizer_converts_to_nothing() {
    let dir = temp_dir("no-tokenizer");
    std::fs::write(dir.join("model.gguf"), b"GGUF").unwrap();
    let converter = opened(&dir.join("model.gguf"));
    assert_eq!(converter.status(), Status::Failed);
    assert_eq!(
        converter.convert("きかいがくしゅう", "", 3),
        Vec::<String>::new()
    );
}

#[test]
fn a_file_that_is_no_model_converts_to_nothing() {
    let dir = temp_dir("not-a-model");
    std::fs::write(dir.join("model.gguf"), b"not a model").unwrap();
    std::fs::write(dir.join("tokenizer.json"), b"{}").unwrap();
    let converter = opened(&dir.join("model.gguf"));
    assert_eq!(converter.status(), Status::Failed);
    assert_eq!(
        converter.convert("きかいがくしゅう", "", 3),
        Vec::<String>::new()
    );
}

/// Runs only with the model file KANAEMI_NEURAL_MODEL names: the model is
/// too large to keep with the tests.
#[test]
fn the_model_converts_a_reading_to_at_most_as_many_surfaces_as_asked_best_first_in_time() {
    let Some(path) = std::env::var_os("KANAEMI_NEURAL_MODEL") else {
        eprintln!("skipped: KANAEMI_NEURAL_MODEL is not set");
        return;
    };
    let converter = opened(Path::new(&path));
    assert_eq!(converter.status(), Status::Ready);
    let started = Instant::now();
    let surfaces = converter.convert("きかいがくしゅう", "", 3);
    let elapsed = started.elapsed();
    eprintln!("{surfaces:?} in {elapsed:?}");
    assert_eq!(surfaces.first().map(String::as_str), Some("機械学習"));
    assert!(surfaces.len() <= 3);
    assert!(elapsed < Duration::from_millis(100), "{elapsed:?}");
}
