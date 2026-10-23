use selfcast_core::account::{Account, ParseAccountError, Username};

fn name(s: &str) -> Username {
    s.parse().expect("a username")
}

#[test]
fn usernames_follow_the_fname_pattern_and_end_in_eth() {
    let good = [
        "v",
        "dwr",
        "0x",
        "a-b",
        "123",
        "sixteen-chars-ok",
        "vitalik.eth",
        "jesse.base.eth",
    ];
    let bad = [
        "",
        "Dwr",
        "-dwr",
        "d_wr",
        "dwr.",
        ".eth",
        "dwr.com",
        "dwr..eth",
        "seventeen-chars-x",
        "dwr eth",
        "@dwr",
        "dwr.ETH",
    ];
    good.iter()
        .for_each(|s| assert!(s.parse::<Username>().is_ok(), "{s:?} refused"));
    bad.iter()
        .for_each(|s| assert!(s.parse::<Username>().is_err(), "{s:?} accepted"));
}

#[test]
fn digits_are_a_fid_and_an_at_makes_them_a_name() {
    assert_eq!("3".parse(), Ok(Account::Fid(3)));
    assert_eq!("@3".parse(), Ok(Account::Name(name("3"))));
    assert_eq!("dwr".parse(), Ok(Account::Name(name("dwr"))));
    assert_eq!("@dwr".parse(), Ok(Account::Name(name("dwr"))));
    assert!("@@dwr".parse::<Account>().is_err());
    assert!("@".parse::<Account>().is_err());
    assert!("".parse::<Account>().is_err());
    assert_eq!(
        "99999999999999999999".parse::<Account>(),
        Err(ParseAccountError::BadFid("99999999999999999999".to_owned()))
    );
}
