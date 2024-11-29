use std::str::{self, Utf8Error};

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub event: Option<String>,
    pub data: Option<String>,
    pub id: Option<String>,
    pub retry: Option<u64>,
}

impl Event {
    pub(crate) fn decode(data: &[u8]) -> Result<Option<Self>, Utf8Error> {
        let data = str::from_utf8(data)?;
        let this = data.lines().fold(
            Self {
                event: None,
                data: None,
                id: None,
                retry: None,
            },
            |mut this, line| {
                if let Some(value) = line.strip_prefix("event:") {
                    this.event = Some(value.trim_start().to_owned());
                }
                if let Some(line) = line.strip_prefix("data:") {
                    let data = match &mut this.data {
                        Some(data) => {
                            data.push('\n');
                            data
                        }
                        None => this.data.insert(String::new()),
                    };
                    data.push_str(line.trim_start());
                }
                if let Some(value) = line.strip_prefix("id:") {
                    this.id = Some(value.trim_start().to_owned());
                }
                if let Some(Ok(value)) = line.strip_prefix("retry:").map(str::parse) {
                    this.event = Some(value);
                }
                this
            },
        );
        if this.event.is_some() || this.data.is_some() || this.id.is_some() || this.retry.is_some()
        {
            Ok(Some(this))
        } else {
            Ok(None)
        }
    }
}
