//! Handwritten formulas to LaTeX: TexTeller 3, a ViT encoder with a TrOCR decoder, as ONNX.
//!
//! The decoder is Optimum's merged export: the first step runs without a cache, every later one
//! feeds only the newest token and the keys and values the previous step returned.

use crate::image::{gray_scaled, ink_bounds};
use ort::session::{Session, SessionInputValue};
use ort::value::{DynValue, Tensor};
use std::collections::HashMap;
use std::path::Path;
use tiny_skia::Pixmap;

const MAX_TOKENS: usize = 256;
/// Formula images are cut to whatever is not near-white.
const INK: u32 = 240;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(serde::Deserialize)]
struct Config {
    decoder: DecoderConfig,
}

#[derive(serde::Deserialize)]
struct DecoderConfig {
    decoder_layers: usize,
    decoder_attention_heads: usize,
    d_model: usize,
}

#[derive(serde::Deserialize)]
struct Preprocessor {
    size: Size,
    image_mean: Vec<f32>,
    image_std: Vec<f32>,
}

#[derive(serde::Deserialize)]
struct Size {
    height: usize,
}

#[derive(serde::Deserialize)]
struct Generation {
    bos_token_id: i64,
    eos_token_id: i64,
}

pub struct FormulaModel {
    encoder: Session,
    decoder: Session,
    vocab: Vec<String>,
    size: usize,
    mean: f32,
    std: f32,
    start: i64,
    end: i64,
    layers: usize,
    heads: usize,
    head_dim: usize,
}

fn json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    serde_json::from_slice(&std::fs::read(path).map_err(err)?).map_err(err)
}

impl FormulaModel {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let config: Config = json(&dir.join("config.json"))?;
        let pre: Preprocessor = json(&dir.join("preprocessor_config.json"))?;
        let generation: Generation = json(&dir.join("generation_config.json"))?;
        let d = config.decoder;
        Ok(Self {
            encoder: crate::runtime::session(&dir.join("encoder.onnx"))?,
            decoder: crate::runtime::session(&dir.join("decoder.onnx"))?,
            vocab: vocabulary(dir)?,
            size: pre.size.height,
            mean: pre.image_mean.first().copied().unwrap_or(0.5),
            std: pre.image_std.first().copied().unwrap_or(0.5),
            start: generation.bos_token_id,
            end: generation.eos_token_id,
            layers: d.decoder_layers,
            heads: d.decoder_attention_heads,
            head_dim: d.d_model / d.decoder_attention_heads,
        })
    }

    pub fn recognize(&mut self, image: &Pixmap) -> Result<String, String> {
        let pixels = Tensor::from_array(self.pixels(image)?).map_err(err)?;
        let hidden: DynValue = {
            let mut out = self.encoder.run(ort::inputs![pixels]).map_err(err)?;
            let name = out.keys().next().ok_or("encoder gave nothing")?.to_owned();
            out.remove(&name).ok_or("encoder gave nothing")?
        };
        let mut past: Vec<(String, DynValue)> = Vec::new();
        for layer in 0..self.layers {
            for part in [
                "decoder.key",
                "decoder.value",
                "encoder.key",
                "encoder.value",
            ] {
                let empty = ndarray::Array4::<f32>::zeros((1, self.heads, 0, self.head_dim));
                let value = Tensor::from_array(empty).map_err(err)?.into_dyn();
                past.push((format!("past_key_values.{layer}.{part}"), value));
            }
        }
        let mut ids = Vec::new();
        let mut next = self.start;
        for step in 0..MAX_TOKENS {
            let mut inputs: Vec<(String, SessionInputValue)> = vec![
                (
                    "input_ids".into(),
                    Tensor::from_array(([1usize, 1], vec![next]))
                        .map_err(err)?
                        .into(),
                ),
                ("encoder_hidden_states".into(), (&hidden).into()),
                (
                    "use_cache_branch".into(),
                    Tensor::from_array(([1usize], vec![step > 0]))
                        .map_err(err)?
                        .into(),
                ),
            ];
            inputs.extend(
                past.iter()
                    .map(|(name, value)| (name.clone(), value.into())),
            );
            let mut out = self.decoder.run(inputs).map_err(err)?;
            let logits = out["logits"].try_extract_array::<f32>().map_err(err)?;
            next = logits
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map_or(self.end, |(i, _)| i as i64);
            if next == self.end {
                break;
            }
            ids.push(next);
            // The encoder's keys and values come from the first step; later steps return placeholders.
            for (name, value) in &mut past {
                if step > 0 && name.contains(".encoder.") {
                    continue;
                }
                let present = name.replacen("past_key_values", "present", 1);
                *value = out
                    .remove(&present)
                    .ok_or_else(|| format!("{present} missing"))?;
            }
        }
        Ok(latex(&self.text(&ids)))
    }

    // As TexTeller: the ink scaled so the longer side fills the square, top-left, the rest 0.
    fn pixels(&self, image: &Pixmap) -> Result<ndarray::Array4<f32>, String> {
        let s = self.size;
        let (x0, y0, w, h) = ink_bounds(image, INK).ok_or("no ink")?;
        let k = ((s - 1) as f32 / w.min(h) as f32).min(s as f32 / w.max(h) as f32);
        let dw = ((w as f32 * k).round() as usize).clamp(1, s);
        let dh = ((h as f32 * k).round() as usize).clamp(1, s);
        let gray = gray_scaled(image, (x0, y0, w, h), dw as u32, dh as u32)?;
        let mut plane = ndarray::Array4::<f32>::zeros((1, 1, s, s));
        for y in 0..dh {
            for x in 0..dw {
                plane[[0, 0, y, x]] = (gray[y * dw + x] - self.mean) / self.std;
            }
        }
        Ok(plane)
    }

    fn text(&self, ids: &[i64]) -> String {
        let bytes = byte_decoder();
        let mut out = Vec::new();
        for &id in ids {
            let Some(token) = self.vocab.get(id as usize) else {
                continue;
            };
            if token.starts_with('<') && token.ends_with('>') {
                continue;
            }
            out.extend(
                token
                    .chars()
                    .map(|c| bytes.get(&c).copied().unwrap_or(b'?')),
            );
        }
        String::from_utf8_lossy(&out).trim().to_owned()
    }
}

