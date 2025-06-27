use fnv::FnvHashMap;
use stacker::ArenaHashMap;

use crate::{
    Token,
    fingerprint::fingerprint,
    tokenizer::{TokenType, Tokenizer},
};

pub struct PreliminaryIndex {
    pub term_hash_map: ArenaHashMap,
    pub preliminary_docs: FnvHashMap<u64, Vec<PrelimDoc>>,
}

// A 32-bit composite: top 4 bits store token type, lower 28 bits store term ID
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompositeToken(u32);

impl CompositeToken {
    /// Pack a TokenType (4 bits) and a 28-bit ID into one u32
    #[inline]
    pub fn new(token_type: TokenType, term_id: u32) -> Self {
        // Ensure id fits in 28 bits
        assert!(term_id <= 0x0FFF_FFFF, "term ID out of range");
        let tt = (token_type.0 as u32) & 0xF;
        CompositeToken((tt << 28) | term_id)
    }

    /// Extract the TokenType from the top 4 bits
    #[inline]
    pub fn token_type(&self) -> TokenType {
        let token_type = ((self.0 >> 28) & 0xF) as u8;
        token_type.into()
    }

    /// Extract the 28-bit term ID
    pub fn term_id(&self) -> u32 {
        self.0 & 0x0FFF_FFFF
    }
}
impl From<(TokenType, u32)> for CompositeToken {
    #[inline]
    fn from(value: (TokenType, u32)) -> Self {
        CompositeToken::new(value.0, value.1)
    }
}

pub fn preliminary_index(lines: impl Iterator<Item = String>) -> PreliminaryIndex {
    let mut term_hash_map = ArenaHashMap::with_capacity(4);
    let mut preliminary_docs = FnvHashMap::default();

    for line in lines {
        let mut token_type_with_term_ids: Vec<CompositeToken> = Vec::with_capacity(32);
        let tokenizer = Tokenizer::new(&line);
        for token in tokenizer {
            let next_id = term_hash_map.len() as u32;
            match token {
                Token::IPv4(v)
                | Token::Number(v)
                | Token::Uuid(v)
                | Token::Word(v)
                | Token::Punctuation(v) => {
                    let mut term_id = 0;
                    term_hash_map.mutate_or_create(v.as_bytes(), |opt| {
                        term_id = opt.unwrap_or(next_id);
                        term_id
                    });
                    token_type_with_term_ids.push((token.token_type(), term_id).into());
                }
                // Term id for whitespace is the number of whitespace characters
                Token::Whitespace(num) => {
                    token_type_with_term_ids.push((token.token_type(), num as u32).into());
                }
            }
        }

        // Check wihtout whitespace tokens
        // TODO: This happens very often with the current tokenizer
        //if token_type_with_term_ids
        //.iter()
        //.filter(|comp_token| !comp_token.token_type().is_whitespace())
        //.count()
        //> 32
        //{
        //// Print the log line
        //println!("Warning: line exceeds 32 tokens: \n{}", &line);
        //let tokens = Tokenizer::new(&line).collect::<Vec<_>>();
        //println!(
        //"{:?}",
        //tokens
        //.iter()
        //.filter(|token| !token.token_type().is_whitespace())
        //.collect::<Vec<_>>()
        //);
        //}

        let prelim_doc = PrelimDoc(token_type_with_term_ids.clone());
        let fingerprint = fingerprint(&prelim_doc);

        preliminary_docs
            .entry(fingerprint)
            .or_insert_with(Vec::new)
            .push(prelim_doc);
    }

    PreliminaryIndex {
        term_hash_map,
        preliminary_docs,
    }
}



#[derive(Debug, Clone)]
pub struct PrelimDoc(pub Vec<CompositeToken>);

impl PrelimDoc {
    pub fn without_whitespace(&self) -> impl Iterator<Item = &CompositeToken> {
        self.0
            .iter()
            .filter(|token| !token.token_type().is_whitespace())
    }
}
