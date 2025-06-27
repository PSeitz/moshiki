use stacker::ArenaHashMap;

use crate::{
    Token,
    tokenizer::{TokenType, Tokenizer},
};

pub struct PreliminaryIndex {
    pub term_hash_map: ArenaHashMap,
    pub preliminary_docs: Vec<Vec<PrelimDoc>>,
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
    let mut preliminary_docs: Vec<Vec<PrelimDoc>> = Vec::new();

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
                    // Only map to ordinal ID after the term_hash_map is fully populated
                    // For now, use the original term_id
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
        let num_tokens = prelim_doc.without_whitespace().count();

        if num_tokens >= preliminary_docs.len() {
            preliminary_docs.resize(num_tokens + 1, Vec::new());
        }
        preliminary_docs[num_tokens].push(prelim_doc);
    }

    //let old_to_new_id_map = generate_term_id_mapping(&term_hash_map);
    //remap_term_ids(&mut preliminary_docs, &old_to_new_id_map);

    PreliminaryIndex {
        term_hash_map,
        preliminary_docs,
    }
}

fn generate_term_id_mapping(term_hash_map: &ArenaHashMap) -> Vec<u32> {
    let mut sorted_terms: Vec<(&[u8], u32)> = Vec::with_capacity(term_hash_map.len());
    let max_old_id = term_hash_map.len() as u32;
    for (term_bytes, old_id_addr) in term_hash_map.iter() {
        let old_id: u32 = term_hash_map.read(old_id_addr);
        sorted_terms.push((term_bytes, old_id));
    }
    sorted_terms.sort_by(|(term_a, _), (term_b, _)| term_a.cmp(term_b));

    let mut old_to_new_id_map: Vec<u32> = vec![0; (max_old_id + 1) as usize];
    for (new_id, (_, old_id)) in sorted_terms.into_iter().enumerate() {
        old_to_new_id_map[old_id as usize] = new_id as u32;
    }
    old_to_new_id_map
}

fn remap_term_ids(preliminary_docs: &mut [Vec<PrelimDoc>], old_to_new_id_map: &[u32]) {
    for docs_vec in preliminary_docs.iter_mut() {
        for doc in docs_vec.iter_mut() {
            for composite_token in doc.0.iter_mut() {
                if !composite_token.token_type().is_whitespace() {
                    let old_term_id = composite_token.term_id();
                    let ordinal_term_id = old_to_new_id_map[old_term_id as usize];
                    *composite_token =
                        CompositeToken::new(composite_token.token_type(), ordinal_term_id);
                }
            }
        }
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
