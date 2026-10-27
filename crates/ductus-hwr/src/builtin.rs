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

/// PaddleOCR's recognizer for one script: its ONNX model and the dictionary in its inference.yml.
macro_rules! script {
    ($id:literal, $title:literal, $repo:literal, $rev:literal,
     $onnx:literal, $onnx_sha:literal, $yml:literal, $yml_sha:literal) => {
        ModelInfo {
            id: $id,
            title: $title,
            files: &[
                ModelFile {
                    repo: $repo,
                    revision: $rev,
                    path: "inference.onnx",
                    name: "ppocr.onnx",
                    size: $onnx,
                    sha256: $onnx_sha,
                },
                ModelFile {
                    repo: $repo,
                    revision: $rev,
                    path: "inference.yml",
                    name: "ppocr.yml",
                    size: $yml,
                    sha256: $yml_sha,
                },
            ],
        }
    };
}

pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "text",
        title: "Handwriting: Latin script, Chinese, Japanese",
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
    script!(
        "text-greek",
        "Handwriting: Greek",
        "PaddlePaddle/el_PP-OCRv5_mobile_rec_onnx",
        "8152b89d2ee0e1d4c92decab75fe75e5e3d836b6",
        7808735,
        "2acf17fcaea2bc81b878e311e6263b8885f48bb03796f75f9f30ed3242bbaa6d",
        3317,
        "17d85b2fe2d2f24cd4ab07bcbc33e0c126859b956ced36e281dc65e2d0c1f0bf"
    ),
    script!(
        "text-eslav",
        "Handwriting: Russian, Ukrainian, Belarusian",
        "PaddlePaddle/eslav_PP-OCRv5_mobile_rec_onnx",
        "9a32171fc5718746875e1a261818884517975013",
        7887627,
        "b3018ef2b09a0250b6e0c8e871c927098363e5fd4df890cc68e8358eb0aaf1bd",
        4538,
        "025039bac23eb4a308efcefa4d58eab3af440767815c6ba6938468bf6353ee5a"
    ),
    script!(
        "text-cyrillic",
        "Handwriting: Cyrillic",
        "PaddlePaddle/cyrillic_PP-OCRv5_mobile_rec_onnx",
        "2cef88145434beb8afa9dd82d77d799eb1ad7b29",
        8048799,
        "5371ee1ddaa7983cc62d0818d99e982b6804638c85e4f960d59a574094e172e5",
        6991,
        "5c76cc91fa98410178a09f498db10050d0ec1634a660053d3005ab7be581f501"
    ),
    script!(
        "text-arabic",
        "Handwriting: Arabic script",
        "PaddlePaddle/arabic_PP-OCRv5_mobile_rec_onnx",
        "14aaedcd75825982689ecf5cd64ab33ee083215a",
        7998947,
        "799113ebf267fbe742deb99eb36e8d42c9ddc5291ceacf92add41b4d52a59110",
        6165,
        "21368419e6c016c31db55d316d59e11c128e1913e6e6fe10287084710043d3a6"
    ),
    script!(
        "text-korean",
        "Handwriting: Korean",
        "PaddlePaddle/korean_PP-OCRv5_mobile_rec_onnx",
        "5c6f574b8e2230adf4287b33e736d71b9fabd28e",
        13418787,
        "92f0b7785e64fc9090106a241cf4c1eb97472824558272751b88a2a4476d3a08",
        96039,
        "f757fa1c40e99edcf27e9cce879b93eb2a51fa46f5ef39095689b8c37dd75998"
    ),
    script!(
        "text-devanagari",
        "Handwriting: Devanagari",
        "PaddlePaddle/devanagari_PP-OCRv5_mobile_rec_onnx",
        "251aec19e36739540d35e2cc943f6aa7503b98e5",
        7912311,
        "cb789212ce96c69d3e74728ae4309d179281d68cb3945d0616b67cafab41c986",
        5027,
        "9bd172dd26440c8ce94d1cde5d5baea6aefdc7cf3c5c8492e0beedef656d4e54"
    ),
    script!(
        "text-thai",
        "Handwriting: Thai",
        "PaddlePaddle/th_PP-OCRv5_mobile_rec_onnx",
        "1d4adbbafb1034a2fd6618498575b81ea7b69f69",
        7891015,
        "27618be66018f8598ac0a526a593f9f1cebf794e7eded93428e8fb016e537f5f",
        4667,
        "f6ba7fefc38ca1ff398ddafa75d67d16e0b3757c4e6c833adffee98a981766c9"
    ),
    script!(
        "text-tamil",
        "Handwriting: Tamil",
        "PaddlePaddle/ta_PP-OCRv5_mobile_rec_onnx",
        "0db89798af0d5218fad9ea7ace0557526e9077f1",
        7885691,
        "c6d2b682d2a0ea4cb1fccdba295976f93fd439964d16cdc666cadef531accbee",
        4579,
        "88a28f5a1bb30cabe38a0985cb5e6619fa4f0c7c78e57a08274674228c5219a6"
    ),
    script!(
        "text-telugu",
        "Handwriting: Telugu",
        "PaddlePaddle/te_PP-OCRv5_mobile_rec_onnx",
        "f1197c83430241c58c64b0952b3a80366d04eeba",
        7898759,
        "8238bfc46d4cffe720ed6706e3842802467343497428693ff2bfb4e6b3caa36b",
        4795,
        "acebbe53f1831bf28ddfed75aedf58225d7aa5d09100c1d5a9140a2a53b137ce"
    ),
];

