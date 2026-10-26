// A ViT encoder with a TrOCR decoder (Hugging Face's VisionEncoderDecoderModel), as TrOCR
// and TexTeller use it. Adapted from candle-transformers' vit.rs and trocr.rs (MIT/Apache-2.0),
// which lack the decoder biases and `layernorm_embedding` these checkpoints have.

use candle_core::{D, DType, Module, Result, Tensor};
use candle_nn::{
    Activation, Conv2dConfig, Embedding, LayerNorm, VarBuilder, conv2d, embedding, layer_norm,
};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EncoderConfig {
    pub hidden_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub intermediate_size: usize,
    pub hidden_act: Activation,
    pub layer_norm_eps: f64,
    pub image_size: usize,
    pub patch_size: usize,
    pub num_channels: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DecoderConfig {
    pub vocab_size: usize,
    pub d_model: usize,
    pub cross_attention_hidden_size: usize,
    pub decoder_layers: usize,
    pub decoder_attention_heads: usize,
    pub decoder_ffn_dim: usize,
    pub activation_function: Activation,
    pub max_position_embeddings: usize,
    #[serde(default)]
    pub layernorm_embedding: bool,
    #[serde(default)]
    pub scale_embedding: bool,
}

// Weights are stored transposed: candle's CPU matmul is ~30x slower on a transposed operand.
struct Linear {
    weight: Tensor,
    bias: Option<Tensor>,
}

impl Module for Linear {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let ys = xs.broadcast_matmul(&self.weight)?;
        match &self.bias {
            Some(bias) => ys.broadcast_add(bias),
            None => Ok(ys),
        }
    }
}

// With a bias exactly when the checkpoint has one.
fn linear(input: usize, output: usize, vb: VarBuilder) -> Result<Linear> {
    Ok(Linear {
        weight: vb.get((output, input), "weight")?.t()?.contiguous()?,
        bias: match vb.contains_tensor("bias") {
            true => Some(vb.get(output, "bias")?),
            false => None,
        },
    })
}

fn heads(xs: &Tensor, heads: usize) -> Result<Tensor> {
    let (b, n, d) = xs.dims3()?;
    xs.reshape((b, n, heads, d / heads))?
        .transpose(1, 2)?
        .contiguous()
}

fn merge_heads(xs: &Tensor) -> Result<Tensor> {
    let (b, h, n, d) = xs.dims4()?;
    xs.transpose(1, 2)?.reshape((b, n, h * d))
}

struct EncoderLayer {
    query: Linear,
    key: Linear,
    value: Linear,
    output: Linear,
    intermediate: Linear,
    down: Linear,
    before: LayerNorm,
    after: LayerNorm,
    act: Activation,
    heads: usize,
}

impl EncoderLayer {
    fn new(cfg: &EncoderConfig, vb: VarBuilder) -> Result<Self> {
        let h = cfg.hidden_size;
        let attention = vb.pp("attention");
        Ok(Self {
            query: linear(h, h, attention.pp("attention.query"))?,
            key: linear(h, h, attention.pp("attention.key"))?,
            value: linear(h, h, attention.pp("attention.value"))?,
            output: linear(h, h, attention.pp("output.dense"))?,
            intermediate: linear(h, cfg.intermediate_size, vb.pp("intermediate.dense"))?,
            down: linear(cfg.intermediate_size, h, vb.pp("output.dense"))?,
            before: layer_norm(h, cfg.layer_norm_eps, vb.pp("layernorm_before"))?,
            after: layer_norm(h, cfg.layer_norm_eps, vb.pp("layernorm_after"))?,
            act: cfg.hidden_act,
            heads: cfg.num_attention_heads,
        })
    }

    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let normed = xs.apply(&self.before)?;
        let q = heads(&normed.apply(&self.query)?, self.heads)?;
        let k = heads(&normed.apply(&self.key)?, self.heads)?;
        let v = heads(&normed.apply(&self.value)?, self.heads)?;
        let scale = (q.dim(D::Minus1)? as f64).sqrt();
        let attn = candle_nn::ops::softmax_last_dim(&(q.matmul(&k.t()?.contiguous()?)? / scale)?)?;
        let xs = (merge_heads(&attn.matmul(&v)?)?.apply(&self.output)? + xs)?;
        let ys = xs
            .apply(&self.after)?
            .apply(&self.intermediate)?
            .apply(&self.act)?;
        ys.apply(&self.down)? + xs
    }
}

