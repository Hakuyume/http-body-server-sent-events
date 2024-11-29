use bytes::{BufMut, Bytes, BytesMut};

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub data: Option<Bytes>,
}

impl Event {
    pub(crate) fn decode(data: &[u8]) -> Self {
        let data = data
            .split(|b| *b == b'\n')
            .fold(None, |mut data: Option<BytesMut>, line| {
                if let Some(line) = line.strip_prefix(b"data:") {
                    let data = match &mut data {
                        Some(data) => {
                            data.put_u8(b'\n');
                            data
                        }
                        None => data.insert(BytesMut::new()),
                    };
                    data.put_slice(line.trim_ascii_start());
                }
                data
            });
        Self {
            data: data.map(BytesMut::freeze),
        }
    }
}
