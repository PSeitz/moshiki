use std::ops::Range;

#[derive(Debug, Copy, Clone)]
pub enum Number {
    F64([u8; 8]),
    /// Represents u64 as little-endian bytes
    U64([u8; 8]),
}
impl From<u64> for Number {
    #[inline]
    fn from(num: u64) -> Self {
        Number::U64(num.to_le_bytes())
    }
}
impl From<f64> for Number {
    #[inline]
    fn from(num: f64) -> Self {
        Number::F64(num.to_le_bytes())
    }
}

impl Number {
    #[inline]
    pub fn new(input: &str, range: Range<usize>) -> Self {
        let num_str = &input[range];
        let number = num_str.parse::<u64>();
        match number {
            Ok(n) => n.into(),
            Err(_) => {
                let num = num_str
                    .parse::<f64>()
                    .expect("Failed to parse number as f64");
                num.into()
            }
        }
    }
    #[inline]
    pub fn as_bytes(&self, _input: &str) -> &[u8] {
        match self {
            Number::F64(bytes) => bytes,
            Number::U64(bytes) => bytes,
        }
    }
    #[inline]
    pub fn to_string(&self, _input: &str) -> String {
        match self {
            Number::F64(bytes) => f64::from_le_bytes(*bytes).to_string(),
            Number::U64(bytes) => u64::from_le_bytes(*bytes).to_string(),
        }
    }
}
