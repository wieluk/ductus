use crate::model::{Decoder, DecoderConfig, Encoder, EncoderConfig};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use std::collections::HashMap;
use std::path::Path;
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

const MAX_TOKENS: usize = 256;

/// A downloadable recognition model, pinned to one revision.
pub struct ModelInfo {
    pub id: &'static str,
    pub title: &'static str,
    repo: &'static str,
    revision: &'static str,
    files: &'static [(&'static str, u64, &'static str)],
}

impl ModelInfo {
    pub fn size(&self) -> u64 {
        self.files.iter().map(|f| f.1).sum()
    }
}

pub const MODELS: [ModelInfo; 3] = [
    ModelInfo {
        id: "text-en",
        title: "English handwriting",
        repo: "microsoft/trocr-base-handwritten",
        revision: "eaacaf452b06415df8f10bb6fad3a4c11e609406",
        files: &[
            (
                "config.json",
                4165,
                "4c779f24e063c437c3dafd5b2e6c9f59f2fa2bd1dbb4ae6a30153bbbbf19e647",
            ),
            (
                "generation_config.json",
                190,
                "41149cdcffec4d657f32dfcddd9b208037f01286c9e07945c724908c58ed0193",
            ),
            (
                "vocab.json",
                898822,
                "06b4d46c8e752d410213d9548eb27a54db70fda0319b6271fb8d59dead5e1cab",
            ),
            (
                "model.safetensors",
                1333384464,
                "25a40cddc7e6120140a3d5b9e3dd3878a92ada7b4f312953ab22edc19c2a5acc",
            ),
        ],
    },
    ModelInfo {
        id: "text-de",
        title: "German handwriting",
        repo: "fhswf/TrOCR_german_handwritten",
        revision: "f43d8831af99105e9dbb718fcfc8373c0010174d",
        files: &[
            (
                "config.json",
                4744,
                "9ed8f8e93a3c813adfc4bd0b386f874d436d9df013bd86e06da99c56d93e2864",
            ),
            (
                "generation_config.json",
                185,
                "c1f96dd2d12621cfb6fbd9e93bbc40798a4a15bedcf441fa083f388e1c253fb5",
            ),
            (
                "vocab.json",
                798293,
                "ed19656ea1707df69134c4af35c8ceda2cc9860bf2c3495026153a133670ab5e",
            ),
            (
                "model.safetensors",
                2232985336,
                "de633614afb8b25f625c7c9d8596e33cb6eb71a4854bd5e3d1acd767483f5bae",
            ),
        ],
    },
    ModelInfo {
        id: "math",
        title: "Formulas",
        repo: "OleehyO/TexTeller",
        revision: "7b96df06b9d81cdb129c3bef68b7250bc3e2b0ea",
        files: &[
            (
                "config.json",
                4504,
                "ae1df4b712a25df6cbf0441d0e5602cee139f3f371c1b3555274db7aed2c0135",
            ),
            (
                "generation_config.json",
                154,
                "dce3cbe591ed7fc9d9ea9c588697e9e3a05248446343cf1682df309c013d5ab7",
            ),
            (
                "vocab.json",
                146663,
                "6c5fdc7c688b7da8ca9f6abca73ff2c12c08e14386461daf9af1f0716b31e359",
            ),
            (
                "added_tokens.json",
                56003,
                "7e88a0bc018da0461d442e43efdb9c36d0c9ccee4ee77040435a6393c4dc0f0e",
            ),
            (
                "model.safetensors",
                1192464688,
                "6d535b29f94de294f6d82cb777285379480672cbe396bbd645840eda56b2c407",
            ),
        ],
    },
];

pub fn model_info(id: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.id == id)
}

pub fn installed(dir: &Path, id: &str) -> bool {
    dir.join(id).join("model.safetensors").is_file()
}

/// Downloads a model into `dir/<id>`, each file checked against its pinned digest.
pub fn install(
    dir: &Path,
    id: &str,
    cancel: &std::sync::atomic::AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let info = model_info(id).ok_or("unknown model")?;
    // A model only counts as installed once every file is in place.
    let partial = dir.join(format!("{id}.part"));
    std::fs::create_dir_all(&partial).map_err(err)?;
    let total = info.size();
    let mut done = 0;
    for (name, size, sha) in info.files {
        let dest = partial.join(name);
        if !dest.is_file() {
            let asset = ductus_update::Asset {
                name: name.to_string(),
                url: format!(
                    "https://huggingface.co/{}/resolve/{}/{name}",
                    info.repo, info.revision
                ),
                size: *size,
            };
            let digest = ductus_update::digest_from_hex(sha).ok_or("bad digest")?;
            ductus_update::download(&asset, &digest, &dest, cancel, |n, _| {
                progress(done + n, total)
            })?;
        }
        done += size;
    }
    let target = dir.join(id);
    let _ = std::fs::remove_dir_all(&target);
    std::fs::rename(&partial, &target).map_err(err)
}

pub fn remove(dir: &Path, id: &str) -> Result<(), String> {
    match std::fs::remove_dir_all(dir.join(id)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(err(e)),
        _ => Ok(()),
    }
}

/// A recognition model on disk: a Hugging Face VisionEncoderDecoder checkpoint.
pub struct Model {
    encoder: Encoder,
    decoder: Decoder,
    image_size: usize,
    channels: usize,
    vocab: Vec<String>,
    start: u32,
    end: u32,
}

#[derive(serde::Deserialize)]
struct Config {
    encoder: EncoderConfig,
    decoder: DecoderConfig,
}

