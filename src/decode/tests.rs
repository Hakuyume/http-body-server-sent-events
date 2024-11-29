use crate::Event;
use bytes::Bytes;
use futures::{StreamExt, TryStreamExt};
use http_body::Frame;
use http_body_util::StreamBody;
use std::str::Utf8Error;
use std::time::Duration;

async fn check<'a, I>(iter: I, expected: &[Event])
where
    I: IntoIterator<Item = &'a [u8]>,
{
    let iter = iter
        .into_iter()
        .map(|chunk| Ok::<_, Utf8Error>(Frame::data(Bytes::copy_from_slice(chunk))));
    let events = &super::decode(StreamBody::new(futures::stream::iter(iter).then(
        |chunk| async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            chunk
        },
    )))
    .try_filter_map(|frame| futures::future::ok(frame.into_data().ok()))
    .try_collect::<Vec<_>>()
    .await
    .unwrap();
    assert_eq!(events, expected);
}

#[rstest::rstest]
#[case(4)]
#[case(16)]
#[case(64)]
#[tokio::test]
// https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events#examples
async fn test_data_only_messages(#[case] chunk_size: usize) {
    check(
        include_bytes!("../examples/data_only_messages.txt").chunks(chunk_size),
        &[
            Event {
                event: None,
                data: Some("some text".to_owned()),
                id: None,
                retry: None,
            },
            Event {
                event: None,
                data: Some("another message\nwith two lines".to_owned()),
                id: None,
                retry: None,
            },
        ],
    )
    .await;
}

#[rstest::rstest]
#[case(4)]
#[case(16)]
#[case(64)]
#[tokio::test]
// https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events#examples
async fn test_mixing_and_matching(#[case] chunk_size: usize) {
    check(
        include_bytes!("../examples/mixing_and_matching.txt").chunks(chunk_size),
        &[
            Event {
                event: Some("userconnect".to_owned()),
                data: Some(r#"{"username": "bobby", "time": "02:33:48"}"#.into()),
                id: None,
                retry: None,
            },
            Event {
                event: None,
                data: Some(
                    concat!(
                        "Here's a system message of some kind that will get used\n",
                        "to accomplish some task."
                    )
                    .to_owned(),
                ),
                id: None,
                retry: None,
            },
            Event {
                event: Some("usermessage".to_owned()),
                data:
                    Some(
                        concat!(
                            r#"{"username": "bobby", "time": "02:34:11", "text": "Hi everyone."}"#,
                        )
                        .to_owned(),
                    ),
                id: None,
                retry: None,
            },
        ],
    )
    .await;
}