/// TexTeller 3 wraps its formulas as display math; Ductus wants the LaTeX alone.
fn latex(text: &str) -> String {
    let t = text.trim();
    let t = t
        .strip_prefix("\\[")
        .and_then(|t| t.strip_suffix("\\]"))
        .or_else(|| t.strip_prefix("$$").and_then(|t| t.strip_suffix("$$")))
        .unwrap_or(t);
    t.trim().to_owned()
}

// vocab.json (and added_tokens.json) as id → token.
fn vocabulary(dir: &Path) -> Result<Vec<String>, String> {
    let mut map: HashMap<String, u32> = json(&dir.join("vocab.json"))?;
    if dir.join("added_tokens.json").is_file() {
        let added: HashMap<String, u32> = json(&dir.join("added_tokens.json"))?;
        map.extend(added);
    }
    let mut vocab = vec![String::new(); map.values().max().map_or(0, |m| *m as usize + 1)];
    for (token, id) in map {
        vocab[id as usize] = token;
    }
    Ok(vocab)
}

// GPT-2's byte-level BPE maps every byte to a printable character.
fn byte_decoder() -> HashMap<char, u8> {
    let mut printable: Vec<u32> = (b'!' as u32..=b'~' as u32)
        .chain(0xA1..=0xAC)
        .chain(0xAE..=0xFF)
        .collect();
    let mut chars = printable.clone();
    let mut extra = 0;
    for b in 0..=255u32 {
        if !printable.contains(&b) {
            printable.push(b);
            chars.push(256 + extra);
            extra += 1;
        }
    }
    printable
        .into_iter()
        .zip(chars)
        .map(|(b, c)| (char::from_u32(c).unwrap(), b as u8))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_math_is_unwrapped() {
        assert_eq!(latex("\\[\\frac{a}{b}\\]"), "\\frac{a}{b}");
        assert_eq!(latex(" $$x^2$$ "), "x^2");
        assert_eq!(latex("x+1"), "x+1");
    }

    #[test]
    fn bytes_round_trip() {
        let decoder = byte_decoder();
        assert_eq!(decoder.len(), 256);
        assert_eq!(decoder[&'Ġ'], b' ');
    }
}
