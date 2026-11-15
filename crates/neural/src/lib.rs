//! Kana-kanji conversion by a neural model that runs in this process, for
//! readings no dictionary has. Only macOS has it; elsewhere the crate is
//! empty, so no other system builds the model's runtime.
#![cfg(target_os = "macos")]

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use kanaemi_engine::Fallback;
use karukan_engine::kanji::{KanaKanjiConverter, ModelSource};

/// What candidates from the model carry as their source.
const NAME: &str = "neural";

/// How long one conversion may take: a key waits for it.
const DEADLINE: Duration = Duration::from_millis(50);

/// The model: being read, read, or not readable.
enum Model {
    Loading,
    Ready(KanaKanjiConverter),
    Failed,
}

/// Whether the model is read yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Loading,
    Ready,
    Failed,
}

/// A converter by a neural model, read from a file while the input method
/// goes on. Until it is read, and when it cannot be, it converts to nothing.
#[derive(Clone)]
pub struct NeuralConverter {
    model: Arc<Mutex<Model>>,
}

impl NeuralConverter {
    /// Starts reading the model file `model`, whose `tokenizer.json` is
    /// beside it, and returns at once. Nothing is fetched from elsewhere.
    pub fn open(model: &Path) -> Self {
        let shared = Arc::new(Mutex::new(Model::Loading));
        let source = ModelSource::Path(model.to_owned());
        let loading = shared.clone();
        let load = move || {
            let started = Instant::now();
            let loaded = match KanaKanjiConverter::from_source(&source, NAME) {
                Ok(converter) => {
                    tracing::info!(elapsed = ?started.elapsed(), "neural model read");
                    Model::Ready(converter)
                }
                Err(error) => {
                    tracing::warn!(%error, "neural model not read");
                    Model::Failed
                }
            };
            *lock(&loading) = loaded;
        };
        if let Err(error) = thread::Builder::new()
            .name("kanaemi-neural".to_owned())
            .spawn(load)
        {
            tracing::warn!(%error, "neural model not read");
            *lock(&shared) = Model::Failed;
        }
        Self { model: shared }
    }

    /// Whether the model is read yet.
    pub fn status(&self) -> Status {
        match *lock(&self.model) {
            Model::Loading => Status::Loading,
            Model::Ready(_) => Status::Ready,
            Model::Failed => Status::Failed,
        }
    }
}

impl Fallback for NeuralConverter {
    fn name(&self) -> &str {
        NAME
    }

    fn convert(&self, reading: &str, context: &str, limit: usize) -> Vec<String> {
        let Model::Ready(converter) = &*lock(&self.model) else {
            return Vec::new();
        };
        let started = Instant::now();
        match converter.convert_within(reading, context, limit, Some(started + DEADLINE)) {
            Ok(mut surfaces) => {
                // The model's runtime gives the reading back when it has
                // nothing.
                surfaces.retain(|surface| surface != reading);
                surfaces.truncate(limit);
                tracing::info!(
                    elapsed = ?started.elapsed(),
                    candidates = surfaces.len(),
                    "converted by the neural model"
                );
                surfaces
            }
            Err(error) => {
                tracing::warn!(elapsed = ?started.elapsed(), %error, "not converted by the neural model");
                Vec::new()
            }
        }
    }
}

/// A thread that panicked while holding the lock left the model as it was.
fn lock(model: &Mutex<Model>) -> MutexGuard<'_, Model> {
    model.lock().unwrap_or_else(PoisonError::into_inner)
}
