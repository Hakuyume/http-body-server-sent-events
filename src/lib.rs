mod decode;

pub use decode::{decode, Decode};

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub event: Option<String>,
    pub data: Option<String>,
    pub id: Option<String>,
    pub retry: Option<u64>,
}
