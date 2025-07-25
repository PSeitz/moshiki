use fxhash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::TemplateId;
use crate::indexing::DocGroups;
use crate::indexing::termmap::TermStore;
use crate::tokenizer::{Token, TokenType, TokenTypeTrait, Tokenizer};
use stacker::fastcmp::fast_short_slice_compare;

use super::termmap::IndexingTermmap;

#[derive(Debug, Clone, Default)]
pub(crate) struct IndexingTemplate {
    pub template_id: TemplateId,
    pub num_docs: usize,
    pub tokens: Vec<TemplateTokenWithMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub(crate) struct TemplateTokenWithMeta {
    pub token: IndexingTemplateToken,
    /// This is the index in the token sequence
    pub token_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub(crate) enum IndexingTemplateToken {
    Constant(ConstTemplateToken),
    Variable {
        is_id_like: bool,
        column_index: usize,
        token_type: TokenType,
    },
    #[cfg(feature = "whitespace")]
    Whitespace(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub(crate) struct ConstTemplateToken {
    pub(crate) composite_token: CompositeToken,
    // u64 LE bytes for numbers (with feature_flag `number_as_string`)
    // String for words
    pub(crate) text: Vec<u8>,
}
impl ConstTemplateToken {
    pub(crate) fn new(token: CompositeToken, text: Vec<u8>) -> Self {
        ConstTemplateToken {
            composite_token: token,
            text,
        }
    }
}
impl TokenTypeTrait for IndexingTemplateToken {
    fn token_type(&self) -> TokenType {
        match self {
            IndexingTemplateToken::Constant(ct) => ct.composite_token.token_type(),
            IndexingTemplateToken::Variable { token_type, .. } => *token_type,
            #[cfg(feature = "whitespace")]
            IndexingTemplateToken::Whitespace(_) => TokenType::Whitespace,
        }
    }
}

impl IndexingTemplateToken {
    pub(crate) fn new_variable(column_index: usize, token_type: TokenType) -> Self {
        IndexingTemplateToken::Variable {
            column_index,
            is_id_like: false,
            token_type,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn is_variable(&self) -> bool {
        match self {
            IndexingTemplateToken::Constant(_) => false,
            IndexingTemplateToken::Variable { .. } => true,
            #[cfg(feature = "whitespace")]
            IndexingTemplateToken::Whitespace(_) => false,
        }
    }
}

/// A preliminary index that contains the term hash map and document groups.
pub struct PreliminaryIndex {
    pub(crate) term_hash_map: IndexingTermmap,
    /// Document groups, keyed by the token length.
    pub doc_groups: DocGroups,
}
impl PreliminaryIndex {
    pub(crate) fn iter_templates(&self) -> impl Iterator<Item = &IndexingTemplate> {
        self.doc_groups.values().map(|group| &group.template)
    }
    /// Print stats about the number of tokens
    pub(crate) fn print_stats(&self) {
        // group by token length
        //
        #[derive(Debug, Clone, Hash, PartialEq, Eq)]
        struct Stats {
            num_templates: usize,
            num_docs: usize,
            vals_in_columns: usize,
            token_lists: Vec<Vec<TemplateTokenWithMeta>>,
        }
        let mut token_length_map: FxHashMap<usize, Stats> = FxHashMap::default();

        for group in self.doc_groups.values() {
            token_length_map
                .entry(group.template.tokens.len())
                .and_modify(|e| {
                    e.num_templates += 1; // Increment count of this length
                    e.num_docs += group.num_docs; // Add number of documents
                    e.vals_in_columns += group.vals_in_columns();
                    e.token_lists.push(group.template.tokens.clone());
                })
                .or_insert(Stats {
                    num_templates: 1,
                    num_docs: group.num_docs,
                    vals_in_columns: group.vals_in_columns(),
                    token_lists: vec![group.template.tokens.clone()],
                });
        }
        println!("Token Length Stats:");
        // sort by key
        let mut sorted_lengths: Vec<_> = token_length_map.iter().collect();
        sorted_lengths.sort_by_key(|&(k, _)| k);
        for (length, stats) in sorted_lengths {
            println!(
                "Num Tokens: {length}, Num Templates: {} Num Docs: {} ValsInColumns: {}",
                stats.num_templates, stats.num_docs, stats.vals_in_columns
            );
            // Print the token types to see how they differ
            if stats.token_lists.len() > 1 {
                for tokens in stats.token_lists.iter() {
                    let token_types: String = tokens
                        .iter()
                        .map(|tt| tt.token.token_type().get_color_code())
                        .collect();
                    println!("{token_types}");
                }
            }
        }

        println!("Total Number of Groups: {}", self.doc_groups.num_groups());

        // Dictionary stats
        // Avg length of terms
        let total_terms = self.term_hash_map.regular.num_terms();
        let total_length: usize = self
            .term_hash_map
            .regular
            .iter()
            .map(|(term_bytes, _)| term_bytes.len())
            .sum::<usize>();
        let avg_length = total_length as f32 / total_terms as f32;
        println!("Total Terms: {total_terms}, Avg Length: {avg_length:.2}");

        // Print the number of: unique like, constant, and variable tokens
        let mut num_like = 0;
        let mut num_constant = 0;
        let mut num_variable = 0;
        for group in self.doc_groups.values() {
            for template_token in &group.template.tokens {
                match &template_token.token {
                    IndexingTemplateToken::Constant(_) => num_constant += 1,
                    IndexingTemplateToken::Variable { is_id_like, .. } => {
                        num_variable += 1;
                        if *is_id_like {
                            num_like += 1;
                        }
                    }
                    #[cfg(feature = "whitespace")]
                    IndexingTemplateToken::Whitespace(_) => {}
                }
            }
        }
        println!(
            "Total Tokens: {}, Constant: {}, Variable: {}, ID-like: {}",
            num_constant + num_variable,
            num_constant,
            num_variable,
            num_like
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
    match token {
        Token::IPv4(v) | Token::Uuid(v) | Token::Word(v) | Token::Punctuation(v) => {
            let term_slice = &line.as_bytes()[v.start as usize..v.end as usize];
            term_hash_map.mutate_or_create(term_slice, is_id_like)
        }
        #[cfg(feature = "whitespace")]
        Token::Whitespace(num_whitespace) => *num_whitespace,
        Token::Number(number) => term_hash_map.mutate_or_create(number.as_bytes(line), is_id_like),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PrelimDocGroup {
    pub(crate) template: IndexingTemplate,
    /// Tokens of the first document in this group. We use it to compare token types
    //pub tokens: Vec<Token>,
    pub(crate) columns: Vec<Vec<u32>>,
    pub(crate) num_docs: usize,
}

impl PrelimDocGroup {
    pub(crate) fn vals_in_columns(&self) -> usize {
        self.columns.iter().map(|c| c.len()).sum()
    }

    #[inline]
    pub(crate) fn remove_rows<F>(&mut self, mut keep: F)
    where
        F: FnMut(&u32) -> bool,
    {
        for column in self.columns.iter_mut() {
            let mut row = 0;
            column.retain(|_| {
                let keep = keep(&row);
                row += 1;
                keep
            });
        }
    }

    /// Return an iterator over the columns, yielding (&[u32])
    pub fn iter_columns(&self) -> impl Iterator<Item = &[u32]> {
        self.template.tokens.iter().flat_map(|template_token| {
            // Iterate in the right order
            match template_token.token {
                IndexingTemplateToken::Variable { column_index, .. } => {
                    Some(self.columns[column_index].as_slice())
                }
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

    pub fn convert_to_variable(&mut self, token_idx: usize, _term_hash_map: &mut IndexingTermmap) {
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
            #[cfg(feature = "whitespace")]
            IndexingTemplateToken::Whitespace(num) => {
                let white_space = " ".repeat(*num as usize);
                let term_id = _term_hash_map.mutate_or_create(white_space.as_bytes(), false, false);
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
                #[cfg(feature = "whitespace")]
                Token::Whitespace(num) => TemplateTokenWithMeta {
                    token: IndexingTemplateToken::Whitespace(*num),
                    token_index: token_pos as u32,
                },
            })
            .collect();

        Self {
            template: IndexingTemplate {
                template_id: 0.into(), // This will be set later
                num_docs: 0,           // This will be set later
                tokens: template_tokens,
            },
            columns: Vec::new(),
            num_docs: 1,
            //tokens: tokens.to_vec(),
        }
    }

    #[inline]
    pub(crate) fn push(
        &mut self,
        tokens: &[Token],
        line: &str,
        term_hash_map: &mut IndexingTermmap,
    ) {
        // Compare with template and update if necessary
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
                    if self.num_docs == 10000 {
                        *is_id_like = check_is_id_like(&self.columns[*column_index]);
                    }
                }
                #[cfg(feature = "whitespace")]
                IndexingTemplateToken::Whitespace(_) => {
                    // Whitespace is constant within a group
                }
            }
        }
        self.num_docs += 1;
    }

    //pub(crate) fn matches_token_types(&self, tokens: &[Token]) -> bool {
    //assert_eq!(
    //self.template.tokens.len(),
    //tokens.len(),
    //"Token length mismatch: {} != {}",
    //self.template.tokens.len(),
    //tokens.len()
    //);
    //for (template_token, token) in self.tokens.iter().zip(tokens) {
    //if !template_token.matches(token) {
    //return false;
    //}
    //}
    //true
    //}
}

/// TODO: The check could be done on a bitvec, since we probably have very few term IDs
#[inline(never)]
pub fn check_is_id_like(column: &[u32]) -> bool {
    let mut seen_ids = FxHashSet::default();
    for term_id in column {
        if !seen_ids.insert(term_id) {
            //return false; // Found a duplicate
        }
    }
    let unique_count = seen_ids.len();
    let total_count = column.len();
    let unique_ratio = unique_count as f32 / total_count as f32;
    unique_ratio >= 0.98

    //unique_count == total_count
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct CompositeToken {
    token_type: TokenType,
    term_id: u32,
}

impl CompositeToken {
    #[inline]
    pub(crate) fn new(token_type: TokenType, term_id: u32) -> Self {
        CompositeToken {
            token_type,
            term_id,
        }
    }

    #[inline]
    pub(crate) fn token_type(&self) -> TokenType {
        self.token_type
    }

    #[inline]
    pub(crate) fn term_id(&self) -> u32 {
        self.term_id
    }
}
impl From<(TokenType, u32)> for CompositeToken {
    #[inline]
    fn from(value: (TokenType, u32)) -> Self {
        CompositeToken::new(value.0, value.1)
    }
}

/// Create a preliminary index from log lines
pub fn preliminary_index<T: Into<String>>(lines: impl Iterator<Item = T>) -> PreliminaryIndex {
    let mut term_hash_map = IndexingTermmap::default();
    let mut preliminary_docs = DocGroups::default();

    let mut tokens = Vec::new();
    //let mut num = 0;
    for line in lines {
        //num += 1;
        let line: String = line.into();
        let tokenizer = Tokenizer::new(&line);
        tokens.extend(tokenizer);
        if tokens.len() == 2318 {
            //println!("num: {num}");
            println!("Line: {line:?}");
            println!(
                "{:?}",
                crate::tokenizer::tokens_as_string(&line, tokens.iter().cloned())
            );
        }
        //let fingerprint = fingerprint(&tokens);

        preliminary_docs.insert(&tokens, &line, &mut term_hash_map);
        //.entry(fingerprint)
        //.or_insert_with(|| PrelimDocGroup::new(&tokens, &line, &mut term_hash_map));
        //group.push(&tokens, &line, &mut term_hash_map);
        tokens.clear();
    }

    PreliminaryIndex {
        term_hash_map,
        doc_groups: preliminary_docs,
    }
}

#[derive(Clone)]
pub(crate) enum SingleOrHashSet {
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
pub(crate) fn term_id_idx_to_template_ids(prelim_index: &PreliminaryIndex) -> Vec<SingleOrHashSet> {
    let num_terms = prelim_index.term_hash_map.regular.num_terms();
    // Poor mans bitvec
    let mut marked_termids = vec![false; num_terms];

    let mut term_id_to_templates: Vec<SingleOrHashSet> =
        vec![SingleOrHashSet::default(); prelim_index.term_hash_map.regular.num_terms()];

    for (template_id, group) in prelim_index.doc_groups.values().enumerate() {
        for column in group.iter_columns() {
            if column.len() > 500_000 {
                for term_id in column.iter().copied() {
                    marked_termids[term_id as usize] = true;
                }
                for (term_id, is_marked) in marked_termids.iter().enumerate() {
                    if *is_marked {
                        term_id_to_templates[term_id].insert(template_id as u32);
                    }
                }
                marked_termids.fill(false);
            } else {
                for term_id in dedup_term_ids_iter(column.iter().copied()) {
                    term_id_to_templates[term_id as usize].insert(template_id as u32);
                }
            }
        }
    }

    term_id_to_templates
}

// Filter repeated term IDs in an iterator (in a row, not globally)
fn dedup_term_ids_iter(iter: impl Iterator<Item = u32>) -> impl Iterator<Item = u32> {
    let mut last_id: Option<u32> = None;
    iter.filter(move |&id| {
        if Some(id) == last_id {
            false
        } else {
            last_id = Some(id);
            true
        }
    })
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
