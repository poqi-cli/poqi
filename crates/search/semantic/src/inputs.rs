use anyhow::{anyhow, Context, Result};
use ndarray::Array2;
use ort::{
    session::SessionOutputs,
    value::{DynValue, Tensor},
};

#[derive(Debug, Clone)]
pub(crate) struct ModelEncoding {
    pub ids: Vec<i64>,
    pub attention_mask: Vec<i64>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ModelInputSpec {
    pub pad_token_id: i64,
    pub has_attention_mask: bool,
    pub has_token_type_ids: bool,
    pub has_position_ids: bool,
}

pub(crate) fn build_model_inputs(
    encodings: &[ModelEncoding],
    batch: usize,
    max_len: usize,
    spec: ModelInputSpec,
) -> Result<Vec<(&'static str, Tensor<i64>)>> {
    let mut input_ids = Array2::<i64>::from_elem((batch, max_len), spec.pad_token_id);
    let mut attention = Array2::<i64>::zeros((batch, max_len));

    for (row_idx, encoding) in encodings.iter().enumerate() {
        for (col_idx, token) in encoding.ids.iter().take(max_len).enumerate() {
            input_ids[(row_idx, col_idx)] = *token;
        }
        for (col_idx, mask) in encoding.attention_mask.iter().take(max_len).enumerate() {
            attention[(row_idx, col_idx)] = *mask;
        }
    }

    let shape = vec![
        to_i64(batch, "batch size")?,
        to_i64(max_len, "sequence length")?,
    ];
    let mut feeds: Vec<(&'static str, Tensor<i64>)> = Vec::with_capacity(4);
    feeds.push((
        "input_ids",
        Tensor::from_array((shape.clone(), input_ids.into_raw_vec()))
            .context("failed to build input_ids tensor")?,
    ));

    if spec.has_attention_mask {
        feeds.push((
            "attention_mask",
            Tensor::from_array((shape.clone(), attention.into_raw_vec()))
                .context("failed to build attention_mask tensor")?,
        ));
    }

    if spec.has_token_type_ids {
        let zeros = vec![0_i64; checked_mul(batch, max_len, "token_type_ids length")?];
        feeds.push((
            "token_type_ids",
            Tensor::from_array((shape.clone(), zeros))
                .context("failed to build token_type_ids tensor")?,
        ));
    }

    if spec.has_position_ids {
        let capacity = checked_mul(batch, max_len, "position ids length")?;
        let mut positions = Vec::with_capacity(capacity);
        for _ in 0..batch {
            for pos in 0..max_len {
                positions.push(to_i64(pos, "position index")?);
            }
        }
        feeds.push((
            "position_ids",
            Tensor::from_array((shape.clone(), positions))
                .context("failed to build position_ids tensor")?,
        ));
    }

    Ok(feeds)
}

pub(crate) fn select_embedding_value(
    outputs: SessionOutputs<'_>,
    preferred: Option<&str>,
) -> Result<DynValue> {
    let mut owned: Vec<(String, DynValue)> = outputs
        .into_iter()
        .map(|(name, value)| (name.to_string(), value))
        .collect();
    if owned.is_empty() {
        return Err(anyhow!("model produced no outputs"));
    }

    if let Some(name) = preferred {
        owned
            .into_iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
            .ok_or_else(|| anyhow!("model output `{name}` not found"))
    } else if owned.len() > 1 {
        let (_, value) = owned.remove(1);
        Ok(value)
    } else {
        let (_, value) = owned.remove(0);
        Ok(value)
    }
}

fn to_i64(value: usize, label: &str) -> Result<i64> {
    i64::try_from(value).map_err(|_| anyhow!("{label} exceeds i64::MAX (value={value})"))
}

fn checked_mul(lhs: usize, rhs: usize, label: &str) -> Result<usize> {
    lhs.checked_mul(rhs)
        .ok_or_else(|| anyhow!("{label} exceeds usize::MAX (lhs={lhs}, rhs={rhs})"))
}
