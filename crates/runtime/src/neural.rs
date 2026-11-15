//! The converter by the neural model in the settings folder, asked for
//! readings no dictionary has. Only macOS has one.

use std::path::Path;
use std::rc::Rc;

use kanaemi_engine::Fallback;

/// The converter by the model in `support_dir`, which it goes on reading
/// after this returns; `None` without the model file.
#[cfg(target_os = "macos")]
pub(crate) fn open(support_dir: &Path) -> Option<Rc<dyn Fallback>> {
    use kanaemi_config::{NEURAL_DIR, NEURAL_MODEL_FILE};

    let model = support_dir.join(NEURAL_DIR).join(NEURAL_MODEL_FILE);
    model
        .is_file()
        .then(|| Rc::new(kanaemi_neural::NeuralConverter::open(&model)) as Rc<dyn Fallback>)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn open(_support_dir: &Path) -> Option<Rc<dyn Fallback>> {
    None
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use kanaemi_config::{DICTIONARY_DIR, NEURAL_DIR, NEURAL_MODEL_FILE};
    use kanaemi_core::Converter;

    use crate::Profile;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kanaemi-runtime-neural-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(NEURAL_DIR)).unwrap();
        dir
    }

    fn surfaces(profile: &Profile, reading: &str) -> Vec<String> {
        profile
            .converter()
            .convert(reading, None)
            .into_iter()
            .map(|c| c.surface)
            .collect()
    }

    #[test]
    fn without_a_model_file_a_reading_no_dictionary_has_converts_to_nothing() {
        let profile = Profile::open(temp_dir("none"));
        assert_eq!(surfaces(&profile, "きかいがくしゅう"), Vec::<String>::new());
    }

    #[test]
    fn a_model_file_that_cannot_be_read_changes_nothing() {
        let dir = temp_dir("unreadable");
        fs::write(dir.join(NEURAL_DIR).join(NEURAL_MODEL_FILE), b"").unwrap();
        fs::create_dir_all(dir.join(DICTIONARY_DIR)).unwrap();
        fs::write(dir.join(DICTIONARY_DIR).join("a.tsv"), "きしゃ\t記者\n").unwrap();
        let profile = Profile::open(&dir);
        assert_eq!(surfaces(&profile, "きしゃ"), ["記者"]);
        assert_eq!(surfaces(&profile, "きかいがくしゅう"), Vec::<String>::new());
    }

    /// Runs only with the model file KANAEMI_NEURAL_MODEL names: the model
    /// is too large to keep with the tests.
    #[test]
    fn the_model_in_the_settings_folder_is_asked_and_outlives_a_reload() {
        let Some(model) = std::env::var_os("KANAEMI_NEURAL_MODEL").map(PathBuf::from) else {
            eprintln!("skipped: KANAEMI_NEURAL_MODEL is not set");
            return;
        };
        let dir = temp_dir("model");
        let neural = dir.join(NEURAL_DIR);
        std::os::unix::fs::symlink(&model, neural.join(NEURAL_MODEL_FILE)).unwrap();
        let tokenizer = model.with_file_name("tokenizer.json");
        std::os::unix::fs::symlink(tokenizer, neural.join("tokenizer.json")).unwrap();
        let mut profile = Profile::open(&dir);
        let started = Instant::now();
        while surfaces(&profile, "きかいがくしゅう").is_empty() {
            assert!(started.elapsed() < Duration::from_secs(60), "still loading");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(surfaces(&profile, "きかいがくしゅう")[0], "機械学習");
        fs::create_dir_all(dir.join(DICTIONARY_DIR)).unwrap();
        fs::write(dir.join(DICTIONARY_DIR).join("a.tsv"), "きしゃ\t記者\n").unwrap();
        profile.reload_if_changed();
        assert_eq!(surfaces(&profile, "きしゃ"), ["記者"]);
        assert_eq!(
            surfaces(&profile, "きかいがくしゅう")[0],
            "機械学習",
            "at once: the model is not read again"
        );
    }
}
