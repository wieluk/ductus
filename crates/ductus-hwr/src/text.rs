//! Handwritten lines to text: two small CTC models, DE·HTR (German) and PP-OCRv6 (50 languages).
//!
//! Each reads a whole line in one pass. With the language unknown both read it and the more confident
//! reading wins; measured on pen ink that keeps German at DE·HTR's error rate and English near
//! PP-OCRv6's, see docs/HANDWRITING.md.

use crate::image::{gray_scaled, ink_bounds};
use ort::session::Session;
use std::path::Path;
use tiny_skia::Pixmap;

/// Pixels darker than this count as ink when a line is cut to its strokes.
const INK: u32 = 160;
/// White kept around the ink, times its height. Ductus pads lines by a quarter of their height,
/// which leaves letters too small for these models.
const MARGIN: f32 = 0.05;

/// A line recognizer whose output is one class per column step, decoded greedily.
pub struct Ctc {
    session: Session,
    height: u32,
    channels: usize,
    mean: f32,
    std: f32,
    min_width: u32,
    max_width: u32,
    multiple: u32,
    pad: f32,
    /// Index 0 is the blank.
    chars: Vec<String>,
    /// Right-to-left scripts come out in visual order.
    reverse: bool,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(serde::Deserialize)]
struct DehtrConfig {
    alphabet: String,
    image_height: u32,
    max_width: u32,
    output_stride: u32,
}

#[derive(serde::Deserialize)]
struct PaddleConfig {
    #[serde(rename = "PreProcess")]
    pre: PaddlePre,
    #[serde(rename = "PostProcess")]
    post: PaddlePost,
}

#[derive(serde::Deserialize)]
struct PaddlePre {
    transform_ops: Vec<serde_yaml::Value>,
}

#[derive(serde::Deserialize)]
struct PaddlePost {
    character_dict: Vec<serde_yaml::Value>,
}

impl Ctc {
    /// DE·HTR: gray in 0..1 at 64 px, white padding to a multiple of its stride (its config.json).
    pub fn dehtr(model: &Path, config: &Path) -> Result<Self, String> {
        let c: DehtrConfig =
            serde_json::from_slice(&std::fs::read(config).map_err(err)?).map_err(err)?;
        Ok(Self {
            session: crate::runtime::session(model)?,
            height: c.image_height,
            channels: 1,
            mean: 0.0,
            std: 1.0,
            min_width: c.output_stride,
            max_width: c.max_width,
            multiple: c.output_stride,
            pad: 1.0,
            chars: std::iter::once(String::new())
                .chain(c.alphabet.chars().map(String::from))
                .collect(),
            reverse: false,
        })
    }

    /// PaddleOCR's recognizers: RGB scaled to -1..1, at least as wide as the training shape,
    /// zero padding; their dictionary plus a space follows the blank (their inference.yml).
    /// `reverse` for Arabic script, which they read in visual order.
    pub fn paddle(model: &Path, config: &Path, reverse: bool) -> Result<Self, String> {
        let c: PaddleConfig =
            serde_yaml::from_slice(&std::fs::read(config).map_err(err)?).map_err(err)?;
        let shape: Vec<u32> = c
            .pre
            .transform_ops
            .iter()
            .find_map(|op| {
                op.get("RecResizeImg")?
                    .get("image_shape")?
                    .as_sequence()
                    .cloned()
            })
            .ok_or("no image shape in the recognizer's configuration")?
            .iter()
            .filter_map(|v| v.as_u64().map(|v| v as u32))
            .collect();
        let [_, height, width] = shape[..] else {
            return Err("unexpected image shape".into());
        };
        // Plain YAML scalars: digits and `true` arrive as numbers and booleans.
        let chars = c.post.character_dict.iter().map(|v| match v {
            serde_yaml::Value::String(s) => s.clone(),
            other => serde_yaml::to_string(other)
                .unwrap_or_default()
                .trim()
                .to_owned(),
        });
        Ok(Self {
            session: crate::runtime::session(model)?,
            height,
            channels: 3,
            mean: 0.5,
            std: 0.5,
            min_width: width,
            max_width: 3200,
            multiple: 1,
            pad: 0.0,
            chars: std::iter::once(String::new())
                .chain(chars)
                .chain([" ".to_owned()])
                .collect(),
            reverse,
        })
    }

    /// The text and its confidence: the geometric mean of the probabilities of its characters.
    pub fn read(&mut self, image: &Pixmap) -> Result<(String, f64), String> {
        let (x0, y0, w, h) = ink_bounds(image, INK).ok_or("no ink")?;
        let margin = (h as f32 * MARGIN).round() as u32;
        let (x0, y0) = (x0.saturating_sub(margin), y0.saturating_sub(margin));
        let w = (w + 2 * margin).min(image.width() - x0);
        let h = (h + 2 * margin).min(image.height() - y0);
        let width =
            ((w as f32 * self.height as f32 / h as f32).ceil() as u32).clamp(1, self.max_width);
        let gray = gray_scaled(image, (x0, y0, w, h), width, self.height)?;
        let full = width.max(self.min_width).div_ceil(self.multiple) * self.multiple;
        let (rows, cols) = (self.height as usize, full as usize);
        let mut data = vec![self.pad; self.channels * rows * cols];
        for plane in data.chunks_mut(rows * cols) {
            for y in 0..rows {
                for x in 0..width as usize {
                    plane[y * cols + x] = (gray[y * width as usize + x] - self.mean) / self.std;
                }
            }
        }
        let input =
            ndarray::Array4::from_shape_vec((1, self.channels, rows, cols), data).map_err(err)?;
        let input = ort::value::Tensor::from_array(input).map_err(err)?;
        let outputs = self.session.run(ort::inputs![input]).map_err(err)?;
        let scores = outputs[0].try_extract_array::<f32>().map_err(err)?;
        let (steps, classes) = match *scores.shape() {
            [1, t, c] | [t, 1, c] => (t, c),
            ref s => return Err(format!("unexpected output shape {s:?}")),
        };
        let flat: Vec<f32> = scores.iter().copied().collect();
        let (text, confidence) = decode(&flat, steps, classes, &self.chars);
        Ok(match self.reverse {
            true => (reading_order(&text), confidence),
            false => (text, confidence),
        })
    }
}

