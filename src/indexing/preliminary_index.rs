use fxhash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::indexing::termmap::TermStore;
use crate::tokenizer::{Token, TokenType, Tokenizer};
use stacker::fastcmp::fast_short_slice_compare;

use super::{fingerprint, termmap::IndexingTermmap};

#[derive(Debug, Clone, Default)]
pub struct IndexingTemplate {
    pub template_id: u32,
    pub tokens: Vec<TemplateTokenWithMeta>,
}

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
        token_type: TokenType,
    },
    Whitespace(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConstTemplateToken {
    pub composite_token: CompositeToken,
    // u64 LE bytes for numbers
    // String for words
    pub text: Vec<u8>,
}
impl ConstTemplateToken {
    pub fn new(token: CompositeToken, text: Vec<u8>) -> Self {
        ConstTemplateToken {
            composite_token: token,
            text,
        }
    }
    pub fn term_id(&self) -> u32 {
        self.composite_token.term_id()
    }
}

impl IndexingTemplateToken {
    pub fn new_variable(column_index: usize, token_type: TokenType) -> Self {
        IndexingTemplateToken::Variable {
            column_index,
            is_id_like: false,
            token_type,
        }
    }

    pub fn is_variable(&self) -> bool {
        match self {
            IndexingTemplateToken::Constant(_) => false,
            IndexingTemplateToken::Variable { .. } => true,
            IndexingTemplateToken::Whitespace(_) => false,
        }
    }
    pub fn token_type(&self) -> TokenType {
        match self {
            IndexingTemplateToken::Constant(ct) => ct.composite_token.token_type(),
            IndexingTemplateToken::Variable { token_type, .. } => *token_type,
            IndexingTemplateToken::Whitespace(_) => TokenType::Whitespace,
        }
    }
}

pub struct PreliminaryIndex {
    pub term_hash_map: IndexingTermmap,
    pub doc_groups: FxHashMap<u64, PrelimDocGroup>,
}
impl PreliminaryIndex {
    pub fn iter_templates(&self) -> impl Iterator<Item = &IndexingTemplate> {
        self.doc_groups.values().map(|group| &group.template)
    }
    /// Print stats about the number of tokens
    pub fn print_stats(&self) {
        // group by token length
        let mut token_length_map: FxHashMap<usize, (usize, usize)> = FxHashMap::default();

        for group in self.doc_groups.values() {
            token_length_map
                .entry(group.template.tokens.len())
                .and_modify(|e| {
                    e.0 += 1; // Increment count of this length
                    e.1 += group.num_docs; // Add number of documents
                })
                .or_insert((1, group.num_docs));
        }
        println!("Token Length Stats:");
        // sort by key
        let mut sorted_lengths: Vec<_> = token_length_map.iter().collect();
        sorted_lengths.sort_by_key(|&(k, _)| k);
        for (length, (count, num_docs)) in sorted_lengths {
            println!("Num Tokens: {length}, Num Templates: {count} Num Docs: {num_docs}");
        }

        println!("Total Number of Groups: {}", self.doc_groups.len());

        // Dictionary stats
        // Avg length of terms
        let total_terms = self.term_hash_map.regular.len();
        let total_length: usize = self
            .term_hash_map
            .regular
            .iter()
            .map(|(term_bytes, _)| term_bytes.len())
            .sum::<usize>();
        let avg_length = total_length as f32 / total_terms as f32;
        println!("Total Terms: {total_terms}, Avg Length: {avg_length:.2}");
        let total_catch_all_terms = (&self.term_hash_map.catch_all).len();
        let total_catch_all_length: usize = self
            .term_hash_map
            .catch_all
            .iter()
            .map(|(term_bytes, _)| term_bytes.len())
            .sum();
        let avg_catch_all_length = total_catch_all_length as f32 / total_catch_all_terms as f32;
        println!(
            "Total CatchAll Terms: {total_catch_all_terms}, Avg Length: {avg_catch_all_length:.2}"
        );
    }
}

fn create_composite_token(
    token: &Token,
    line: &str,
    term_hash_map: &mut IndexingTermmap,
    is_id_like: bool,
) -> CompositeToken {
    (
        token.token_type(),
        get_term_id(token, line, term_hash_map, is_id_like),
    )
        .into()
}

#[inline]
fn get_term_id(
    token: &Token,
    line: &str,
    term_hash_map: &mut IndexingTermmap,
    is_id_like: bool,
) -> u32 {
    let token_type = token.token_type();
    match token {
        Token::IPv4(v)
        | Token::Uuid(v)
        | Token::Word(v)
        | Token::CatchAll(v)
        | Token::Punctuation(v) => {
            let term_slice = &line.as_bytes()[v.start as usize..v.end as usize];
            term_hash_map.mutate_or_create(term_slice, is_id_like, token_type.is_catch_all())
        }
        Token::Whitespace(num_whitespace) => *num_whitespace,
        Token::Number(number) => {
            term_hash_map.mutate_or_create(number.as_bytes(), is_id_like, token_type.is_catch_all())
        }
    }
}

#[derive(Debug, Clone)]
pub struct PrelimDocGroup {
    pub template: IndexingTemplate,
    pub columns: Vec<Vec<u32>>,
    pub num_docs: usize,
}

impl PrelimDocGroup {
    pub fn for_each_mut_column<F>(&mut self, mut cb: F)
    where
        F: FnMut(bool, &mut [u32]),
    {
        // We can borrow `self.columns` mutably up front, and only borrow `self.template.tokens` immutably, so these borrows don’t conflict.
        let columns = &mut self.columns;
        for template_token in &self.template.tokens {
            if let IndexingTemplateToken::Variable { column_index, .. } = template_token.token {
                let catch_all = template_token.token.token_type().is_catch_all();
                let slice = &mut columns[column_index][..];
                cb(catch_all, slice);
            }
        }
    }
    /// Return an iterator over the columns, yielding a tuple of (is_catch_all, &[u32])
    pub fn iter_columns(&self) -> impl Iterator<Item = (bool, &[u32])> {
        self.template.tokens.iter().flat_map(|template_token| {
            // Iterate in the right order
            match template_token.token {
                IndexingTemplateToken::Variable { column_index, .. } => Some((
                    template_token.token.token_type().is_catch_all(),
                    self.columns[column_index].as_slice(),
                )),
                _ => None,
            }
        })
    }

    pub fn append(&mut self, other: &PrelimDocGroup) {
        self.num_docs += other.num_docs;
        // Merge only variable columns
        for (target_token, source_token) in self
            .template
            .tokens
            .iter()
            .zip(other.template.tokens.iter())
        {
            if let (
                IndexingTemplateToken::Variable {
                    column_index: target_index,
                    ..
                },
                IndexingTemplateToken::Variable {
                    column_index: source_index,
                    ..
                },
            ) = (&target_token.token, &source_token.token)
            {
                // Append the source column to the target column
                self.columns[*target_index].extend_from_slice(&other.columns[*source_index]);
            }
        }
    }
    pub fn convert_to_variable(&mut self, token_idx: usize, term_hash_map: &mut IndexingTermmap) {
        // Convert the token at token_idx to a variable
        let template_token = &mut self.template.tokens[token_idx];
        match &mut template_token.token {
            IndexingTemplateToken::Constant(existing_ct) => {
                // This position is now variable
                let column_index = self.columns.len();
                let new_column = vec![existing_ct.composite_token.term_id(); self.num_docs];
                self.columns.push(new_column);
                template_token.token = IndexingTemplateToken::new_variable(
                    column_index,
                    existing_ct.composite_token.token_type(),
                );
            }
            IndexingTemplateToken::Whitespace(num) => {
                let white_space = " ".repeat(*num as usize);
                let term_id = term_hash_map.mutate_or_create(white_space.as_bytes(), false, false);
                // This position is now variable
                let column_index = self.columns.len();
                let new_column = vec![term_id; self.num_docs];
                self.columns.push(new_column);
                template_token.token =
                    IndexingTemplateToken::new_variable(column_index, TokenType::Whitespace);
            }
            IndexingTemplateToken::Variable { .. } => {}
        }
    }

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
                | Token::CatchAll(_)
                | Token::Punctuation(_) => {
                    let ct = create_composite_token(token, line, term_hash_map, false);
                    TemplateTokenWithMeta {
                        token: IndexingTemplateToken::Constant(ConstTemplateToken::new(
                            ct,
                            token
                                .as_bytes(line)
                                .expect("Token should have bytes (except whitespace)")
                                .to_vec(),
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
                template_id: 0, // This will be set later
                tokens: template_tokens,
            },
            columns: Vec::new(),
            num_docs: 0,
        }
    }

    #[inline]
    fn push(&mut self, tokens: &[Token], line: &str, term_hash_map: &mut IndexingTermmap) {
        // Compare with template and update if necessary
        // TODO: fast path here to quickly hashcheck all the constants after e.g. 1000 documents
        for template_token in &mut self.template.tokens {
            match &mut template_token.token {
                IndexingTemplateToken::Constant(existing_ct) => {
                    let token = &tokens[template_token.token_index as usize];
                    let token_bytes = token
                        .as_bytes(line)
                        .expect("Token should have bytes (except whitespace)");
                    if !fast_short_slice_compare(&existing_ct.text, token_bytes) {
                        let ct = get_term_id(token, line, term_hash_map, false);
                        // This position is now variable
                        let column_index = self.columns.len();
                        let mut new_column =
                            vec![existing_ct.composite_token.term_id(); self.num_docs];
                        new_column.push(ct);
                        self.columns.push(new_column);
                        template_token.token =
                            IndexingTemplateToken::new_variable(column_index, token.token_type());
                    }
                }
                IndexingTemplateToken::Variable {
                    column_index,
                    is_id_like,
                    ..
                } => {
                    let token = &tokens[template_token.token_index as usize];
                    let term_id = get_term_id(token, line, term_hash_map, *is_id_like);
                    self.columns[*column_index].push(term_id);
                    if self.num_docs == 1000 {
                        *is_id_like = check_is_id_like(&self.columns[*column_index]);
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
pub fn check_is_id_like(column: &[u32]) -> bool {
    let mut seen_ids = FxHashSet::default();
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
        doc_groups: preliminary_docs,
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
pub fn term_id_idx_to_template_ids(
    prelim_index: &PreliminaryIndex,
) -> (Vec<SingleOrHashSet>, Vec<SingleOrHashSet>) {
    // TODO: BUG template_id is not known here yet (correct now, but probably not in the future)
    let mut catch_all_term_id_to_templates: Vec<SingleOrHashSet> =
        vec![SingleOrHashSet::default(); prelim_index.term_hash_map.catch_all.len()];
    let mut term_id_to_templates: Vec<SingleOrHashSet> =
        vec![SingleOrHashSet::default(); prelim_index.term_hash_map.regular.len()];

    // TODO: BUG template_id is not known here yet (correct now, but not in the future)
    for (template_id, group) in prelim_index.doc_groups.values().enumerate() {
        for (is_catch_all, column) in group.iter_columns() {
            if is_catch_all {
                for term_id in column {
                    catch_all_term_id_to_templates[*term_id as usize].insert(template_id as u32);
                }
            } else {
                for term_id in column {
                    term_id_to_templates[*term_id as usize].insert(template_id as u32);
                }
            }
        }
    }

    (term_id_to_templates, catch_all_term_id_to_templates)
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