#[derive(serde::Deserialize, Default)]
struct Generation {
    decoder_start_token_id: Option<u32>,
    bos_token_id: Option<u32>,
    eos_token_id: Option<u32>,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl Model {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let config: Config =
            serde_json::from_slice(&std::fs::read(dir.join("config.json")).map_err(err)?)
                .map_err(err)?;
        let generation: Generation = std::fs::read(dir.join("generation_config.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        // SAFETY: the file is only read, and nothing else writes it while it is mapped.
        #[allow(unsafe_code)]
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(
                &[dir.join("model.safetensors")],
                DType::F32,
                &Device::Cpu,
            )
        }
        .map_err(err)?;
        Ok(Self {
            encoder: Encoder::new(&config.encoder, vb.pp("encoder")).map_err(err)?,
            decoder: Decoder::new(&config.decoder, vb.clone()).map_err(err)?,
            image_size: config.encoder.image_size,
            channels: config.encoder.num_channels,
            vocab: vocabulary(dir)?,
            // As transformers' generate(): the start token, else the beginning-of-text one.
            start: generation
                .decoder_start_token_id
                .or(generation.bos_token_id)
                .unwrap_or(2),
            end: generation.eos_token_id.unwrap_or(2),
        })
    }

    /// Reads the ink in `image` (dark on white).
    pub fn recognize(&mut self, image: &Pixmap) -> Result<String, String> {
        let pixels = self.pixels(image)?;
        let encoded = self.encoder.forward(&pixels).map_err(err)?;
        self.decoder.reset();
        let mut ids = vec![self.start];
        for step in 0..MAX_TOKENS {
            let input = match step {
                0 => Tensor::new(ids.as_slice(), &Device::Cpu),
                _ => Tensor::new(&ids[ids.len() - 1..], &Device::Cpu),
            }
            .and_then(|t| t.unsqueeze(0))
            .map_err(err)?;
            let past = if step == 0 { 0 } else { ids.len() - 1 };
            let logits = self.decoder.forward(&input, past, &encoded).map_err(err)?;
            let last = logits
                .squeeze(0)
                .and_then(|l| l.get(l.dim(0)? - 1))
                .map_err(err)?;
            let next = last
                .argmax(0)
                .and_then(|t| t.to_scalar::<u32>())
                .map_err(err)?;
            if next == self.end {
                break;
            }
            ids.push(next);
        }
        Ok(self.text(&ids[1..]))
    }

    // TrOCR stretches the line over a square of RGB; TexTeller crops the white border and
    // scales grayscale so the longer side fills its square, top-left.
    fn pixels(&self, image: &Pixmap) -> Result<Tensor, String> {
        let size = self.image_size as u32;
        let (x0, y0, w, h) = match self.channels {
            1 => ink_bounds(image).ok_or("no ink")?,
            _ => (0, 0, image.width(), image.height()),
        };
        let (sx, sy, quality) = match self.channels {
            1 => {
                let s = ((size - 1) as f32 / w.min(h) as f32).min(size as f32 / w.max(h) as f32);
                (s, s, FilterQuality::Bicubic)
            }
            _ => (
                size as f32 / w as f32,
                size as f32 / h as f32,
                FilterQuality::Bilinear,
            ),
        };
        let mut canvas = Pixmap::new(size, size).ok_or("empty image")?;
        canvas.fill(tiny_skia::Color::WHITE);
        let paint = PixmapPaint {
            quality,
            ..Default::default()
        };
        let at = Transform::from_translate(-(x0 as f32), -(y0 as f32)).post_scale(sx, sy);
        canvas.draw_pixmap(0, 0, image.as_ref(), &paint, at, None);
        let (drawn_w, drawn_h) = (
            (w as f32 * sx).round() as usize,
            (h as f32 * sy).round() as usize,
        );
        let side = size as usize;
        let gray: Vec<f32> = canvas
            .pixels()
            .iter()
            .map(|p| {
                (p.red() as f32 * 0.299 + p.green() as f32 * 0.587 + p.blue() as f32 * 0.114)
                    / 255.0
            })
            .collect();
        let data: Vec<f32> = match self.channels {
            // Padding stays 0, which is close to white after TexTeller's normalization.
            1 => (0..side * side)
                .map(|i| match (i % side < drawn_w, i / side < drawn_h) {
                    (true, true) => (gray[i] - 0.954_546_7) / 0.153_944_45,
                    _ => 0.0,
                })
                .collect(),
            _ => (0..3)
                .flat_map(|_| gray.iter().map(|g| (g - 0.5) / 0.5))
                .collect(),
        };
        Tensor::from_vec(data, (1, self.channels, side, side), &Device::Cpu).map_err(err)
    }

    fn text(&self, ids: &[u32]) -> String {
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

// The box around pixels clearly darker than white.
fn ink_bounds(image: &Pixmap) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = (image.width(), image.height());
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (i, p) in image.pixels().iter().enumerate() {
        if p.alpha() > 0 && (p.red() as u32 + p.green() as u32 + p.blue() as u32) < 3 * 240 {
            let (x, y) = (i as u32 % w, i as u32 / w);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    (x1 >= x0 && y1 >= y0).then(|| (x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}

// vocab.json (and added_tokens.json) as id → token.
fn vocabulary(dir: &Path) -> Result<Vec<String>, String> {
    let mut map: HashMap<String, u32> =
        serde_json::from_slice(&std::fs::read(dir.join("vocab.json")).map_err(err)?)
            .map_err(err)?;
    if let Ok(added) = std::fs::read(dir.join("added_tokens.json")) {
        let added: HashMap<String, u32> = serde_json::from_slice(&added).map_err(err)?;
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