/// Greedy CTC: the best class per step, repeats merged, blanks dropped. The scores may be
/// probabilities (PaddleOCR) or logits (DE·HTR).
fn decode(scores: &[f32], steps: usize, classes: usize, chars: &[String]) -> (String, f64) {
    let first = &scores[..classes.min(scores.len())];
    let probabilities =
        first.iter().all(|v| *v >= 0.0) && (first.iter().sum::<f32>() - 1.0).abs() < 0.01;
    let mut text = String::new();
    let (mut log_sum, mut count) = (0.0f64, 0usize);
    let mut previous = usize::MAX;
    for row in scores.chunks(classes).take(steps) {
        let (best, top) = row
            .iter()
            .copied()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0, 0.0));
        if best != 0 {
            let p = match probabilities {
                true => top,
                false => 1.0 / row.iter().map(|v| (v - top).exp()).sum::<f32>(),
            };
            log_sum += f64::from(p.max(1e-9)).ln();
            count += 1;
            if best != previous {
                text.push_str(chars.get(best).map_or("", String::as_str));
            }
        }
        previous = best;
    }
    let confidence = match count {
        0 => 0.0,
        n => (log_sum / n as f64).exp(),
    };
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (text, confidence)
}

/// Visual order to reading order for right-to-left text, as PaddleOCR does: the characters turn
/// around, while runs of Latin letters, digits and their punctuation keep their own order.
fn reading_order(text: &str) -> String {
    let keeps = |c: char| c.is_ascii_alphanumeric() || " :*./%+-".contains(c);
    let mut parts: Vec<String> = Vec::new();
    let mut run = String::new();
    for c in text.chars() {
        if keeps(c) {
            run.push(c);
        } else {
            if !run.is_empty() {
                parts.push(std::mem::take(&mut run));
            }
            parts.push(c.to_string());
        }
    }
    if !run.is_empty() {
        parts.push(run);
    }
    parts.into_iter().rev().collect()
}

/// Both line recognizers: German goes to DE·HTR, "auto" to whichever is more confident,
/// every other language to PP-OCRv6.
pub struct TextModel {
    german: Ctc,
    latin: Ctc,
}

impl TextModel {
    pub fn load(dir: &Path) -> Result<Self, String> {
        Ok(Self {
            german: Ctc::dehtr(&dir.join("dehtr.onnx"), &dir.join("dehtr.json"))?,
            latin: Ctc::paddle(&dir.join("ppocr.onnx"), &dir.join("ppocr.yml"), false)?,
        })
    }

    pub fn recognize(&mut self, image: &Pixmap, lang: &str) -> Result<String, String> {
        Ok(match lang {
            "de" => self.german.read(image)?.0,
            "auto" | "" => {
                let german = self.german.read(image)?;
                let latin = self.latin.read(image)?;
                match latin.1 > german.1 {
                    true => latin.0,
                    false => german.0,
                }
            }
            _ => self.latin.read(image)?.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars() -> Vec<String> {
        ["", "a", "b", " "].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn ctc_merges_repeats_and_drops_blanks() {
        // Steps: a a blank a b blank (probabilities over blank, a, b, space).
        let rows = [
            [0.1, 0.8, 0.05, 0.05],
            [0.1, 0.8, 0.05, 0.05],
            [0.9, 0.05, 0.03, 0.02],
            [0.1, 0.8, 0.05, 0.05],
            [0.1, 0.1, 0.7, 0.1],
            [0.9, 0.05, 0.03, 0.02],
        ];
        let flat: Vec<f32> = rows.iter().flatten().copied().collect();
        let (text, confidence) = decode(&flat, 6, 4, &chars());
        assert_eq!(text, "aab");
        let expected = (0.8f64.ln() * 3.0 + 0.7f64.ln()) / 4.0;
        assert!((confidence - expected.exp()).abs() < 1e-6);
    }

    #[test]
    fn logits_become_probabilities() {
        // Equal logits for a and b: just under half each, whichever wins.
        let flat = [0.0, 5.0, 5.0, -10.0];
        let (_, confidence) = decode(&flat, 1, 4, &chars());
        let expected = 1.0 / (2.0 + (-5.0f64).exp() + (-15.0f64).exp());
        assert!((confidence - expected).abs() < 1e-6);
    }

    #[test]
    fn arabic_turns_around_but_numbers_do_not() {
        // Visual order, left to right: "123", a space, then ب ا.
        assert_eq!(reading_order("123 با"), "اب123 ");
        assert_eq!(reading_order("abc"), "abc");
    }

    #[test]
    fn spaces_are_collapsed() {
        let rows = [
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let flat: Vec<f32> = rows.iter().flatten().copied().collect();
        assert_eq!(decode(&flat, 4, 4, &chars()).0, "a");
    }
}
