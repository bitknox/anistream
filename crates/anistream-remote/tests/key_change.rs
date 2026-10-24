//! A TV reset since a phone paired answers with a new key: the phone's link says so and stops,
//! rather than retrying a key it will never accept.
#![cfg(feature = "testing")]

use std::time::Duration;

use anistream_remote::{
    Error,
    client::{Hello, Link, LinkOptions, LinkStatus, RemoteClient},
    fake::FakeTv,
    identity::Identity,
};

#[tokio::test]
async fn a_tv_with_a_new_key_is_refused_and_the_link_stops() {
    let tv = FakeTv::start("Den", Vec::new()).await.unwrap();
    let phone = Identity::generate().unwrap();
    let hello = Hello { name: "Hall phone".into(), app: "test/0".into() };
    // Pinned to some other TV's key, as a phone paired before the reset is.
    let old = Identity::generate().unwrap().fingerprint();
    let refused = RemoteClient::connect(tv.addr(), &phone, Some(old), &hello).await;
    assert!(matches!(refused, Err(Error::KeyChanged)), "{:?}", refused.err());

    let link = Link::open(tv.addr(), phone, old, hello, LinkOptions::default());
    let mut status = link.status();
    let stopped = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if *status.borrow_and_update() == LinkStatus::KeyChanged {
                return;
            }
            status.changed().await.unwrap();
        }
    });
    stopped.await.expect("the link says the key changed");
}
