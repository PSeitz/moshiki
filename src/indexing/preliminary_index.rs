use fxhash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::tokenizer::{tokens_as_string, Token, TokenType, Tokenizer};
use stacker::fastcmp::fast_short_slice_compare;

use super::{fingerprint, termmap::IndexingTermmap};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TemplateTokenWithMeta {
    pub token: IndexingTemplateToken,
    /// This is the index in the token sequence
    pub token_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IndexingTemplateToken {
    Constant(ConstTemplateToken),
    Variable {
        is_id_like: bool,
        column_index: usize,
    },
    Whitespace(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl IndexingTemplateToken {
    pub fn new_variable(column_index: usize) -> Self {
        IndexingTemplateToken::Variable {
            column_index,
            is_id_like: false,
        }
    }

    pub fn is_variable(&self) -> bool {
        match self {
            IndexingTemplateToken::Constant(_) => false,
            IndexingTemplateToken::Variable { .. } => true,
            IndexingTemplateToken::Whitespace(_) => false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct IndexingTemplate {
    pub tokens: Vec<TemplateTokenWithMeta>,
}

pub struct PreliminaryIndex {
    pub term_hash_map: IndexingTermmap,
    pub preliminary_docs: FxHashMap<u64, PrelimDocGroup>,
}
impl PreliminaryIndex {
    /// Print stats about the number of tokens
    pub fn print_stats(&self) {
        // group by token length

        let mut token_length_map: FxHashMap<usize, usize> = FxHashMap::default();

        for group in self.preliminary_docs.values() {
            token_length_map
                .entry(group.template.tokens.len())
                .and_modify(|e| *e += 1)
                .or_insert(1);
        }
        println!("Token Length Stats:");
        // sort by key
        let mut sorted_lengths: Vec<_> = token_length_map.iter().collect();
        sorted_lengths.sort_by_key(|&(k, _)| k);
        for (length, count) in sorted_lengths {
            println!("Length: {}, Count: {}", length, count);
        }
    }
}

#[derive(Debug, Clone)]
pub struct PrelimDocGroup {
    pub template: IndexingTemplate,
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

#[inline]
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
                        token: IndexingTemplateToken::Constant(ConstTemplateToken::new(
                            ct,
                            token.as_str(line).unwrap(),
                        )),
                        token_index: token_pos as u32,
                    }
                }
                Token::Whitespace(num) => TemplateTokenWithMeta {
                    token: IndexingTemplateToken::Whitespace(*num),
                    token_index: token_pos as u32,
                },
            })
            .collect();

        Self {
            template: IndexingTemplate {
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
                IndexingTemplateToken::Constant(existing_ct) => {
                    let token = &tokens[template_token.token_index as usize];
                    let token_bytes = token.as_bytes(line).unwrap();
                    if !fast_short_slice_compare(existing_ct.text.as_bytes(), token_bytes) {
                        let ct = get_term_id(token, line, term_hash_map, false);
                        // This position is now variable
                        let column_index = self.columns.len();
                        let mut new_column =
                            vec![existing_ct.composite_token.term_id(); self.num_docs];
                        new_column.push(ct);
                        self.columns.push(new_column);
                        template_token.token = IndexingTemplateToken::new_variable(column_index);
                    }
                }
                IndexingTemplateToken::Variable {
                    column_index,
                    is_id_like,
                } => {
                    let token = &tokens[template_token.token_index as usize];
                    let term_id = get_term_id(token, line, term_hash_map, *is_id_like);
                    self.columns[*column_index].push(term_id);
                    if self.num_docs == 1000 {
                        *is_id_like = check_is_id_like(&self.columns[*column_index], self.num_docs);
                    }
                }
                IndexingTemplateToken::Whitespace(_) => {
                    // Whitespace is constant within a group
                }
            }
        }
        self.num_docs += 1;
    }
}

#[cold]
/// TODO: The check could be done on a bitvec, since we probably have very few term IDs
pub fn check_is_id_like(column: &[u32], num_docs: usize) -> bool {
    if column.len() != num_docs {
        return false; // Column length mismatch
    }
    let mut seen_ids = std::collections::HashSet::new();
    for term_id in column {
        if !seen_ids.insert(term_id) {
            return false; // Found a duplicate, so not ID-like
        }
    }
    true // All IDs are unique
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CompositeToken {
    token_type: TokenType,
    term_id: u32,
}

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
        CompositeToken {
            token_type,
            term_id,
        }
    }

    /// Extract the TokenType from the top 4 bits
    #[inline]
    pub fn token_type(&self) -> TokenType {
        self.token_type
    }

    /// Extract the 28-bit term ID
    #[inline]
    pub fn term_id(&self) -> u32 {
        self.term_id
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
    let mut preliminary_docs = FxHashMap::default();

    let mut tokens = Vec::new();
    for line in lines {
        let tokenizer = Tokenizer::new(&line);
        tokens.extend(tokenizer);
        //if tokens.len() == 2319 {
        //println!("Line: {}", line);
        //println!("{:?}", tokens_as_string(&line, tokens.iter().cloned()));
        //}
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

#[derive(Clone)]
pub enum SingleOrHashSet {
    Single(Option<u32>),
    HashSet(FxHashSet<u32>),
}
impl Default for SingleOrHashSet {
    fn default() -> Self {
        SingleOrHashSet::Single(None)
    }
}
impl SingleOrHashSet {
    fn insert(&mut self, template_id: u32) {
        match self {
            SingleOrHashSet::Single(opt) => {
                if let Some(existing) = opt {
                    if *existing != template_id {
                        let mut set = FxHashSet::default();
                        set.insert(*existing);
                        set.insert(template_id);
                        *self = SingleOrHashSet::HashSet(set);
                    }
                } else {
                    *opt = Some(template_id);
                }
            }
            SingleOrHashSet::HashSet(set) => {
                set.insert(template_id);
            }
        }
    }
    pub fn copy_into_vec(&self, vec: &mut Vec<u32>) {
        match self {
            SingleOrHashSet::Single(opt) => {
                if let Some(id) = opt {
                    vec.push(*id);
                }
            }
            SingleOrHashSet::HashSet(set) => {
                vec.extend(set.iter().copied());
            }
        }
    }
}

/// Scan the columns and store in which templates a term ID is used
///
/// We can use a vec for the term IDs, since they are guaranteed to be unique within a column.
pub fn term_id_idx_to_template_ids(prelim_index: &PreliminaryIndex) -> Vec<SingleOrHashSet> {
    let mut term_id_to_templates: Vec<SingleOrHashSet> =
        vec![SingleOrHashSet::default(); prelim_index.term_hash_map.len()];

    // TODO: BUG template_id is not known here yet (correct now, but not in the future)
    for (template_id, group) in prelim_index.preliminary_docs.values().enumerate() {
        for column in &group.columns {
            for term_id in column {
                term_id_to_templates[*term_id as usize].insert(template_id as u32);
            }
        }
    }

    term_id_to_templates
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
