use super::*;
use serde_json::json;

#[test]
fn fixedEapiAndWeapiVectorsMatchIndependentDotNetAndPython() {
    let value = json!({"id":123});
    assert_eq!(
        eapi("/api/test", &value).unwrap(),
        "4DC723619A991588865191FD2F319BADF6E6D03161D8DDB0FC57F830088940CAE5D34ED316D10479EBC2BF22C1978D0573B69F50C31BBE660B691E60EE0441C92F17FC26AEC7BBB0A67339ABDCAA9997"
    );
    let (params, key) = weapiWithKey(&value, b"abcdefghijklmnop").unwrap();
    assert_eq!(params, "ugrUdawIejHiEX1nhsPRxaRVzfD0VOUIeBSqjscBUwI=");
    assert_eq!(
        key,
        "d15a1683c992095d0c234c19966605c5c5964911268bbeda8cb8d08d834913e59d53b32358903a121b5fca784c1f5ae44951fd02524df58ecc98e52cc7cf8689b42c2e93ddf05b0592512d87f5960467e2f086c018849d76014d323500e30f13ef4cafbb0cf5a66731a3f1776c75ca35d0062dac70a3e33245afabcf47938487"
    );
}
#[test]
fn ecbSupportsBothProtocolKeySizesAndRejectsInvalidPadding() {
    for key in [eapiKey.as_slice(), staticKey.as_slice()] {
        for size in [0, 1, 16, 256] {
            let bytes = vec![73; size];
            assert_eq!(
                ecb(key, &ecb(key, &bytes, true).unwrap(), false).unwrap(),
                bytes
            );
        }
        assert!(ecb(key, &[0; 16], false).is_err());
        assert!(ecb(key, &[1; 17], false).is_err());
    }
}