/// A handwriting language: its code (`hwr/lang`), names and the model that reads it.
pub struct Language {
    pub code: &'static str,
    pub english: &'static str,
    /// The name in the language itself; empty where it is the English one.
    pub native: &'static str,
    pub model: &'static str,
}

const fn lang(
    code: &'static str,
    english: &'static str,
    native: &'static str,
    model: &'static str,
) -> Language {
    Language {
        code,
        english,
        native,
        model,
    }
}

/// "auto" reads German and English (and the other Latin-script languages) with both text readers.
pub const LANGUAGES: &[Language] = &[
    lang("auto", "German or English", "", "text"),
    lang("de", "German", "Deutsch", "text"),
    lang("en", "English", "", "text"),
    lang("ar", "Arabic", "العربية", "text-arabic"),
    lang("be", "Belarusian", "Беларуская", "text-eslav"),
    lang("bg", "Bulgarian", "Български", "text-cyrillic"),
    lang("ca", "Catalan", "Català", "text"),
    lang("zh", "Chinese", "中文", "text"),
    lang("hr", "Croatian", "Hrvatski", "text"),
    lang("cs", "Czech", "Čeština", "text"),
    lang("da", "Danish", "Dansk", "text"),
    lang("nl", "Dutch", "Nederlands", "text"),
    lang("et", "Estonian", "Eesti", "text"),
    lang("fi", "Finnish", "Suomi", "text"),
    lang("fr", "French", "Français", "text"),
    lang("el", "Greek", "Ελληνικά", "text-greek"),
    lang("hi", "Hindi", "हिन्दी", "text-devanagari"),
    lang("hu", "Hungarian", "Magyar", "text"),
    lang("is", "Icelandic", "Íslenska", "text"),
    lang("id", "Indonesian", "Bahasa Indonesia", "text"),
    lang("it", "Italian", "Italiano", "text"),
    lang("ja", "Japanese", "日本語", "text"),
    lang("kk", "Kazakh", "Қазақша", "text-cyrillic"),
    lang("ko", "Korean", "한국어", "text-korean"),
    lang("lv", "Latvian", "Latviešu", "text"),
    lang("lt", "Lithuanian", "Lietuvių", "text"),
    lang("mk", "Macedonian", "Македонски", "text-cyrillic"),
    lang("mr", "Marathi", "मराठी", "text-devanagari"),
    lang("mn", "Mongolian", "Монгол", "text-cyrillic"),
    lang("ne", "Nepali", "नेपाली", "text-devanagari"),
    lang("nb", "Norwegian", "Norsk", "text"),
    lang("fa", "Persian", "فارسی", "text-arabic"),
    lang("pl", "Polish", "Polski", "text"),
    lang("pt", "Portuguese", "Português", "text"),
    lang("ro", "Romanian", "Română", "text"),
    lang("ru", "Russian", "Русский", "text-eslav"),
    lang("sr", "Serbian", "Српски", "text-cyrillic"),
    lang("sk", "Slovak", "Slovenčina", "text"),
    lang("sl", "Slovenian", "Slovenščina", "text"),
    lang("es", "Spanish", "Español", "text"),
    lang("sv", "Swedish", "Svenska", "text"),
    lang("ta", "Tamil", "தமிழ்", "text-tamil"),
    lang("te", "Telugu", "తెలుగు", "text-telugu"),
    lang("th", "Thai", "ไทย", "text-thai"),
    lang("tr", "Turkish", "Türkçe", "text"),
    lang("uk", "Ukrainian", "Українська", "text-eslav"),
    lang("ur", "Urdu", "اردو", "text-arabic"),
];

pub fn language(code: &str) -> &'static Language {
    LANGUAGES
        .iter()
        .find(|l| l.code == code)
        .unwrap_or(&LANGUAGES[0])
}

/// The model that reads handwriting in `lang` (a code from [`LANGUAGES`]).
pub fn text_model(lang: &str) -> &'static str {
    language(lang).model
}

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
    /// One script's reader (the "text-…" models).
    Script(crate::text::Ctc),
    Formula(crate::formula::FormulaModel),
}

impl Model {
    /// The model in `dir`, a folder named after its id.
    pub fn load(dir: &Path) -> Result<Self, String> {
        match dir.file_name().and_then(|n| n.to_str()) {
            Some("math") => Ok(Self::Formula(crate::formula::FormulaModel::load(dir)?)),
            Some("text") => Ok(Self::Text(crate::text::TextModel::load(dir)?)),
            Some(id) => Ok(Self::Script(crate::text::Ctc::paddle(
                &dir.join("ppocr.onnx"),
                &dir.join("ppocr.yml"),
                id == "text-arabic",
            )?)),
            None => Err("no model".into()),
        }
    }

    /// Reads the ink in `image` (dark on white); `lang` is a code from [`LANGUAGES`].
    pub fn recognize(&mut self, image: &Pixmap, lang: &str) -> Result<String, String> {
        match self {
            Self::Text(m) => m.recognize(image, lang),
            Self::Script(m) => Ok(m.read(image)?.0),
            Self::Formula(m) => m.recognize(image),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_a_text_model() {
        for l in LANGUAGES {
            let model = model_info(l.model).unwrap_or_else(|| panic!("{} has no model", l.code));
            assert!(model.id.starts_with("text"));
        }
        assert_eq!(text_model("ru"), "text-eslav");
        assert_eq!(text_model("unknown"), "text");
        let codes: std::collections::HashSet<_> = LANGUAGES.iter().map(|l| l.code).collect();
        assert_eq!(codes.len(), LANGUAGES.len());
    }
}
