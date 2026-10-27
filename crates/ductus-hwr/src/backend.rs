use base64::Engine;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub trait Recognizer: Send {
    fn recognize(&self, png: &[u8], lang: &str) -> Result<String, String>;
}

fn prompt(lang: &str) -> String {
    let language = match lang {
        "de" => "The text is German.",
        "en" => "The text is English.",
        _ => "The text may be German or English.",
    };
    format!(
        "This image shows one line of handwritten notes. {language} Transcribe it exactly, \
         keeping the original spelling. Write mathematics as LaTeX between $ signs. \
         Reply with the transcription only, without quotes or comments."
    )
}

// Models like to wrap their answer in quotes or code fences.
fn clean(reply: &str) -> String {
    let t = reply.trim().trim_matches('`').trim();
    let t = t
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(t);
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub struct Server {
    pub url: String,
    pub model: String,
    pub api_key: String,
}

impl Recognizer for Server {
    fn recognize(&self, png: &[u8], lang: &str) -> Result<String, String> {
        let image = base64::engine::general_purpose::STANDARD.encode(png);
        let body = serde_json::json!({
            "model": self.model,
            "temperature": 0,
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "text", "text": prompt(lang) },
                    { "type": "image_url",
                      "image_url": { "url": format!("data:image/png;base64,{image}") } },
                ],
            }],
        });
        let base = self.url.trim_end_matches('/');
        let url = match base.ends_with("/v1") {
            true => format!("{base}/chat/completions"),
            false => format!("{base}/v1/chat/completions"),
        };
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(180)))
            .build()
            .into();
        let mut request = agent.post(&url);
        if !self.api_key.is_empty() {
            request = request.header("Authorization", &format!("Bearer {}", self.api_key));
        }
        let reply: serde_json::Value = request
            .send_json(&body)
            .map_err(|e| format!("{}: {e}", self.url))?
            .body_mut()
            .read_json()
            .map_err(|e| format!("The server answered something unexpected: {e}"))?;
        reply["choices"][0]["message"]["content"]
            .as_str()
            .map(clean)
            .ok_or_else(|| {
                let error = reply["error"]["message"].as_str().unwrap_or("no answer");
                format!("The server: {error}")
            })
    }
}

/// A built-in model (see [`crate::builtin`]), loaded on first use.
pub struct Builtin {
    dir: std::path::PathBuf,
    id: String,
}

/// Unused this long, a model is unloaded: the text models take about 300 MB, the formula one 600 MB.
pub const IDLE_UNLOAD: Duration = Duration::from_secs(5 * 60);

struct Loaded {
    model: crate::builtin::Model,
    used: Instant,
}

type Slot = Arc<Mutex<Option<Loaded>>>;

/// One copy of each model, shared by background recognition and conversions.
static SLOTS: Mutex<Vec<(PathBuf, Slot)>> = Mutex::new(Vec::new());

fn slot(path: &Path) -> Slot {
    let mut slots = SLOTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, slot)) = slots.iter().find(|(p, _)| p == path) {
        return Arc::clone(slot);
    }
    let slot = Slot::default();
    slots.push((path.to_owned(), Arc::clone(&slot)));
    slot
}

/// Unloads models unused for `idle`; a model in use stays.
pub fn unload_idle(idle: Duration) {
    let slots: Vec<Slot> = SLOTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .map(|(_, slot)| Arc::clone(slot))
        .collect();
    let mut unloaded = false;
    for slot in slots {
        if let Ok(mut loaded) = slot.try_lock() {
            if loaded.as_ref().is_some_and(|l| l.used.elapsed() >= idle) {
                *loaded = None;
                unloaded = true;
            }
        }
    }
    if unloaded {
        ductus_core::release_freed_memory();
    }
}

fn start_unloading() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        let _ = std::thread::Builder::new()
            .name("hwr-unload".into())
            .spawn(|| {
                loop {
                    std::thread::sleep(Duration::from_secs(30));
                    unload_idle(IDLE_UNLOAD);
                }
            });
    });
}

impl Recognizer for Builtin {
    fn recognize(&self, png: &[u8], lang: &str) -> Result<String, String> {
        let image = tiny_skia::Pixmap::decode_png(png).map_err(|e| e.to_string())?;
        let path = self.dir.join(&self.id);
        let slot = slot(&path);
        let mut loaded = slot.lock().map_err(|_| "recognizer failed before")?;
        let loaded = match &mut *loaded {
            Some(loaded) => loaded,
            empty => {
                start_unloading();
                empty.insert(Loaded {
                    model: crate::builtin::Model::load(&path)?,
                    used: Instant::now(),
                })
            }
        };
        let text = loaded.model.recognize(&image, lang);
        loaded.used = Instant::now();
        text
    }
}

pub struct Fixed(pub String);

impl Recognizer for Fixed {
    fn recognize(&self, _png: &[u8], _lang: &str) -> Result<String, String> {
        Ok(self.0.clone())
    }
}

pub fn from_settings(
    kind: &str,
    url: &str,
    model: &str,
    api_key: &str,
) -> Option<Box<dyn Recognizer>> {
    let or = |value: &str, default: &str| match value {
        "" => default.to_owned(),
        v => v.to_owned(),
    };
    match kind {
        "server" => Some(Box::new(Server {
            url: or(url, "http://localhost:11434/v1"),
            model: or(model, "qwen2.5vl"),
            api_key: api_key.into(),
        })),
        "builtin" => Some(Box::new(Builtin {
            dir: url.into(),
            id: model.into(),
        })),
        k => k
            .strip_prefix("fixed:")
            .map(|text| Box::new(Fixed(text.into())) as Box<dyn Recognizer>),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_are_cleaned_up() {
        assert_eq!(
            clean("  \"Grenzwert einer Folge\"\n"),
            "Grenzwert einer Folge"
        );
        assert_eq!(clean("```\nx^2 + 1\n```"), "x^2 + 1");
        assert_eq!(clean("zwei\nZeilen"), "zwei Zeilen");
    }
}
