use anyhow::{anyhow, Result};
use tokenizers::Tokenizer;

const CLS_TOKEN: &str = "<|startoftext|>";
const SEP_TOKEN: &str = "<|return|>";
const PAD_TOKEN: &str = "<|endoftext|>";

#[derive(Debug, Clone, Copy)]
pub(crate) struct SpecialTokens {
    cls: i64,
    sep: i64,
    pub(crate) pad: i64,
}

impl SpecialTokens {
    pub(crate) fn from_tokenizer(tokenizer: &Tokenizer) -> Result<Self> {
        let cls = required_token_id(tokenizer, CLS_TOKEN)?;
        let sep = required_token_id(tokenizer, SEP_TOKEN)?;
        let pad = required_token_id(tokenizer, PAD_TOKEN)?;
        Ok(Self { cls, sep, pad })
    }

    pub(crate) fn validate_sequence(self, ids: &[i64]) -> Result<()> {
        if ids.first() != Some(&self.cls) {
            return Err(anyhow!(
                "Granite tokenizer did not add the required CLS token"
            ));
        }
        if ids.last() != Some(&self.sep) {
            return Err(anyhow!(
                "Granite tokenizer did not add the required separator token"
            ));
        }
        Ok(())
    }
}

fn required_token_id(tokenizer: &Tokenizer, token: &str) -> Result<i64> {
    tokenizer
        .token_to_id(token)
        .map(i64::from)
        .ok_or_else(|| anyhow!("Granite tokenizer is missing required token `{token}`"))
}

#[cfg(test)]
mod tests {
    use super::{SpecialTokens, CLS_TOKEN};
    use tokenizers::{models::bpe::BPE, Tokenizer};

    #[test]
    fn rejects_tokenizer_without_granite_special_tokens() {
        let tokenizer = Tokenizer::new(BPE::default());
        let error = SpecialTokens::from_tokenizer(&tokenizer).unwrap_err();
        assert!(error.to_string().contains(CLS_TOKEN));
    }

    #[test]
    fn rejects_sequences_without_cls_or_separator_boundaries() {
        let tokens = SpecialTokens {
            cls: 10,
            sep: 20,
            pad: 0,
        };
        assert!(tokens.validate_sequence(&[11, 20]).is_err());
        assert!(tokens.validate_sequence(&[10, 21]).is_err());
        assert!(tokens.validate_sequence(&[10, 20]).is_ok());
    }
}
