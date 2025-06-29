use crate::{
    fingerprint::fingerprint,
    tokenizer::{Token, TokenType, Tokenizer},
};
use fnv::FnvHashMap;
use stacker::ArenaHashMap;

#[derive(Debug, Clone, Copy)]
pub enum TemplateToken {
    Constant(CompositeToken),
    Variable {
        column_index: usize,
        is_id_like: bool,
    },
    Whitespace(u32),
}

impl TemplateToken {
    pub fn new_variable(column_index: usize) -> Self {
        TemplateToken::Variable {
            column_index,
            is_id_like: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Template {
    pub tokens: Vec<TemplateToken>,
}

pub struct PreliminaryIndex {
    pub term_hash_map: ArenaHashMap,
    pub preliminary_docs: FnvHashMap<u64, PrelimDocGroup>,
}

#[derive(Debug, Clone)]
pub struct PrelimDocGroup {
    pub template: Template,
    pub columns: Vec<Vec<CompositeToken>>,
    pub num_docs: usize,
}

fn create_composite_token(
    token: &Token,
    line: &str,
    term_hash_map: &mut ArenaHashMap,
) -> CompositeToken {
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
            (token.token_type(), term_id).into()
        }
        Token::Whitespace(num) => (token.token_type(), *num).into(),
    }
}

impl PrelimDocGroup {
    pub fn new(tokens: &[Token], line: &str, term_hash_map: &mut ArenaHashMap) -> Self {
        let template_tokens = tokens
            .iter()
            .map(|token| match token {
                Token::IPv4(_)
                | Token::Number(_)
                | Token::Uuid(_)
                | Token::Word(_)
                | Token::Punctuation(_) => {
                    let ct = create_composite_token(token, line, term_hash_map);
                    TemplateToken::Constant(ct)
                }
                Token::Whitespace(num) => TemplateToken::Whitespace(*num),
            })
            .collect();

        Self {
            template: Template {
                tokens: template_tokens,
            },
            columns: Vec::new(),
            num_docs: 0,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = PrelimDoc<'_>> + '_ {
        (0..self.num_docs).map(move |i| PrelimDoc {
            group: self,
            doc_index: i,
        })
    }

    fn push(&mut self, tokens: &[Token], line: &str, term_hash_map: &mut ArenaHashMap) {
        // Compare with template and update if necessary
        for (i, ct) in tokens.iter().enumerate() {
            let ct = create_composite_token(ct, line, term_hash_map);
            let template_token = &mut self.template.tokens[i];
            match template_token {
                TemplateToken::Constant(existing_ct) => {
                    if existing_ct.term_id() != ct.term_id() {
                        // This position is now variable
                        let column_index = self.columns.len();
                        let mut new_column = vec![*existing_ct; self.num_docs];
                        new_column.push(ct);
                        self.columns.push(new_column);
                        *template_token = TemplateToken::new_variable(column_index);
                    }
                }
                TemplateToken::Variable {
                    column_index,
                    is_id_like: _,
                } => {
                    self.columns[*column_index].push(ct);
                }
                TemplateToken::Whitespace(_) => {
                    // Whitespace is constant within a group
                }
            }
        }
        self.num_docs += 1;
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

    let mut tokens = Vec::new();
    for line in lines {
        let tokenizer = Tokenizer::new(&line);
        tokens.extend(tokenizer);
        let fingerprint = fingerprint(&tokens);

        let group = preliminary_docs
            .entry(fingerprint)
            .or_insert_with(|| PrelimDocGroup::new(&tokens, &line, &mut term_hash_map));
        group.push(&tokens, &line, &mut term_hash_map);
        tokens.clear();
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
            .template
            .tokens
            .iter()
            .map(move |template_token| match template_token {
                TemplateToken::Constant(ct) => *ct,
                TemplateToken::Variable { column_index, .. } => {
                    self.group.columns[*column_index][self.doc_index]
                }
                TemplateToken::Whitespace(num) => CompositeToken::new(TokenType::Whitespace, *num),
            })
    }

    pub fn without_whitespace(self) -> impl Iterator<Item = CompositeToken> + 'a {
        self.iter()
            .filter(|token| !token.token_type().is_whitespace())
    }

    pub fn token_at(self, column_index: usize) -> CompositeToken {
        self.group.columns[column_index][self.doc_index]
    }
}
