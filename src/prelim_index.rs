use fnv::FnvHashMap;
use stacker::ArenaHashMap;

use crate::{
    fingerprint::fingerprint,
    tokenizer::{Token, TokenType, Tokenizer},
};

pub struct PreliminaryIndex {
    pub term_hash_map: ArenaHashMap,
    pub preliminary_docs: FnvHashMap<u64, PrelimDocGroup>,
}

#[derive(Debug, Clone, Default)]
pub struct PrelimDocGroup {
    pub columns: Vec<Vec<CompositeToken>>,
    pub num_docs: usize,
}

impl PrelimDocGroup {
    pub fn iter(&self) -> impl Iterator<Item = PrelimDoc<'_>> + '_ {
        (0..self.num_docs).map(move |i| PrelimDoc {
            group: self,
            doc_index: i,
        })
    }

    pub fn push(&mut self, tokens: &[Token], line: &str, term_hash_map: &mut ArenaHashMap) {
        if self.columns.is_empty() {
            self.columns = vec![Vec::new(); tokens.len()];
        }

        for (i, token) in tokens.iter().enumerate() {
            let next_id = term_hash_map.len() as u32;
            match token {
                Token::IPv4(v)
                | Token::Number(v)
                | Token::Uuid(v)
                | Token::Word(v)
                | Token::Punctuation(v) => {
                    let mut term_id = 0;
                    let term_slice = &line[v.start as usize..v.end as usize];
                    term_hash_map.mutate_or_create(term_slice.as_bytes(), |opt| {
                        term_id = opt.unwrap_or(next_id);
                        term_id
                    });
                    self.columns[i].push((token.token_type(), term_id).into());
                }
                Token::Whitespace(num) => {
                    self.columns[i].push((token.token_type(), *num).into());
                }
            }
        }
        self.num_docs += 1;
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.num_docs == 0
    }

    pub(crate) fn num_tokens(&self) -> usize {
        self.columns.len()
    }
    pub(crate) fn num_docs(&self) -> usize {
        self.num_docs
    }
}

// A 32-bit composite: top 4 bits store token type, lower 28 bits store term ID
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompositeToken(u32);

impl std::fmt::Debug for CompositeToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Display both the token type and term ID
        write!(
            f,
            "CompositeToken(type: {:?}, term_id: {})",
            self.token_type(),
            self.term_id()
        )
    }
}

impl CompositeToken {
    /// Pack a TokenType (4 bits) and a 28-bit ID into one u32
    #[inline]
    pub fn new(token_type: TokenType, term_id: u32) -> Self {
        // Ensure id fits in 28 bits
        assert!(term_id <= 0x0FFF_FFFF, "term ID out of range");
        let tt = (token_type as u32) & 0xF;
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

    /// Get the raw u32 value
    #[inline]
    pub fn as_u32(&self) -> u32 {
        self.0
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
    let mut preliminary_docs: FnvHashMap<u64, PrelimDocGroup> = FnvHashMap::default();

    for line in lines {
        let tokenizer = Tokenizer::new(&line);
        let tokens = tokenizer.collect::<Vec<_>>();
        let fingerprint = fingerprint(&tokens);
        preliminary_docs
            .entry(fingerprint)
            .or_default()
            .push(&tokens, &line, &mut term_hash_map);
    }

    PreliminaryIndex {
        term_hash_map,
        preliminary_docs,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PrelimDoc<'a> {
    group: &'a PrelimDocGroup,
    doc_index: usize,
}

impl<'a> PrelimDoc<'a> {
    pub fn iter(self) -> impl Iterator<Item = CompositeToken> + 'a {
        self.group
            .columns
            .iter()
            .map(move |column| column[self.doc_index])
    }

    pub fn without_whitespace(self) -> impl Iterator<Item = CompositeToken> + 'a {
        self.iter()
            .filter(|token| !token.token_type().is_whitespace())
    }

    pub fn token_at(self, column_index: usize) -> CompositeToken {
        self.group.columns[column_index][self.doc_index]
    }
}
