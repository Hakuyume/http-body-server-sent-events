mod lines;

use crate::Event;
use http_body::{Body, Frame};
use std::mem;
use std::pin::Pin;
use std::str::{self, Utf8Error};
use std::task::{self, Context, Poll};
use std::time::Duration;

pub fn decode<B>(body: B) -> Decode<B>
where
    B: Body,
    B::Error: From<Utf8Error>,
{
    Decode {
        lines: lines::Lines::new(body),
        event: Event::default(),
    }
}

#[pin_project::pin_project]
pub struct Decode<B> {
    #[pin]
    lines: lines::Lines<B>,
    event: Event,
}

impl<B> Decode<B>
where
    B: Body,
    B::Error: From<Utf8Error>,
{
    pub fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Event>, B::Error>>> {
        // https://html.spec.whatwg.org/multipage/server-sent-events.html#processField
        fn process_field(event: &mut Event, field: &str, value: &str) {
            match field {
                "event" => event.event = Some(value.to_owned()),
                "data" => {
                    let data = event.data.get_or_insert_default();
                    data.push_str(value);
                    data.push('\n');
                }
                "id" => {
                    event.id = Some(value.to_owned());
                }
                "retry" => {
                    if let Ok(value) = value.parse() {
                        event.retry = Some(Duration::from_millis(value));
                    }
                }
                _ => (),
            }
        }

        let mut this = self.project();
        loop {
            match task::ready!(this.lines.as_mut().poll_frame(cx)) {
                Some(Ok(frame)) => match frame.into_data() {
                    Ok(line) => {
                        let line = str::from_utf8(&line)?;
                        if line.is_empty() {
                            let mut event = mem::take(this.event);
                            if event.event.is_some()
                                || event.data.is_some()
                                || event.id.is_some()
                                || event.retry.is_some()
                            {
                                if let Some(data) = &mut event.data {
                                    assert_eq!(data.pop(), Some('\n'));
                                }
                                break Poll::Ready(Some(Ok(Frame::data(event))));
                            }
                        } else if line.starts_with(':') {
                            // comment
                        } else {
                            let (field, value) = line.split_once(':').unwrap_or((line, ""));
                            let value = value.strip_prefix(' ').unwrap_or(value);
                            process_field(this.event, field, value);
                        }
                    }
                    Err(frame) => break Poll::Ready(Some(Ok(frame.map_data(|_| unreachable!())))),
                },
                Some(Err(e)) => break Poll::Ready(Some(Err(e))),
                None => break Poll::Ready(None),
            }
        }
    }

    pub fn into_event_stream(self) -> impl futures::Stream<Item = Result<Event, B::Error>> {
        #[pin_project::pin_project]
        struct Stream<B>(#[pin] Decode<B>);
        impl<B> futures::Stream for Stream<B>
        where
            B: Body,
            B::Error: From<Utf8Error>,
        {
            type Item = Result<Event, B::Error>;
            fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
                let mut this = self.project();
                loop {
                    match task::ready!(this.0.as_mut().poll_frame(cx)) {
                        Some(Ok(frame)) => {
                            if let Ok(event) = frame.into_data() {
                                break Poll::Ready(Some(Ok(event)));
                            }
                        }
                        Some(Err(e)) => break Poll::Ready(Some(Err(e))),
                        None => break Poll::Ready(None),
                    }
                }
            }
        }

        Stream(self)
    }
}

#[cfg(test)]
mod tests;
