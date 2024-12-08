use bytes::{Buf, BufMut, Bytes, BytesMut};
use http_body::{Body, Frame};
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{self, Context, Poll};

#[pin_project::pin_project]
pub(super) struct Lines<B> {
    #[pin]
    body: B,
    data: BytesMut,
    lines: VecDeque<Bytes>,
}

impl<B> Lines<B>
where
    B: Body,
{
    pub(super) fn new(body: B) -> Self {
        Self {
            body,
            data: BytesMut::new(),
            lines: VecDeque::new(),
        }
    }
}

impl<B> Body for Lines<B>
where
    B: Body,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        fn split(data: &mut BytesMut) -> Option<Bytes> {
            let (left, right) =
                (0..data.len()).find_map(|i| match (data.get(i), data.get(i + 1)) {
                    (Some(b'\r'), Some(b'\n')) => Some((i, i + 2)),
                    (Some(b'\r'), Some(_)) => Some((i, i + 1)),
                    (Some(b'\n'), _) => Some((i, i + 1)),
                    _ => None,
                })?;
            let mut line = data.split_to(right).freeze();
            line.truncate(left);
            Some(line)
        }

        let mut this = self.project();
        loop {
            if let Some(line) = this.lines.pop_front() {
                break Poll::Ready(Some(Ok(Frame::data(line))));
            }
            match task::ready!(this.body.as_mut().poll_frame(cx)) {
                Some(Ok(frame)) => match frame.into_data() {
                    Ok(mut data) => {
                        this.data.reserve(data.remaining());
                        while data.has_remaining() {
                            this.data.extend_from_slice(data.chunk());
                            data.advance(data.chunk().len());
                        }
                        while let Some(line) = split(this.data) {
                            this.lines.push_back(line);
                        }
                    }
                    Err(frame) => break Poll::Ready(Some(Ok(frame.map_data(|_| unreachable!())))),
                },
                Some(Err(e)) => break Poll::Ready(Some(Err(e))),
                None => {
                    this.data.put_u8(b'\0');
                    break Poll::Ready(split(this.data).map(Frame::data).map(Ok));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures_test::stream::StreamTestExt;
    use http_body::Frame;
    use http_body_util::{BodyExt, StreamBody};
    use std::convert::Infallible;

    #[rstest::rstest(
        chunk_size => [1, 4, 16, 64],
        end_of_line => [b"\r\n", b"\r", b"\n"],
    )]
    #[futures_test::test]
    async fn test(chunk_size: usize, end_of_line: &[u8]) {
        let lines: &[&[u8]] = &[
            b"The quick brown fox",
            b" ",
            b"jumps over",
            b" the lazy dog.",
            b"",
        ];

        let body = lines.join(end_of_line);
        let body = StreamBody::new(
            futures::stream::iter(
                body.chunks(chunk_size)
                    .map(Bytes::copy_from_slice)
                    .map(Frame::data)
                    .map(Ok::<_, Infallible>),
            )
            .interleave_pending(),
        );
        let mut body = super::Lines::new(body);

        for line in &lines[..lines.len() - 1] {
            assert_eq!(
                body.frame().await.unwrap().unwrap().into_data().unwrap(),
                line,
            );
        }
        assert!(body.frame().await.is_none())
    }
}
