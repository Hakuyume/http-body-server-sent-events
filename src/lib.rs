mod decode;

pub use decode::{decode, Decode};
use std::time::Duration;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Event {
    pub event: Option<String>,
    pub data: Option<String>,
    pub id: Option<String>,
    pub retry: Option<Duration>,
}
