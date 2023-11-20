use lazy_static::lazy_static;
use mp_felt::Felt252Wrapper;

pub const ACCOUNT_PUBLIC_KEY: &str = "0x03603a2692a2ae60abb343e832ee53b55d6b25f02a3ef1565ec691edc7a209b2";
pub const ARGENT_ACCOUNT_CLASS_HASH_CAIRO_0: &str =
    "0x06f0d6f6ae72e1a507ff4b65181291642889742dbf8f1a53e9ec1c595d01ba7d";
pub const BRAAVOS_ACCOUNT_CLASS_HASH_CAIRO_0: &str =
    "0x0244ca3d9fe8b47dd565a6f4270d979ba31a7d6ff2c3bf8776198161505e8b52";
pub const BRAAVOS_PROXY_CLASS_HASH_CAIRO_0: &str = "0x06a89ae7bd72c96202c040341c1ee422474b562e1d73c6848f08cae429c33262";
pub const OPENZEPPELIN_ACCOUNT_CLASS_HASH_CAIRO_0: &str =
    "0x006280083f8c2a2db9f737320d5e3029b380e0e820fe24b8d312a6a34fdba0cd";
pub const NO_VALIDATE_ACCOUNT_CLASS_HASH_CAIRO_0: &str =
    "0x0279d77db761fba82e0054125a6fdb5f6baa6286fa3fb73450cc44d193c2d37f";
pub const NO_VALIDATE_ACCOUNT_CLASS_HASH_CAIRO_1: &str =
    "0x35ccefcf9d5656da623468e27e682271cd327af196785df99e7fee1436b6276";
pub const TOKEN_CONTRACT_CLASS_HASH: &str = "0x06232eeb9ecb5de85fc927599f144913bfee6ac413f2482668c9f03ce4d07922";
pub const UNAUTHORIZED_INNER_CALL_ACCOUNT_CLASS_HASH_CAIRO_0: &str =
    "0x071aaf68d30c3e52e1c4b7d1209b0e09525939c31bb0275919dffd4cd53f57c4";

// salts for address calculation
lazy_static! {
    pub static ref SALT: Felt252Wrapper =
        Felt252Wrapper::from_hex_be("0x03b37cbe4e9eac89d54c5f7cc6329a63a63e8c8db2bf936f981041e086752463").unwrap();
    pub static ref TEST_ACCOUNT_SALT: Felt252Wrapper =
        Felt252Wrapper::from_hex_be("0x0780f72e33c1508df24d8f00a96ecc6e08a850ecb09f7e6dff6a81624c0ef46a").unwrap();
}
