use crate::Event;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use std::future;
use std::pin::Pin;
use std::str::Utf8Error;
use std::time::Duration;

async fn check<E>(body: &'static [u8], events: E)
where
    E: IntoIterator<Item = Event>,
{
    let body = Full::<Bytes>::from(body).map_err(|_| -> Utf8Error { unreachable!() });
    let mut decode = super::decode(body);
    for event in events {
        assert_eq!(
            future::poll_fn(|cx| Pin::new(&mut decode).poll_frame(cx))
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap(),
            event,
        )
    }
    assert!(future::poll_fn(|cx| Pin::new(&mut decode).poll_frame(cx))
        .await
        .is_none())
}

#[futures_test::test]
async fn test_all_fields() {
    check(
        include_bytes!("../examples/all_fields.txt"),
        [
            Event {
                event: Some("event".to_owned()),
                data: Some("data".to_owned()),
                id: Some("id".to_owned()),
                retry: Some(Duration::from_secs(42)),
            },
            Event {
                event: Some("foo".to_owned()),
                ..Event::default()
            },
            Event {
                data: Some("bar".to_owned()),
                ..Event::default()
            },
            Event {
                id: Some("baz".to_owned()),
                ..Event::default()
            },
            Event {
                retry: Some(Duration::from_secs(57)),
                ..Event::default()
            },
        ],
    )
    .await;
}

#[futures_test::test]
async fn test_html_living_standard_0() {
    check(
        include_bytes!("../examples/html_living_standard_0.txt"),
        [Event {
            data: Some("YHOO\n+2\n10".to_owned()),
            ..Event::default()
        }],
    )
    .await;
}

#[futures_test::test]
async fn test_html_living_standard_1() {
    check(
        include_bytes!("../examples/html_living_standard_1.txt"),
        [
            Event {
                data: Some("first event".to_owned()),
                id: Some("1".to_owned()),
                ..Event::default()
            },
            Event {
                data: Some("second event".to_owned()),
                id: Some("".to_owned()),
                ..Event::default()
            },
            Event {
                data: Some(" third event".to_owned()),
                ..Event::default()
            },
        ],
    )
    .await;
}

#[futures_test::test]
async fn test_html_living_standard_2() {
    check(
        include_bytes!("../examples/html_living_standard_2.txt"),
        [
            Event {
                data: Some("".to_owned()),
                ..Event::default()
            },
            Event {
                data: Some("\n".to_owned()),
                ..Event::default()
            },
        ],
    )
    .await;
}

#[futures_test::test]
async fn test_data_only_messages() {
    check(
        include_bytes!("../examples/data_only_messages.txt"),
        [
            Event {
                event: None,
                data: Some("some text".to_owned()),
                ..Event::default()
            },
            Event {
                event: None,
                data: Some("another message\nwith two lines".to_owned()),
                ..Event::default()
            },
        ],
    )
    .await;
}

#[futures_test::test]
async fn test_mixing_and_matching() {
    check(
        include_bytes!("../examples/mixing_and_matching.txt"),
        [
            Event {
                event: Some("userconnect".to_owned()),
                data: Some(r#"{"username": "bobby", "time": "02:33:48"}"#.to_owned()),
                ..Event::default()
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
                ..Event::default()
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
                ..Event::default()
            },
        ],
    )
    .await;
}