pub struct Encoder {
    patches: candle_nn::Conv2d,
    cls: Tensor,
    positions: Tensor,
    layers: Vec<EncoderLayer>,
    norm: LayerNorm,
}

impl Encoder {
    pub fn new(cfg: &EncoderConfig, vb: VarBuilder) -> Result<Self> {
        let e = vb.pp("embeddings");
        let n = (cfg.image_size / cfg.patch_size).pow(2);
        let conv = Conv2dConfig {
            stride: cfg.patch_size,
            ..Default::default()
        };
        let patches = conv2d(
            cfg.num_channels,
            cfg.hidden_size,
            cfg.patch_size,
            conv,
            e.pp("patch_embeddings.projection"),
        )?;
        let layers = (0..cfg.num_hidden_layers)
            .map(|i| EncoderLayer::new(cfg, vb.pp("encoder.layer").pp(i)))
            .collect::<Result<_>>()?;
        Ok(Self {
            patches,
            cls: e.get((1, 1, cfg.hidden_size), "cls_token")?,
            positions: e.get((1, n + 1, cfg.hidden_size), "position_embeddings")?,
            layers,
            norm: layer_norm(cfg.hidden_size, cfg.layer_norm_eps, vb.pp("layernorm"))?,
        })
    }

    pub fn forward(&self, pixels: &Tensor) -> Result<Tensor> {
        let xs = self
            .patches
            .forward(pixels)?
            .flatten_from(2)?
            .transpose(1, 2)?;
        let cls = self.cls.broadcast_as((xs.dim(0)?, 1, self.cls.dim(2)?))?;
        let mut xs = Tensor::cat(&[&cls, &xs], 1)?.broadcast_add(&self.positions)?;
        for layer in &self.layers {
            xs = layer.forward(&xs)?;
        }
        xs.apply(&self.norm)
    }
}

struct Attention {
    q: Linear,
    k: Linear,
    v: Linear,
    out: Linear,
    heads: usize,
    cache: Option<(Tensor, Tensor)>,
}

impl Attention {
    fn new(cfg: &DecoderConfig, kv_dim: usize, vb: VarBuilder) -> Result<Self> {
        let d = cfg.d_model;
        Ok(Self {
            q: linear(d, d, vb.pp("q_proj"))?,
            k: linear(kv_dim, d, vb.pp("k_proj"))?,
            v: linear(kv_dim, d, vb.pp("v_proj"))?,
            out: linear(d, d, vb.pp("out_proj"))?,
            heads: cfg.decoder_attention_heads,
            cache: None,
        })
    }

    // Self-attention appends to the cache; cross-attention caches the encoder's keys once.
    // Keys are kept transposed (see Linear).
    fn forward(
        &mut self,
        xs: &Tensor,
        encoder: Option<&Tensor>,
        mask: Option<&Tensor>,
    ) -> Result<Tensor> {
        let q = heads(&xs.apply(&self.q)?, self.heads)?;
        let keys = |src: &Tensor| heads(&src.apply(&self.k)?, self.heads)?.t()?.contiguous();
        let (kt, v) = match (encoder, &self.cache) {
            (Some(_), Some(cached)) => cached.clone(),
            (Some(enc), None) => {
                let kv = (keys(enc)?, heads(&enc.apply(&self.v)?, self.heads)?);
                self.cache = Some(kv.clone());
                kv
            }
            (None, cached) => {
                let mut kt = keys(xs)?;
                let mut v = heads(&xs.apply(&self.v)?, self.heads)?;
                if let Some((pk, pv)) = cached {
                    kt = Tensor::cat(&[pk, &kt], 3)?;
                    v = Tensor::cat(&[pv, &v], 2)?;
                }
                self.cache = Some((kt.clone(), v.clone()));
                (kt, v)
            }
        };
        let scale = (q.dim(D::Minus1)? as f64).sqrt();
        let mut scores = (q.matmul(&kt)? / scale)?;
        if let Some(mask) = mask {
            scores = scores.broadcast_add(mask)?;
        }
        merge_heads(&candle_nn::ops::softmax_last_dim(&scores)?.matmul(&v)?)?.apply(&self.out)
    }
}

struct DecoderLayer {
    self_attn: Attention,
    self_norm: LayerNorm,
    cross_attn: Attention,
    cross_norm: LayerNorm,
    fc1: Linear,
    fc2: Linear,
    final_norm: LayerNorm,
    act: Activation,
}

