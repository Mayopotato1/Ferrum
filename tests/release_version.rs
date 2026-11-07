use spotlight_prototype::release_version::numeric;
#[test]
fn beta_windows_version_is_explicit_and_orders_future_betas_and_final() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.1.0-beta.2");
    assert_eq!(numeric(env!("CARGO_PKG_VERSION")).unwrap(), [0, 1, 0, 2]);
    assert!(numeric("0.1.0-beta.2").unwrap() > numeric("0.1.0-beta.1").unwrap());
    assert!(numeric("0.1.0").unwrap() > numeric("0.1.0-beta.65534").unwrap());
    for value in [
        "0.1.0-beta.0",
        "0.1.0-beta.65535",
        "0.1.0-alpha.1",
        "0.1",
        "0.1.65536",
    ] {
        assert!(numeric(value).is_err());
    }
}
