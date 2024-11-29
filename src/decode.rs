use bytes::{BufMut, Bytes, BytesMut};
use futures::Stream;
use http_body::{Body, Frame};
use std::collections::VecDeque;
use std::pin::Pin;
use std::str::Utf8Error;
use std::task::{self, Context, Poll};

pub fn decode<B>(body: B) -> Decode<B>
where
    B: Body<Data = Bytes>,
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
    events: VecDeque<crate::Event>,
}

impl<B> Stream for Decode<B>
where
    B: Body<Data = Bytes>,
    B::Error: From<Utf8Error>,
{
    type Item = Result<Frame<crate::Event>, B::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        loop {
            if let Some(event) = this.events.pop_front() {
                break Poll::Ready(Some(Ok(Frame::data(event))));
            }
            match task::ready!(this.body.as_mut().poll_frame(cx)) {
                Some(Ok(frame)) => match frame.into_data() {
                    Ok(data) => {
                        this.data.put_slice(&data);
                        while let Some(at) =
                            this.data.windows(2).position(|window| window == b"\n\n")
                        {
                            let data = this.data.split_to(at + 2);
                            if let Some(event) = crate::Event::decode(&data)? {
                                this.events.push_back(event);
                            }
                        }
                    }
                    Err(frame) => break Poll::Ready(Some(Ok(frame.map_data(|_| unreachable!())))),
                },
                Some(Err(e)) => break Poll::Ready(Some(Err(e))),
                None => {
                    let data = this.data.split();
                    break Poll::Ready(crate::Event::decode(&data)?.map(Frame::data).map(Ok));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