impl DecoderLayer {
    fn new(cfg: &DecoderConfig, vb: VarBuilder) -> Result<Self> {
        let d = cfg.d_model;
        Ok(Self {
            self_attn: Attention::new(cfg, d, vb.pp("self_attn"))?,
            self_norm: layer_norm(d, 1e-5, vb.pp("self_attn_layer_norm"))?,
            cross_attn: Attention::new(
                cfg,
                cfg.cross_attention_hidden_size,
                vb.pp("encoder_attn"),
            )?,
            cross_norm: layer_norm(d, 1e-5, vb.pp("encoder_attn_layer_norm"))?,
            fc1: linear(d, cfg.decoder_ffn_dim, vb.pp("fc1"))?,
            fc2: linear(cfg.decoder_ffn_dim, d, vb.pp("fc2"))?,
            final_norm: layer_norm(d, 1e-5, vb.pp("final_layer_norm"))?,
            act: cfg.activation_function,
        })
    }

    fn forward(&mut self, xs: &Tensor, encoder: &Tensor, mask: Option<&Tensor>) -> Result<Tensor> {
        let xs = (self.self_attn.forward(xs, None, mask)? + xs)?.apply(&self.self_norm)?;
        let xs =
            (self.cross_attn.forward(&xs, Some(encoder), None)? + xs)?.apply(&self.cross_norm)?;
        let ys = xs.apply(&self.fc1)?.apply(&self.act)?.apply(&self.fc2)?;
        (ys + xs)?.apply(&self.final_norm)
    }
}

pub struct Decoder {
    tokens: Embedding,
    output: Tensor,
    positions: Embedding,
    embed_norm: Option<LayerNorm>,
    embed_scale: f64,
    layers: Vec<DecoderLayer>,
}

impl Decoder {
    pub fn new(cfg: &DecoderConfig, vb: VarBuilder) -> Result<Self> {
        let vb = vb.pp("decoder.model.decoder");
        let layers = (0..cfg.decoder_layers)
            .map(|i| DecoderLayer::new(cfg, vb.pp("layers").pp(i)))
            .collect::<Result<_>>()?;
        let tokens = embedding(cfg.vocab_size, cfg.d_model, vb.pp("embed_tokens"))?;
        Ok(Self {
            // The output projection is tied to the token embeddings.
            output: tokens.embeddings().t()?.contiguous()?,
            tokens,
            // Learned positions start at offset 2, as in BART.
            positions: embedding(
                cfg.max_position_embeddings + 2,
                cfg.d_model,
                vb.pp("embed_positions"),
            )?,
            embed_norm: match cfg.layernorm_embedding {
                true => Some(layer_norm(cfg.d_model, 1e-5, vb.pp("layernorm_embedding"))?),
                false => None,
            },
            embed_scale: match cfg.scale_embedding {
                true => (cfg.d_model as f64).sqrt(),
                false => 1.0,
            },
            layers,
        })
    }

    pub fn reset(&mut self) {
        for layer in &mut self.layers {
            layer.self_attn.cache = None;
            layer.cross_attn.cache = None;
        }
    }

    /// Logits for `ids`, which follow `past` tokens already in the cache.
    pub fn forward(&mut self, ids: &Tensor, past: usize, encoder: &Tensor) -> Result<Tensor> {
        let n = ids.dim(1)?;
        let positions =
            Tensor::arange(past as u32 + 2, (past + n) as u32 + 2, ids.device())?.unsqueeze(0)?;
        let mut xs =
            ((ids.apply(&self.tokens)? * self.embed_scale)? + positions.apply(&self.positions)?)?;
        if let Some(norm) = &self.embed_norm {
            xs = xs.apply(norm)?;
        }
        let mask = match n {
            1 => None,
            _ => {
                let m: Vec<f32> = (0..n)
                    .flat_map(|i| (0..n).map(move |j| if j > i { f32::NEG_INFINITY } else { 0.0 }))
                    .collect();
                Some(Tensor::from_vec(m, (n, n), ids.device())?.to_dtype(DType::F32)?)
            }
        };
        for layer in &mut self.layers {
            xs = layer.forward(&xs, encoder, mask.as_ref())?;
        }
        xs.broadcast_matmul(&self.output)
    }
}
