use super::*;
#[test]
fn descriptionAndValidationNeverCreateQrOrNetworkRequests() {
    let (store, _) = crate::sessionStore::testing::memoryStore();
    let mut provider = Provider::withStore(store);
    let budget = Duration::from_secs(1);
    let description = provider
        .handleControlled(Operation::Describe, budget, &|| false)
        .unwrap();
    let ResultData::Description(info) = description else {
        panic!()
    };
    assert_eq!(info.id, "netease");
    assert!(info.supports(&Capability::Resolve));
    assert!(provider.Client.is_none());
    assert!(provider.Pending.is_none());
    assert_eq!(
        provider
            .handleControlled(
                Operation::Search {
                    query: "".into(),
                    page: 0
                },
                budget,
                &|| false
            )
            .err()
            .unwrap()
            .code,
        "PARAMETERS"
    );
    assert_eq!(
        provider
            .handleControlled(Operation::Account, budget, &|| false)
            .err()
            .unwrap()
            .code,
        "LOGIN_REQUIRED"
    );
    provider
        .handleControlled(Operation::Logout, budget, &|| false)
        .unwrap();
    assert!(provider.Pending.is_none());
    assert!(provider.Session.is_none());
}
