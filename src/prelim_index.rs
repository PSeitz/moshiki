use crate::{
    fingerprint::fingerprint,
    termmap::IndexingTermmap,
    tokenizer::{Token, TokenType, Tokenizer},
};
use fnv::FnvHashMap;

#[derive(Debug, Clone)]
pub struct TemplateTokenWithMeta {
    pub token: TemplateToken,
    /// This is the index in the token sequence
    pub token_index: u32,
}

#[derive(Debug, Clone)]
pub enum TemplateToken {
    Constant(ConstTemplateToken),
    Variable {
        is_id_like: bool,
        column_index: usize,
    },
    Whitespace(u32),
}

#[derive(Debug, Clone)]
pub struct ConstTemplateToken {
    pub composite_token: CompositeToken,
    pub text: String,
}
impl ConstTemplateToken {
    pub fn new(token: CompositeToken, text: &str) -> Self {
        ConstTemplateToken {
            composite_token: token,
            text: text.to_string(),
        }
    }
    pub fn term_id(&self) -> u32 {
        self.composite_token.term_id()
    }
}

impl TemplateToken {
    pub fn new_variable(column_index: usize) -> Self {
        TemplateToken::Variable {
            column_index,
            is_id_like: false,
        }
    }

    pub fn is_variable(&self) -> bool {
        match self {
            TemplateToken::Constant(_) => false,
            TemplateToken::Variable { .. } => true,
            TemplateToken::Whitespace(_) => false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Template {
    pub tokens: Vec<TemplateTokenWithMeta>,
}

pub struct PreliminaryIndex {
    pub term_hash_map: IndexingTermmap,
    pub preliminary_docs: FnvHashMap<u64, PrelimDocGroup>,
}

#[derive(Debug, Clone)]
pub struct PrelimDocGroup {
    pub template: Template,
    // TODO: No need for composite_tokens here, we know the type and can derive it from the
    // template
    pub columns: Vec<Vec<u32>>,
    pub num_docs: usize,
}

fn create_composite_token(
    token: &Token,
    line: &str,
    term_hash_map: &mut IndexingTermmap,
    is_unique: bool,
) -> CompositeToken {
    match token {
        Token::IPv4(v)
        | Token::Number(v)
        | Token::Uuid(v)
        | Token::Word(v)
        | Token::Punctuation(v) => {
            let term_slice = &line[v.start as usize..v.end as usize];
            let term_id = term_hash_map.mutate_or_create(term_slice, is_unique);
            (token.token_type(), term_id).into()
        }
        Token::Whitespace(num) => (token.token_type(), *num).into(),
    }
}

fn get_term_id(
    token: &Token,
    line: &str,
    term_hash_map: &mut IndexingTermmap,
    is_unique: bool,
) -> u32 {
    match token {
        Token::IPv4(v)
        | Token::Number(v)
        | Token::Uuid(v)
        | Token::Word(v)
        | Token::Punctuation(v) => {
            let term_slice = &line[v.start as usize..v.end as usize];
            term_hash_map.mutate_or_create(term_slice, is_unique)
        }
        Token::Whitespace(num) => *num,
    }
}

impl PrelimDocGroup {
    #[cold]
    pub fn new(tokens: &[Token], line: &str, term_hash_map: &mut IndexingTermmap) -> Self {
        let template_tokens = tokens
            .iter()
            .enumerate()
            .map(|(token_pos, token)| match token {
                Token::IPv4(_)
                | Token::Number(_)
                | Token::Uuid(_)
                | Token::Word(_)
                | Token::Punctuation(_) => {
                    let ct = create_composite_token(token, line, term_hash_map, false);
                    TemplateTokenWithMeta {
                        token: TemplateToken::Constant(ConstTemplateToken::new(
                            ct,
                            token.as_str(line).unwrap(),
                        )),
                        token_index: token_pos as u32,
                    }
                }
                Token::Whitespace(num) => TemplateTokenWithMeta {
                    token: TemplateToken::Whitespace(*num),
                    token_index: token_pos as u32,
                },
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

    #[inline]
    fn push(&mut self, tokens: &[Token], line: &str, term_hash_map: &mut IndexingTermmap) {
        // Compare with template and update if necessary
        // TODO: fast path here to quickly hashcheck all the constants.
        for template_token in &mut self.template.tokens {
            match &mut template_token.token {
                TemplateToken::Constant(existing_ct) => {
                    let token = &tokens[template_token.token_index as usize];
                    let token_text = token.as_str(line).unwrap();
                    if existing_ct.text != token_text {
                        let ct = get_term_id(token, line, term_hash_map, false);
                        // This position is now variable
                        let column_index = self.columns.len();
                        let mut new_column =
                            vec![existing_ct.composite_token.term_id(); self.num_docs];
                        new_column.push(ct);
                        self.columns.push(new_column);
                        template_token.token = TemplateToken::new_variable(column_index);
                    }
                }
                TemplateToken::Variable {
                    column_index,
                    is_id_like,
                } => {
                    let token = &tokens[template_token.token_index as usize];
                    let term_id = get_term_id(token, line, term_hash_map, *is_id_like);
                    self.columns[*column_index].push(term_id);
                    if self.num_docs == 1000 {
                        // We can check if this column is ID-like == all term IDs are different
                        // is_id_like is currently set false, so we only set it to true if we find all unique
                        // IDs
                        let mut seen_ids = std::collections::HashSet::new();
                        for term_id in &self.columns[*column_index] {
                            if !seen_ids.insert(term_id) {
                                // Found a duplicate, so this column is not ID-like
                                *is_id_like = false;
                                break;
                            }
                        }
                        if seen_ids.len() == self.columns[*column_index].len() {
                            // All IDs are unique, so this column is ID-like
                            *is_id_like = true;
                        }
                    }
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
    let mut term_hash_map = IndexingTermmap::default();
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

//impl<'a> PrelimDoc<'a> {
//pub fn iter(self) -> impl Iterator<Item = CompositeToken> + 'a {
//self.group
//.template
//.tokens
//.iter()
//.map(move |template_token| match &template_token.token {
//TemplateToken::Constant(ct) => ct.composite_token,
//TemplateToken::Variable { column_index, .. } => {
//self.group.columns[*column_index][self.doc_index]
//}
//TemplateToken::Whitespace(num) => CompositeToken::new(TokenType::Whitespace, *num),
//})
//}

//pub fn without_whitespace(self) -> impl Iterator<Item = CompositeToken> + 'a {
//self.iter()
//.filter(|token| !token.token_type().is_whitespace())
//}
//}
