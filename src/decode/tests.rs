use crate::Event;
use bytes::Bytes;
use futures_test::stream::StreamTestExt;
use http_body::Frame;
use http_body_util::StreamBody;
use std::future;
use std::pin;
use std::str::Utf8Error;

async fn check<'a, D, E>(data: D, events_expected: E)
where
    D: IntoIterator<Item = &'a [u8]>,
    E: IntoIterator<Item = Event>,
{
    let body = StreamBody::new(
        futures::stream::iter(
            data.into_iter()
                .map(Bytes::copy_from_slice)
                .map(Frame::data)
                .map(Ok::<_, Utf8Error>),
        )
        .interleave_pending(),
    );
    let mut events_actual = pin::pin!(super::decode(body));
    for event_expected in events_expected {
        assert_eq!(
            future::poll_fn(|cx| events_actual.as_mut().poll_frame(cx))
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap(),
            event_expected,
        )
    }
    assert!(future::poll_fn(|cx| events_actual.as_mut().poll_frame(cx))
        .await
        .is_none())
}

#[rstest::rstest]
#[case(4)]
#[case(16)]
#[case(64)]
#[futures_test::test]
async fn test_data_only_messages(#[case] chunk_size: usize) {
    check(
        include_bytes!("../examples/data_only_messages.txt").chunks(chunk_size),
        [
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
#[futures_test::test]
async fn test_mixing_and_matching(#[case] chunk_size: usize) {
    check(
        include_bytes!("../examples/mixing_and_matching.txt").chunks(chunk_size),
        [
            Event {
                event: Some("userconnect".to_owned()),
                data: Some(r#"{"username": "bobby", "time": "02:33:48"}"#.to_owned()),
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
