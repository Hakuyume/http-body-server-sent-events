use crate::Event;
use bytes::{Buf, BytesMut};
use http_body::{Body, Frame};
use std::collections::VecDeque;
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
        body,
        data: BytesMut::new(),
        events: VecDeque::new(),
    }
}

#[pin_project::pin_project]
pub struct Decode<B> {
    #[pin]
    body: B,
    data: BytesMut,
    events: VecDeque<Event>,
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
        let mut this = self.project();
        loop {
            if let Some(event) = this.events.pop_front() {
                break Poll::Ready(Some(Ok(Frame::data(event))));
            }
            match task::ready!(this.body.as_mut().poll_frame(cx)) {
                Some(Ok(frame)) => match frame.into_data() {
                    Ok(mut data) => {
                        this.data.reserve(data.remaining());
                        while data.has_remaining() {
                            this.data.extend_from_slice(data.chunk());
                            data.advance(data.chunk().len());
                        }
                        while let Some(at) =
                            this.data.windows(2).position(|window| window == b"\n\n")
                        {
                            let data = this.data.split_to(at + 2);
                            if let Some(event) = decode_data(&data)? {
                                this.events.push_back(event);
                            }
                        }
                    }
                    Err(frame) => break Poll::Ready(Some(Ok(frame.map_data(|_| unreachable!())))),
                },
                Some(Err(e)) => break Poll::Ready(Some(Err(e))),
                None => {
                    let data = this.data.split();
                    break Poll::Ready(decode_data(&data)?.map(Frame::data).map(Ok));
                }
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

fn decode_data(data: &[u8]) -> Result<Option<Event>, Utf8Error> {
    let data = str::from_utf8(data)?;
    let event = data.lines().fold(Event::default(), |mut event, line| {
        if let Some(value) = line.strip_prefix("event:") {
            event.event = Some(value.trim_start().to_owned());
        }
        if let Some(line) = line.strip_prefix("data:") {
            let data = match &mut event.data {
                Some(data) => {
                    data.push('\n');
                    data
                }
                None => event.data.insert(String::new()),
            };
            data.push_str(line.trim_start());
        }
        if let Some(value) = line.strip_prefix("id:") {
            event.id = Some(value.trim_start().to_owned());
        }
        if let Some(Ok(value)) = line
            .strip_prefix("retry:")
            .map(|line| line.trim_start().parse())
        {
            event.retry = Some(Duration::from_millis(value));
        }
        event
    });
    if event.event.is_some() || event.data.is_some() || event.id.is_some() || event.retry.is_some()
    {
        Ok(Some(event))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests;
