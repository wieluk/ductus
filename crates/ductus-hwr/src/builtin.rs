//! The built-in recognizers: ONNX models downloaded on request, run by ONNX Runtime.
//! Why these models: docs/HANDWRITING.md.

use std::path::Path;
use tiny_skia::Pixmap;

/// One file of a model, pinned to a revision and digest.
pub struct ModelFile {
    repo: &'static str,
    revision: &'static str,
    path: &'static str,
    /// The name in the model's folder.
    name: &'static str,
    size: u64,
    sha256: &'static str,
}

/// A downloadable recognition model.
pub struct ModelInfo {
    pub id: &'static str,
    pub title: &'static str,
    files: &'static [ModelFile],
}

impl ModelInfo {
    pub fn size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
}

const DEHTR: &str = "naeyn/de-htr-web-v2";
const DEHTR_REV: &str = "6241c1ff0fd42725e0dc46fcd3217469cac851bb";
const PPOCR: &str = "PaddlePaddle/PP-OCRv6_medium_rec_onnx";
const PPOCR_REV: &str = "50c7eacafc52fa7bcf4194e8cd08e46f8558504b";
const TEXTELLER: &str = "onnx-community/TexTeller3-ONNX";
const TEXTELLER_REV: &str = "ed90c6164810242882ca90863bb85697a76f4841";

pub const MODELS: [ModelInfo; 2] = [
    ModelInfo {
        id: "text",
        title: "Handwriting (German, English)",
        files: &[
            ModelFile {
                repo: DEHTR,
                revision: DEHTR_REV,
                path: "model.onnx",
                name: "dehtr.onnx",
                size: 10396365,
                sha256: "b576a0a1281b9be46b2574028b75575e041b7d7cb650f063886e733467cc1499",
            },
            ModelFile {
                repo: DEHTR,
                revision: DEHTR_REV,
                path: "config.json",
                name: "dehtr.json",
                size: 1362,
                sha256: "4ebd69d9d27b398ea3997b031318accc13123f6a8950def1fedc6e364cb758fa",
            },
            ModelFile {
                repo: PPOCR,
                revision: PPOCR_REV,
                path: "inference.onnx",
                name: "ppocr.onnx",
                size: 76554979,
                sha256: "9c09abf0957f7968c7586464b7397b84ad2387a0497a351af40e9acc71b673ba",
            },
            ModelFile {
                repo: PPOCR,
                revision: PPOCR_REV,
                path: "inference.yml",
                name: "ppocr.yml",
                size: 150580,
                sha256: "991b700facf5b50a7de193468207d5f4255b538dde0d312ae3b7c7a9b6873129",
            },
        ],
    },
    ModelInfo {
        id: "math",
        title: "Formulas",
        files: &[
            ModelFile {
                repo: TEXTELLER,
                revision: TEXTELLER_REV,
                path: "onnx/encoder_model_int8.onnx",
                name: "encoder.onnx",
                size: 88188694,
                sha256: "4b478e6dbc05b83615e1760b625b20a42648c36ace766682a247dacba8ceab27",
            },
            ModelFile {
                repo: TEXTELLER,
                revision: TEXTELLER_REV,
                path: "onnx/decoder_model_merged_int8.onnx",
                name: "decoder.onnx",
                size: 229140257,
                sha256: "1e26574671fdbff033c1e213b23a4ad2f9afef24e1b227709fd6daf3ccd67aa1",
            },
            ModelFile {
                repo: TEXTELLER,
                revision: TEXTELLER_REV,
                path: "vocab.json",
                name: "vocab.json",
                size: 146663,
                sha256: "6c5fdc7c688b7da8ca9f6abca73ff2c12c08e14386461daf9af1f0716b31e359",
            },
            ModelFile {
                repo: TEXTELLER,
                revision: TEXTELLER_REV,
                path: "added_tokens.json",
                name: "added_tokens.json",
                size: 56003,
                sha256: "7e88a0bc018da0461d442e43efdb9c36d0c9ccee4ee77040435a6393c4dc0f0e",
            },
            ModelFile {
                repo: TEXTELLER,
                revision: TEXTELLER_REV,
                path: "config.json",
                name: "config.json",
                size: 4667,
                sha256: "d94417e62061a944f002c9d838ef5428df99fca2b8b54b089e30a1d5964cab65",
            },
            ModelFile {
                repo: TEXTELLER,
                revision: TEXTELLER_REV,
                path: "generation_config.json",
                name: "generation_config.json",
                size: 132,
                sha256: "6bf3790b65ff35b3f2e745c4ea71d236e3a44e23af5fb6ed3003a814871b22f1",
            },
            ModelFile {
                repo: TEXTELLER,
                revision: TEXTELLER_REV,
                path: "preprocessor_config.json",
                name: "preprocessor_config.json",
                size: 352,
                sha256: "9e30687f05b936e9f74915206a5425d0a19517910514bca7b348e619d7ec9349",
            },
        ],
    },
];

pub fn model_info(id: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.id == id)
}

pub fn installed(dir: &Path, id: &str) -> bool {
    model_info(id).is_some_and(|m| m.files.iter().all(|f| dir.join(id).join(f.name).is_file()))
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
    for file in info.files {
        let dest = partial.join(file.name);
        if !dest.is_file() {
            let asset = ductus_update::Asset {
                name: file.name.to_string(),
                url: format!(
                    "https://huggingface.co/{}/resolve/{}/{}",
                    file.repo, file.revision, file.path
                ),
                size: file.size,
            };
            let digest = ductus_update::digest_from_hex(file.sha256).ok_or("bad digest")?;
            ductus_update::download(&asset, &digest, &dest, cancel, |n, _| {
                progress(done + n, total)
            })?;
        }
        done += file.size;
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

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// A loaded built-in model.
pub enum Model {
    Text(crate::text::TextModel),
    Formula(crate::formula::FormulaModel),
}

impl Model {
    /// The model in `dir`, a folder named after its id.
    pub fn load(dir: &Path) -> Result<Self, String> {
        match dir.file_name().and_then(|n| n.to_str()) {
            Some("math") => Ok(Self::Formula(crate::formula::FormulaModel::load(dir)?)),
            _ => Ok(Self::Text(crate::text::TextModel::load(dir)?)),
        }
    }

    /// Reads the ink in `image` (dark on white); `lang` is "de", "en" or anything for both.
    pub fn recognize(&mut self, image: &Pixmap, lang: &str) -> Result<String, String> {
        match self {
            Self::Text(m) => m.recognize(image, lang),
            Self::Formula(m) => m.recognize(image),
        }
    }
}
