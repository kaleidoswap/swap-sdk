// AUTO-GENERATED FILE — DO NOT EDIT MANUALLY.
//
// RGB Lightning Node (RLN) types, generated from specs/rgb-lightning-node.yaml
// via typify. Re-generate with: make generate-rln-types

#![allow(clippy::redundant_closure_call)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::match_single_binding)]
#![allow(clippy::clone_on_copy)]

#[doc = r" Error types."]
pub mod error {
    #[doc = r" Error from a `TryFrom` or `FromStr` implementation."]
    pub struct ConversionError(::std::borrow::Cow<'static, str>);
    impl ::std::error::Error for ConversionError {}
    impl ::std::fmt::Display for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Display::fmt(&self.0, f)
        }
    }
    impl ::std::fmt::Debug for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Debug::fmt(&self.0, f)
        }
    }
    impl From<&'static str> for ConversionError {
        fn from(value: &'static str) -> Self {
            Self(value.into())
        }
    }
    impl From<String> for ConversionError {
        fn from(value: String) -> Self {
            Self(value.into())
        }
    }
}
#[doc = "`AddressResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"address\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"address\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"bcrt1qnc5y6j6dmejrkwy93farhvpezk0lf46gk7aecs\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AddressResponse {
    pub address: ::std::string::String,
}
impl AddressResponse {
    pub fn builder() -> builder::AddressResponse {
        Default::default()
    }
}
#[doc = "`AssetBalanceRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"asset_id\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AssetBalanceRequest {
    pub asset_id: ::std::string::String,
}
impl AssetBalanceRequest {
    pub fn builder() -> builder::AssetBalanceRequest {
        Default::default()
    }
}
#[doc = "`AssetBalanceResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"future\","]
#[doc = "    \"offchain_inbound\","]
#[doc = "    \"offchain_outbound\","]
#[doc = "    \"settled\","]
#[doc = "    \"spendable\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"future\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 777"]
#[doc = "    },"]
#[doc = "    \"offchain_inbound\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 0"]
#[doc = "    },"]
#[doc = "    \"offchain_outbound\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 444"]
#[doc = "    },"]
#[doc = "    \"settled\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 777"]
#[doc = "    },"]
#[doc = "    \"spendable\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 777"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AssetBalanceResponse {
    pub future: i64,
    pub offchain_inbound: i64,
    pub offchain_outbound: i64,
    pub settled: i64,
    pub spendable: i64,
}
impl AssetBalanceResponse {
    pub fn builder() -> builder::AssetBalanceResponse {
        Default::default()
    }
}
#[doc = "`AssetCfa`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"added_at\","]
#[doc = "    \"asset_id\","]
#[doc = "    \"balance\","]
#[doc = "    \"issued_supply\","]
#[doc = "    \"name\","]
#[doc = "    \"precision\","]
#[doc = "    \"timestamp\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"added_at\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691161979"]
#[doc = "    },"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"balance\": {"]
#[doc = "      \"$ref\": \"#/$defs/AssetBalanceResponse\""]
#[doc = "    },"]
#[doc = "    \"details\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"asset details\""]
#[doc = "    },"]
#[doc = "    \"issued_supply\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 777"]
#[doc = "    },"]
#[doc = "    \"media\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/Media\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"Collectible\""]
#[doc = "    },"]
#[doc = "    \"precision\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 0"]
#[doc = "    },"]
#[doc = "    \"timestamp\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691160565"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AssetCfa {
    pub added_at: i64,
    pub asset_id: ::std::string::String,
    pub balance: AssetBalanceResponse,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub details: ::std::option::Option<::std::string::String>,
    pub issued_supply: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub media: ::std::option::Option<Media>,
    pub name: ::std::string::String,
    pub precision: i64,
    pub timestamp: i64,
}
impl AssetCfa {
    pub fn builder() -> builder::AssetCfa {
        Default::default()
    }
}
#[doc = "`AssetIfa`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"added_at\","]
#[doc = "    \"asset_id\","]
#[doc = "    \"balance\","]
#[doc = "    \"initial_supply\","]
#[doc = "    \"known_circulating_supply\","]
#[doc = "    \"max_supply\","]
#[doc = "    \"name\","]
#[doc = "    \"precision\","]
#[doc = "    \"ticker\","]
#[doc = "    \"timestamp\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"added_at\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691161979"]
#[doc = "    },"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"balance\": {"]
#[doc = "      \"$ref\": \"#/$defs/AssetBalanceResponse\""]
#[doc = "    },"]
#[doc = "    \"details\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"asset details\""]
#[doc = "    },"]
#[doc = "    \"initial_supply\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 777"]
#[doc = "    },"]
#[doc = "    \"known_circulating_supply\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 888"]
#[doc = "    },"]
#[doc = "    \"max_supply\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 999"]
#[doc = "    },"]
#[doc = "    \"media\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/Media\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"Tether\""]
#[doc = "    },"]
#[doc = "    \"precision\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 0"]
#[doc = "    },"]
#[doc = "    \"reject_list_url\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"https://some.domain/someasset/rejectlist\""]
#[doc = "    },"]
#[doc = "    \"ticker\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"USDT\""]
#[doc = "    },"]
#[doc = "    \"timestamp\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691160565"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AssetIfa {
    pub added_at: i64,
    pub asset_id: ::std::string::String,
    pub balance: AssetBalanceResponse,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub details: ::std::option::Option<::std::string::String>,
    pub initial_supply: i64,
    pub known_circulating_supply: i64,
    pub max_supply: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub media: ::std::option::Option<Media>,
    pub name: ::std::string::String,
    pub precision: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub reject_list_url: ::std::option::Option<::std::string::String>,
    pub ticker: ::std::string::String,
    pub timestamp: i64,
}
impl AssetIfa {
    pub fn builder() -> builder::AssetIfa {
        Default::default()
    }
}
#[doc = "`AssetMetadataRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"asset_id\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AssetMetadataRequest {
    pub asset_id: ::std::string::String,
}
impl AssetMetadataRequest {
    pub fn builder() -> builder::AssetMetadataRequest {
        Default::default()
    }
}
#[doc = "`AssetMetadataResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"asset_schema\","]
#[doc = "    \"initial_supply\","]
#[doc = "    \"known_circulating_supply\","]
#[doc = "    \"max_supply\","]
#[doc = "    \"name\","]
#[doc = "    \"precision\","]
#[doc = "    \"timestamp\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset_schema\": {"]
#[doc = "      \"$ref\": \"#/$defs/AssetSchema\""]
#[doc = "    },"]
#[doc = "    \"details\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"asset details\""]
#[doc = "    },"]
#[doc = "    \"initial_supply\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 777"]
#[doc = "    },"]
#[doc = "    \"known_circulating_supply\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 777"]
#[doc = "    },"]
#[doc = "    \"max_supply\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 777"]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"Collectible\""]
#[doc = "    },"]
#[doc = "    \"precision\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 0"]
#[doc = "    },"]
#[doc = "    \"ticker\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"USDT\""]
#[doc = "    },"]
#[doc = "    \"timestamp\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691160565"]
#[doc = "    },"]
#[doc = "    \"token\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/Token\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AssetMetadataResponse {
    pub asset_schema: AssetSchema,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub details: ::std::option::Option<::std::string::String>,
    pub initial_supply: i64,
    pub known_circulating_supply: i64,
    pub max_supply: i64,
    pub name: ::std::string::String,
    pub precision: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub ticker: ::std::option::Option<::std::string::String>,
    pub timestamp: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub token: ::std::option::Option<Token>,
}
impl AssetMetadataResponse {
    pub fn builder() -> builder::AssetMetadataResponse {
        Default::default()
    }
}
#[doc = "`AssetNia`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"added_at\","]
#[doc = "    \"asset_id\","]
#[doc = "    \"balance\","]
#[doc = "    \"issued_supply\","]
#[doc = "    \"name\","]
#[doc = "    \"precision\","]
#[doc = "    \"ticker\","]
#[doc = "    \"timestamp\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"added_at\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691161979"]
#[doc = "    },"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"balance\": {"]
#[doc = "      \"$ref\": \"#/$defs/AssetBalanceResponse\""]
#[doc = "    },"]
#[doc = "    \"details\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"asset details\""]
#[doc = "    },"]
#[doc = "    \"issued_supply\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 777"]
#[doc = "    },"]
#[doc = "    \"media\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/Media\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"Tether\""]
#[doc = "    },"]
#[doc = "    \"precision\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 0"]
#[doc = "    },"]
#[doc = "    \"ticker\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"USDT\""]
#[doc = "    },"]
#[doc = "    \"timestamp\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691160565"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AssetNia {
    pub added_at: i64,
    pub asset_id: ::std::string::String,
    pub balance: AssetBalanceResponse,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub details: ::std::option::Option<::std::string::String>,
    pub issued_supply: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub media: ::std::option::Option<Media>,
    pub name: ::std::string::String,
    pub precision: i64,
    pub ticker: ::std::string::String,
    pub timestamp: i64,
}
impl AssetNia {
    pub fn builder() -> builder::AssetNia {
        Default::default()
    }
}
#[doc = "`AssetSchema`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"Nia\","]
#[doc = "    \"Uda\","]
#[doc = "    \"Cfa\","]
#[doc = "    \"Ifa\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum AssetSchema {
    Nia,
    Uda,
    Cfa,
    Ifa,
}
impl ::std::fmt::Display for AssetSchema {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Nia => f.write_str("Nia"),
            Self::Uda => f.write_str("Uda"),
            Self::Cfa => f.write_str("Cfa"),
            Self::Ifa => f.write_str("Ifa"),
        }
    }
}
impl ::std::str::FromStr for AssetSchema {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Nia" => Ok(Self::Nia),
            "Uda" => Ok(Self::Uda),
            "Cfa" => Ok(Self::Cfa),
            "Ifa" => Ok(Self::Ifa),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for AssetSchema {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for AssetSchema {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for AssetSchema {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`AssetUda`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"added_at\","]
#[doc = "    \"asset_id\","]
#[doc = "    \"balance\","]
#[doc = "    \"name\","]
#[doc = "    \"precision\","]
#[doc = "    \"ticker\","]
#[doc = "    \"timestamp\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"added_at\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691161979"]
#[doc = "    },"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"balance\": {"]
#[doc = "      \"$ref\": \"#/$defs/AssetBalanceResponse\""]
#[doc = "    },"]
#[doc = "    \"details\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"asset details\""]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"Unique\""]
#[doc = "    },"]
#[doc = "    \"precision\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 0"]
#[doc = "    },"]
#[doc = "    \"ticker\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"UNI\""]
#[doc = "    },"]
#[doc = "    \"timestamp\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691160565"]
#[doc = "    },"]
#[doc = "    \"token\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/TokenLight\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AssetUda {
    pub added_at: i64,
    pub asset_id: ::std::string::String,
    pub balance: AssetBalanceResponse,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub details: ::std::option::Option<::std::string::String>,
    pub name: ::std::string::String,
    pub precision: i64,
    pub ticker: ::std::string::String,
    pub timestamp: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub token: ::std::option::Option<TokenLight>,
}
impl AssetUda {
    pub fn builder() -> builder::AssetUda {
        Default::default()
    }
}
#[doc = "`Assignment`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"oneOf\": ["]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/AssignmentFungible\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/AssignmentNonFungible\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/AssignmentInflationRight\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/AssignmentAny\""]
#[doc = "    }"]
#[doc = "  ],"]
#[doc = "  \"discriminator\": {"]
#[doc = "    \"mapping\": {"]
#[doc = "      \"Any\": \"#/components/schemas/AssignmentAny\","]
#[doc = "      \"Fungible\": \"#/components/schemas/AssignmentFungible\","]
#[doc = "      \"InflationRight\": \"#/components/schemas/AssignmentInflationRight\","]
#[doc = "      \"NonFungible\": \"#/components/schemas/AssignmentNonFungible\""]
#[doc = "    },"]
#[doc = "    \"propertyName\": \"type\""]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum Assignment {
    Fungible(AssignmentFungible),
    NonFungible(AssignmentNonFungible),
    InflationRight(AssignmentInflationRight),
    Any(AssignmentAny),
}
impl ::std::convert::From<AssignmentFungible> for Assignment {
    fn from(value: AssignmentFungible) -> Self {
        Self::Fungible(value)
    }
}
impl ::std::convert::From<AssignmentNonFungible> for Assignment {
    fn from(value: AssignmentNonFungible) -> Self {
        Self::NonFungible(value)
    }
}
impl ::std::convert::From<AssignmentInflationRight> for Assignment {
    fn from(value: AssignmentInflationRight) -> Self {
        Self::InflationRight(value)
    }
}
impl ::std::convert::From<AssignmentAny> for Assignment {
    fn from(value: AssignmentAny) -> Self {
        Self::Any(value)
    }
}
#[doc = "`AssignmentAny`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"type\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"type\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"enum\": ["]
#[doc = "        \"Any\""]
#[doc = "      ]"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AssignmentAny {
    #[serde(rename = "type")]
    pub type_: AssignmentAnyType,
}
impl AssignmentAny {
    pub fn builder() -> builder::AssignmentAny {
        Default::default()
    }
}
#[doc = "`AssignmentAnyType`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"Any\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum AssignmentAnyType {
    Any,
}
impl ::std::fmt::Display for AssignmentAnyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Any => f.write_str("Any"),
        }
    }
}
impl ::std::str::FromStr for AssignmentAnyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Any" => Ok(Self::Any),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for AssignmentAnyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for AssignmentAnyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for AssignmentAnyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`AssignmentFungible`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"type\","]
#[doc = "    \"value\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"type\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"enum\": ["]
#[doc = "        \"Fungible\""]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"value\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 42"]
#[doc = "    }"]
#[doc = "  },"]
#[doc = "  \"example\": {"]
#[doc = "    \"type\": \"Fungible\","]
#[doc = "    \"value\": 42"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AssignmentFungible {
    #[serde(rename = "type")]
    pub type_: AssignmentFungibleType,
    pub value: i64,
}
impl AssignmentFungible {
    pub fn builder() -> builder::AssignmentFungible {
        Default::default()
    }
}
#[doc = "`AssignmentFungibleType`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"Fungible\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum AssignmentFungibleType {
    Fungible,
}
impl ::std::fmt::Display for AssignmentFungibleType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Fungible => f.write_str("Fungible"),
        }
    }
}
impl ::std::str::FromStr for AssignmentFungibleType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Fungible" => Ok(Self::Fungible),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for AssignmentFungibleType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for AssignmentFungibleType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for AssignmentFungibleType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`AssignmentInflationRight`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"type\","]
#[doc = "    \"value\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"type\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"enum\": ["]
#[doc = "        \"InflationRight\""]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"value\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 200"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AssignmentInflationRight {
    #[serde(rename = "type")]
    pub type_: AssignmentInflationRightType,
    pub value: i64,
}
impl AssignmentInflationRight {
    pub fn builder() -> builder::AssignmentInflationRight {
        Default::default()
    }
}
#[doc = "`AssignmentInflationRightType`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"InflationRight\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum AssignmentInflationRightType {
    InflationRight,
}
impl ::std::fmt::Display for AssignmentInflationRightType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::InflationRight => f.write_str("InflationRight"),
        }
    }
}
impl ::std::str::FromStr for AssignmentInflationRightType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "InflationRight" => Ok(Self::InflationRight),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for AssignmentInflationRightType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for AssignmentInflationRightType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for AssignmentInflationRightType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`AssignmentNonFungible`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"type\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"type\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"enum\": ["]
#[doc = "        \"NonFungible\""]
#[doc = "      ]"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AssignmentNonFungible {
    #[serde(rename = "type")]
    pub type_: AssignmentNonFungibleType,
}
impl AssignmentNonFungible {
    pub fn builder() -> builder::AssignmentNonFungible {
        Default::default()
    }
}
#[doc = "`AssignmentNonFungibleType`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"NonFungible\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum AssignmentNonFungibleType {
    NonFungible,
}
impl ::std::fmt::Display for AssignmentNonFungibleType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::NonFungible => f.write_str("NonFungible"),
        }
    }
}
impl ::std::str::FromStr for AssignmentNonFungibleType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "NonFungible" => Ok(Self::NonFungible),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for AssignmentNonFungibleType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for AssignmentNonFungibleType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for AssignmentNonFungibleType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`BackupRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"backup_path\","]
#[doc = "    \"password\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"backup_path\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"/path/where/to/save/the/backup/file\""]
#[doc = "    },"]
#[doc = "    \"password\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"nodepassword\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct BackupRequest {
    pub backup_path: ::std::string::String,
    pub password: ::std::string::String,
}
impl BackupRequest {
    pub fn builder() -> builder::BackupRequest {
        Default::default()
    }
}
#[doc = "`BitcoinNetwork`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"Mainnet\","]
#[doc = "    \"Testnet\","]
#[doc = "    \"Testnet4\","]
#[doc = "    \"Signet\","]
#[doc = "    \"SignetCustom\","]
#[doc = "    \"Regtest\""]
#[doc = "  ],"]
#[doc = "  \"example\": \"Regtest\""]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum BitcoinNetwork {
    Mainnet,
    Testnet,
    Testnet4,
    Signet,
    SignetCustom,
    Regtest,
}
impl ::std::fmt::Display for BitcoinNetwork {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Mainnet => f.write_str("Mainnet"),
            Self::Testnet => f.write_str("Testnet"),
            Self::Testnet4 => f.write_str("Testnet4"),
            Self::Signet => f.write_str("Signet"),
            Self::SignetCustom => f.write_str("SignetCustom"),
            Self::Regtest => f.write_str("Regtest"),
        }
    }
}
impl ::std::str::FromStr for BitcoinNetwork {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Mainnet" => Ok(Self::Mainnet),
            "Testnet" => Ok(Self::Testnet),
            "Testnet4" => Ok(Self::Testnet4),
            "Signet" => Ok(Self::Signet),
            "SignetCustom" => Ok(Self::SignetCustom),
            "Regtest" => Ok(Self::Regtest),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for BitcoinNetwork {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for BitcoinNetwork {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BitcoinNetwork {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`BlockTime`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"height\","]
#[doc = "    \"timestamp\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"height\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 805434"]
#[doc = "    },"]
#[doc = "    \"timestamp\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691160659"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct BlockTime {
    pub height: i64,
    pub timestamp: i64,
}
impl BlockTime {
    pub fn builder() -> builder::BlockTime {
        Default::default()
    }
}
#[doc = "`BtcBalance`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"future\","]
#[doc = "    \"settled\","]
#[doc = "    \"spendable\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"future\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 777000"]
#[doc = "    },"]
#[doc = "    \"settled\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 777000"]
#[doc = "    },"]
#[doc = "    \"spendable\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 777000"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct BtcBalance {
    pub future: i64,
    pub settled: i64,
    pub spendable: i64,
}
impl BtcBalance {
    pub fn builder() -> builder::BtcBalance {
        Default::default()
    }
}
#[doc = "`BtcBalanceRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"skip_sync\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"skip_sync\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct BtcBalanceRequest {
    pub skip_sync: bool,
}
impl BtcBalanceRequest {
    pub fn builder() -> builder::BtcBalanceRequest {
        Default::default()
    }
}
#[doc = "`BtcBalanceResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"colored\","]
#[doc = "    \"vanilla\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"colored\": {"]
#[doc = "      \"$ref\": \"#/$defs/BtcBalance\""]
#[doc = "    },"]
#[doc = "    \"vanilla\": {"]
#[doc = "      \"$ref\": \"#/$defs/BtcBalance\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct BtcBalanceResponse {
    pub colored: BtcBalance,
    pub vanilla: BtcBalance,
}
impl BtcBalanceResponse {
    pub fn builder() -> builder::BtcBalanceResponse {
        Default::default()
    }
}
#[doc = "`ChangePasswordRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"new_password\","]
#[doc = "    \"old_password\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"new_password\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"nodenewpassword\""]
#[doc = "    },"]
#[doc = "    \"old_password\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"nodepassword\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ChangePasswordRequest {
    pub new_password: ::std::string::String,
    pub old_password: ::std::string::String,
}
impl ChangePasswordRequest {
    pub fn builder() -> builder::ChangePasswordRequest {
        Default::default()
    }
}
#[doc = "`Channel`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"capacity_sat\","]
#[doc = "    \"channel_id\","]
#[doc = "    \"inbound_balance_msat\","]
#[doc = "    \"is_usable\","]
#[doc = "    \"local_balance_sat\","]
#[doc = "    \"next_outbound_htlc_limit_msat\","]
#[doc = "    \"next_outbound_htlc_minimum_msat\","]
#[doc = "    \"outbound_balance_msat\","]
#[doc = "    \"peer_pubkey\","]
#[doc = "    \"public\","]
#[doc = "    \"ready\","]
#[doc = "    \"status\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"asset_local_amount\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 777"]
#[doc = "    },"]
#[doc = "    \"asset_remote_amount\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 0"]
#[doc = "    },"]
#[doc = "    \"capacity_sat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 30010"]
#[doc = "    },"]
#[doc = "    \"channel_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"8129afe1b1d7cf60d5e1bf4c04b09bec925ed4df5417ceee0484e24f816a105a\""]
#[doc = "    },"]
#[doc = "    \"funding_txid\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"5a106a814fe28404eece1754dfd45e92ec9bb0044cbfe1d560cfd7b1e1af2981\""]
#[doc = "    },"]
#[doc = "    \"inbound_balance_msat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 6394000"]
#[doc = "    },"]
#[doc = "    \"is_usable\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    },"]
#[doc = "    \"local_balance_sat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 28616"]
#[doc = "    },"]
#[doc = "    \"next_outbound_htlc_limit_msat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 3001000"]
#[doc = "    },"]
#[doc = "    \"next_outbound_htlc_minimum_msat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1"]
#[doc = "    },"]
#[doc = "    \"outbound_balance_msat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 21616000"]
#[doc = "    },"]
#[doc = "    \"peer_alias\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": null"]
#[doc = "    },"]
#[doc = "    \"peer_pubkey\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"03b79a4bc1ec365524b4fab9a39eb133753646babb5a1da5c4bc94c53110b7795d\""]
#[doc = "    },"]
#[doc = "    \"public\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": true"]
#[doc = "    },"]
#[doc = "    \"ready\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    },"]
#[doc = "    \"short_channel_id\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 120946279120896"]
#[doc = "    },"]
#[doc = "    \"status\": {"]
#[doc = "      \"$ref\": \"#/$defs/ChannelStatus\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Channel {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_id: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_local_amount: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_remote_amount: ::std::option::Option<i64>,
    pub capacity_sat: i64,
    pub channel_id: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub funding_txid: ::std::option::Option<::std::string::String>,
    pub inbound_balance_msat: i64,
    pub is_usable: bool,
    pub local_balance_sat: i64,
    pub next_outbound_htlc_limit_msat: i64,
    pub next_outbound_htlc_minimum_msat: i64,
    pub outbound_balance_msat: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub peer_alias: ::std::option::Option<::std::string::String>,
    pub peer_pubkey: ::std::string::String,
    pub public: bool,
    pub ready: bool,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub short_channel_id: ::std::option::Option<i64>,
    pub status: ChannelStatus,
}
impl Channel {
    pub fn builder() -> builder::Channel {
        Default::default()
    }
}
#[doc = "`ChannelStatus`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"Opening\","]
#[doc = "    \"Opened\","]
#[doc = "    \"Closing\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ChannelStatus {
    Opening,
    Opened,
    Closing,
}
impl ::std::fmt::Display for ChannelStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Opening => f.write_str("Opening"),
            Self::Opened => f.write_str("Opened"),
            Self::Closing => f.write_str("Closing"),
        }
    }
}
impl ::std::str::FromStr for ChannelStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Opening" => Ok(Self::Opening),
            "Opened" => Ok(Self::Opened),
            "Closing" => Ok(Self::Closing),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ChannelStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for ChannelStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ChannelStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`CheckIndexerUrlRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"indexer_url\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"indexer_url\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"127.0.0.1:50001\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct CheckIndexerUrlRequest {
    pub indexer_url: ::std::string::String,
}
impl CheckIndexerUrlRequest {
    pub fn builder() -> builder::CheckIndexerUrlRequest {
        Default::default()
    }
}
#[doc = "`CheckIndexerUrlResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"indexer_protocol\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"indexer_protocol\": {"]
#[doc = "      \"$ref\": \"#/$defs/IndexerProtocol\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct CheckIndexerUrlResponse {
    pub indexer_protocol: IndexerProtocol,
}
impl CheckIndexerUrlResponse {
    pub fn builder() -> builder::CheckIndexerUrlResponse {
        Default::default()
    }
}
#[doc = "`CheckProxyEndpointRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"proxy_endpoint\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"proxy_endpoint\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"rpc://127.0.0.1:3000/json-rpc\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct CheckProxyEndpointRequest {
    pub proxy_endpoint: ::std::string::String,
}
impl CheckProxyEndpointRequest {
    pub fn builder() -> builder::CheckProxyEndpointRequest {
        Default::default()
    }
}
#[doc = "`CloseChannelRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"channel_id\","]
#[doc = "    \"force\","]
#[doc = "    \"peer_pubkey\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"channel_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"8129afe1b1d7cf60d5e1bf4c04b09bec925ed4df5417ceee0484e24f816a105a\""]
#[doc = "    },"]
#[doc = "    \"force\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    },"]
#[doc = "    \"peer_pubkey\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"03b79a4bc1ec365524b4fab9a39eb133753646babb5a1da5c4bc94c53110b7795d\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct CloseChannelRequest {
    pub channel_id: ::std::string::String,
    pub force: bool,
    pub peer_pubkey: ::std::string::String,
}
impl CloseChannelRequest {
    pub fn builder() -> builder::CloseChannelRequest {
        Default::default()
    }
}
#[doc = "`ConnectPeerRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"peer_pubkey_and_addr\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"peer_pubkey_and_addr\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"03b79a4bc1ec365524b4fab9a39eb133753646babb5a1da5c4bc94c53110b7795d@localhost:9736\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ConnectPeerRequest {
    pub peer_pubkey_and_addr: ::std::string::String,
}
impl ConnectPeerRequest {
    pub fn builder() -> builder::ConnectPeerRequest {
        Default::default()
    }
}
#[doc = "`CreateUtxosRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"fee_rate\","]
#[doc = "    \"skip_sync\","]
#[doc = "    \"up_to\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"fee_rate\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 5"]
#[doc = "    },"]
#[doc = "    \"num\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 4"]
#[doc = "    },"]
#[doc = "    \"size\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 32500"]
#[doc = "    },"]
#[doc = "    \"skip_sync\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    },"]
#[doc = "    \"up_to\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct CreateUtxosRequest {
    pub fee_rate: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub num: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub size: ::std::option::Option<i64>,
    pub skip_sync: bool,
    pub up_to: bool,
}
impl CreateUtxosRequest {
    pub fn builder() -> builder::CreateUtxosRequest {
        Default::default()
    }
}
#[doc = "`DecodeLnInvoiceRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"invoice\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"invoice\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"lnbcrt30u1pjv6yzndqud3jxktt5w46x7unfv9kz6mn0v3jsnp4qdpc280eur52luxppv6f3nnj8l6vnd9g2hnv3qv6mjhmhvlzf6327pp5tjjasx6g9dqptea3fhm6yllq5wxzycnnvp8l6wcq3d6j2uvpryuqsp5l8az8x3g8fe05dg7cmgddld3da09nfjvky8xftwsk4cj8p2l7kfq9qyysgqcqpcxqzdylzlwfnkyw3jv344x4rzwgkk53ng0fhxy5rdduk4g5tpvea8xa6rfckkza35va28xjn2tqkhgarcxep5umm4x5k56wfcdvu95eq7qzp20vrl4xz76syapsa3c09j7lg5gerkaj63llj0ark7ph8hfketn6fkqzm8laf66dhsncm23wkwm5l5377we9e8lnlknnkwje5eefkccusqm6rqt8\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct DecodeLnInvoiceRequest {
    pub invoice: ::std::string::String,
}
impl DecodeLnInvoiceRequest {
    pub fn builder() -> builder::DecodeLnInvoiceRequest {
        Default::default()
    }
}
#[doc = "`DecodeLnInvoiceResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"expiry_sec\","]
#[doc = "    \"network\","]
#[doc = "    \"payment_hash\","]
#[doc = "    \"payment_secret\","]
#[doc = "    \"timestamp\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"amt_msat\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 3000000"]
#[doc = "    },"]
#[doc = "    \"asset_amount\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 42"]
#[doc = "    },"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"expiry_sec\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 420"]
#[doc = "    },"]
#[doc = "    \"network\": {"]
#[doc = "      \"$ref\": \"#/$defs/BitcoinNetwork\""]
#[doc = "    },"]
#[doc = "    \"payee_pubkey\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"0343851df9e0e8aff0c10b3498ce723ff4c9b4a855e6c8819adcafbbb3e24ea2af\""]
#[doc = "    },"]
#[doc = "    \"payment_hash\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"5ca5d81b482b4015e7b14df7a27fe0a38c226273604ffd3b008b752571811938\""]
#[doc = "    },"]
#[doc = "    \"payment_secret\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"f9fa239a283a72fa351ec6d0d6fdb16f5e59a64cb10e64add0b57123855ff592\""]
#[doc = "    },"]
#[doc = "    \"timestamp\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691160659"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct DecodeLnInvoiceResponse {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub amt_msat: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_amount: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_id: ::std::option::Option<::std::string::String>,
    pub expiry_sec: i64,
    pub network: BitcoinNetwork,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub payee_pubkey: ::std::option::Option<::std::string::String>,
    pub payment_hash: ::std::string::String,
    pub payment_secret: ::std::string::String,
    pub timestamp: i64,
}
impl DecodeLnInvoiceResponse {
    pub fn builder() -> builder::DecodeLnInvoiceResponse {
        Default::default()
    }
}
#[doc = "`DecodeRgbInvoiceRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"invoice\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"invoice\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"rgb:icfqnK9y-wObZKTu-XJcDL98-sKbE5Mh-OuDJhiI-brRJrzE/RWhwUfTMpuP2Zfx1~j4nswCANGeJrYOqDcKelaMV4zU/~/bcrt:utxob:cbgHUJ4e-7QyKY4U-Jsj5AZw-oI0gxZh-7fxQY2_-tFFUAZN-4CgpX?expiry=1749906951&endpoints=rpcs://proxy.iriswallet.com/0.2/json-rpc\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct DecodeRgbInvoiceRequest {
    pub invoice: ::std::string::String,
}
impl DecodeRgbInvoiceRequest {
    pub fn builder() -> builder::DecodeRgbInvoiceRequest {
        Default::default()
    }
}
#[doc = "`DecodeRgbInvoiceResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"assignment\","]
#[doc = "    \"network\","]
#[doc = "    \"recipient_id\","]
#[doc = "    \"recipient_type\","]
#[doc = "    \"transport_endpoints\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:icfqnK9y-wObZKTu-XJcDL98-sKbE5Mh-OuDJhiI-brRJrzE\""]
#[doc = "    },"]
#[doc = "    \"asset_schema\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/AssetSchema\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"assignment\": {"]
#[doc = "      \"$ref\": \"#/$defs/Assignment\""]
#[doc = "    },"]
#[doc = "    \"expiration_timestamp\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 1698325849"]
#[doc = "    },"]
#[doc = "    \"network\": {"]
#[doc = "      \"$ref\": \"#/$defs/BitcoinNetwork\""]
#[doc = "    },"]
#[doc = "    \"recipient_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"bcrt:utxob:cbgHUJ4e-7QyKY4U-Jsj5AZw-oI0gxZh-7fxQY2_-tFFUAZN-4CgpX\""]
#[doc = "    },"]
#[doc = "    \"recipient_type\": {"]
#[doc = "      \"$ref\": \"#/$defs/RecipientType\""]
#[doc = "    },"]
#[doc = "    \"transport_endpoints\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"string\","]
#[doc = "        \"example\": \"rpcs://proxy.iriswallet.com/0.2/json-rpc\""]
#[doc = "      }"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct DecodeRgbInvoiceResponse {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_id: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_schema: ::std::option::Option<AssetSchema>,
    pub assignment: Assignment,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub expiration_timestamp: ::std::option::Option<i64>,
    pub network: BitcoinNetwork,
    pub recipient_id: ::std::string::String,
    pub recipient_type: RecipientType,
    pub transport_endpoints: ::std::vec::Vec<::std::string::String>,
}
impl DecodeRgbInvoiceResponse {
    pub fn builder() -> builder::DecodeRgbInvoiceResponse {
        Default::default()
    }
}
#[doc = "`DecodeSwapstringRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"swapstring\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"swapstring\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"30/rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8/10/rgb:icfqnK9y-wObZKTu-XJcDL98-sKbE5Mh-OuDJhiI-brRJrzE/1715896416/9d342c6ba006e24abee84a2e034a22d5e30c1f2599fb9c3574d46d3cde3d65a2\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct DecodeSwapstringRequest {
    pub swapstring: ::std::string::String,
}
impl DecodeSwapstringRequest {
    pub fn builder() -> builder::DecodeSwapstringRequest {
        Default::default()
    }
}
#[doc = "`DecodeSwapstringResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"expiry\","]
#[doc = "    \"payment_hash\","]
#[doc = "    \"qty_from\","]
#[doc = "    \"qty_to\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"expiry\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1715896416"]
#[doc = "    },"]
#[doc = "    \"from_asset\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"payment_hash\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"9d342c6ba006e24abee84a2e034a22d5e30c1f2599fb9c3574d46d3cde3d65a2\""]
#[doc = "    },"]
#[doc = "    \"qty_from\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 30"]
#[doc = "    },"]
#[doc = "    \"qty_to\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 10"]
#[doc = "    },"]
#[doc = "    \"to_asset\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:icfqnK9y-wObZKTu-XJcDL98-sKbE5Mh-OuDJhiI-brRJrzE\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct DecodeSwapstringResponse {
    pub expiry: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub from_asset: ::std::option::Option<::std::string::String>,
    pub payment_hash: ::std::string::String,
    pub qty_from: i64,
    pub qty_to: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub to_asset: ::std::option::Option<::std::string::String>,
}
impl DecodeSwapstringResponse {
    pub fn builder() -> builder::DecodeSwapstringResponse {
        Default::default()
    }
}
#[doc = "`DisconnectPeerRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"peer_pubkey\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"peer_pubkey\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"03b79a4bc1ec365524b4fab9a39eb133753646babb5a1da5c4bc94c53110b7795d\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct DisconnectPeerRequest {
    pub peer_pubkey: ::std::string::String,
}
impl DisconnectPeerRequest {
    pub fn builder() -> builder::DisconnectPeerRequest {
        Default::default()
    }
}
#[doc = "`EmbeddedMedia`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"data\","]
#[doc = "    \"mime\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"data\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"integer\""]
#[doc = "      },"]
#[doc = "      \"example\": ["]
#[doc = "        82,"]
#[doc = "        76,"]
#[doc = "        78"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"mime\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"text/plain\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct EmbeddedMedia {
    pub data: ::std::vec::Vec<i64>,
    pub mime: ::std::string::String,
}
impl EmbeddedMedia {
    pub fn builder() -> builder::EmbeddedMedia {
        Default::default()
    }
}
#[doc = "`EmptyResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\""]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct EmptyResponse(pub ::serde_json::Map<::std::string::String, ::serde_json::Value>);
impl ::std::ops::Deref for EmptyResponse {
    type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
    fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
        &self.0
    }
}
impl ::std::convert::From<EmptyResponse>
    for ::serde_json::Map<::std::string::String, ::serde_json::Value>
{
    fn from(value: EmptyResponse) -> Self {
        value.0
    }
}
impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
    for EmptyResponse
{
    fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
        Self(value)
    }
}
#[doc = "`EstimateFeeRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"blocks\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"blocks\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 7"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct EstimateFeeRequest {
    pub blocks: i64,
}
impl EstimateFeeRequest {
    pub fn builder() -> builder::EstimateFeeRequest {
        Default::default()
    }
}
#[doc = "`EstimateFeeResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"fee_rate\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"fee_rate\": {"]
#[doc = "      \"type\": \"number\","]
#[doc = "      \"example\": 9.3"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct EstimateFeeResponse {
    pub fee_rate: f64,
}
impl EstimateFeeResponse {
    pub fn builder() -> builder::EstimateFeeResponse {
        Default::default()
    }
}
#[doc = "`FailTransfersRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"no_asset_only\","]
#[doc = "    \"skip_sync\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"batch_transfer_idx\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": null"]
#[doc = "    },"]
#[doc = "    \"no_asset_only\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    },"]
#[doc = "    \"skip_sync\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct FailTransfersRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub batch_transfer_idx: ::std::option::Option<i64>,
    pub no_asset_only: bool,
    pub skip_sync: bool,
}
impl FailTransfersRequest {
    pub fn builder() -> builder::FailTransfersRequest {
        Default::default()
    }
}
#[doc = "`FailTransfersResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"transfers_changed\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"transfers_changed\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": true"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct FailTransfersResponse {
    pub transfers_changed: bool,
}
impl FailTransfersResponse {
    pub fn builder() -> builder::FailTransfersResponse {
        Default::default()
    }
}
#[doc = "`GetAssetMediaRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"digest\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"digest\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct GetAssetMediaRequest {
    pub digest: ::std::string::String,
}
impl GetAssetMediaRequest {
    pub fn builder() -> builder::GetAssetMediaRequest {
        Default::default()
    }
}
#[doc = "`GetAssetMediaResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"bytes_hex\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"bytes_hex\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"68656c6c6f0a\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct GetAssetMediaResponse {
    pub bytes_hex: ::std::string::String,
}
impl GetAssetMediaResponse {
    pub fn builder() -> builder::GetAssetMediaResponse {
        Default::default()
    }
}
#[doc = "`GetChannelIdRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"temporary_channel_id\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"temporary_channel_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"a8b60c8ce3067b5fc881d4831323e24751daec3b64353c8df3205ec5d838f1c5\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct GetChannelIdRequest {
    pub temporary_channel_id: ::std::string::String,
}
impl GetChannelIdRequest {
    pub fn builder() -> builder::GetChannelIdRequest {
        Default::default()
    }
}
#[doc = "`GetChannelIdResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"channel_id\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"channel_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"8129afe1b1d7cf60d5e1bf4c04b09bec925ed4df5417ceee0484e24f816a105a\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct GetChannelIdResponse {
    pub channel_id: ::std::string::String,
}
impl GetChannelIdResponse {
    pub fn builder() -> builder::GetChannelIdResponse {
        Default::default()
    }
}
#[doc = "`GetPaymentRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"payment_hash\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"payment_hash\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"5ca5d81b482b4015e7b14df7a27fe0a38c226273604ffd3b008b752571811938\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct GetPaymentRequest {
    pub payment_hash: ::std::string::String,
}
impl GetPaymentRequest {
    pub fn builder() -> builder::GetPaymentRequest {
        Default::default()
    }
}
#[doc = "`GetPaymentResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"payment\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"payment\": {"]
#[doc = "      \"$ref\": \"#/$defs/Payment\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct GetPaymentResponse {
    pub payment: Payment,
}
impl GetPaymentResponse {
    pub fn builder() -> builder::GetPaymentResponse {
        Default::default()
    }
}
#[doc = "`GetSwapRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"payment_hash\","]
#[doc = "    \"taker\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"payment_hash\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"5ca5d81b482b4015e7b14df7a27fe0a38c226273604ffd3b008b752571811938\""]
#[doc = "    },"]
#[doc = "    \"taker\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct GetSwapRequest {
    pub payment_hash: ::std::string::String,
    pub taker: bool,
}
impl GetSwapRequest {
    pub fn builder() -> builder::GetSwapRequest {
        Default::default()
    }
}
#[doc = "`GetSwapResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"swap\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"swap\": {"]
#[doc = "      \"$ref\": \"#/$defs/Swap\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct GetSwapResponse {
    pub swap: Swap,
}
impl GetSwapResponse {
    pub fn builder() -> builder::GetSwapResponse {
        Default::default()
    }
}
#[doc = "`HtlcStatus`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"Pending\","]
#[doc = "    \"Succeeded\","]
#[doc = "    \"Failed\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum HtlcStatus {
    Pending,
    Succeeded,
    Failed,
}
impl ::std::fmt::Display for HtlcStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Pending => f.write_str("Pending"),
            Self::Succeeded => f.write_str("Succeeded"),
            Self::Failed => f.write_str("Failed"),
        }
    }
}
impl ::std::str::FromStr for HtlcStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Pending" => Ok(Self::Pending),
            "Succeeded" => Ok(Self::Succeeded),
            "Failed" => Ok(Self::Failed),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for HtlcStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for HtlcStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for HtlcStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`IndexerProtocol`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"Electrum\","]
#[doc = "    \"Esplora\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum IndexerProtocol {
    Electrum,
    Esplora,
}
impl ::std::fmt::Display for IndexerProtocol {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Electrum => f.write_str("Electrum"),
            Self::Esplora => f.write_str("Esplora"),
        }
    }
}
impl ::std::str::FromStr for IndexerProtocol {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Electrum" => Ok(Self::Electrum),
            "Esplora" => Ok(Self::Esplora),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for IndexerProtocol {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for IndexerProtocol {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for IndexerProtocol {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`InflateRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"asset_id\","]
#[doc = "    \"fee_rate\","]
#[doc = "    \"inflation_amounts\","]
#[doc = "    \"min_confirmations\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"fee_rate\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 5"]
#[doc = "    },"]
#[doc = "    \"inflation_amounts\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"integer\""]
#[doc = "      },"]
#[doc = "      \"minItems\": 1,"]
#[doc = "      \"example\": ["]
#[doc = "        100,"]
#[doc = "        50"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"min_confirmations\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InflateRequest {
    pub asset_id: ::std::string::String,
    pub fee_rate: i64,
    pub inflation_amounts: ::std::vec::Vec<i64>,
    pub min_confirmations: i64,
}
impl InflateRequest {
    pub fn builder() -> builder::InflateRequest {
        Default::default()
    }
}
#[doc = "`InflateResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"txid\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"txid\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"7c2c95b9c2aa0a7d140495b664de7973b76561de833f0dd84def3efa08941664\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InflateResponse {
    pub txid: ::std::string::String,
}
impl InflateResponse {
    pub fn builder() -> builder::InflateResponse {
        Default::default()
    }
}
#[doc = "`InitRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"password\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"mnemonic\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"skill lamp please gown put season degree collect decline account monitor insane\""]
#[doc = "    },"]
#[doc = "    \"password\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"nodepassword\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InitRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub mnemonic: ::std::option::Option<::std::string::String>,
    pub password: ::std::string::String,
}
impl InitRequest {
    pub fn builder() -> builder::InitRequest {
        Default::default()
    }
}
#[doc = "`InitResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"mnemonic\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"mnemonic\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"skill lamp please gown put season degree collect decline account monitor insane\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InitResponse {
    pub mnemonic: ::std::string::String,
}
impl InitResponse {
    pub fn builder() -> builder::InitResponse {
        Default::default()
    }
}
#[doc = "`InvoiceStatus`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"Pending\","]
#[doc = "    \"Succeeded\","]
#[doc = "    \"Failed\","]
#[doc = "    \"Expired\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum InvoiceStatus {
    Pending,
    Succeeded,
    Failed,
    Expired,
}
impl ::std::fmt::Display for InvoiceStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Pending => f.write_str("Pending"),
            Self::Succeeded => f.write_str("Succeeded"),
            Self::Failed => f.write_str("Failed"),
            Self::Expired => f.write_str("Expired"),
        }
    }
}
impl ::std::str::FromStr for InvoiceStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Pending" => Ok(Self::Pending),
            "Succeeded" => Ok(Self::Succeeded),
            "Failed" => Ok(Self::Failed),
            "Expired" => Ok(Self::Expired),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for InvoiceStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for InvoiceStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for InvoiceStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`InvoiceStatusRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"invoice\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"invoice\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"lnbcrt30u1pjv6yzndqud3jxktt5w46x7unfv9kz6mn0v3jsnp4qdpc280eur52luxppv6f3nnj8l6vnd9g2hnv3qv6mjhmhvlzf6327pp5tjjasx6g9dqptea3fhm6yllq5wxzycnnvp8l6wcq3d6j2uvpryuqsp5l8az8x3g8fe05dg7cmgddld3da09nfjvky8xftwsk4cj8p2l7kfq9qyysgqcqpcxqzdylzlwfnkyw3jv344x4rzwgkk53ng0fhxy5rdduk4g5tpvea8xa6rfckkza35va28xjn2tqkhgarcxep5umm4x5k56wfcdvu95eq7qzp20vrl4xz76syapsa3c09j7lg5gerkaj63llj0ark7ph8hfketn6fkqzm8laf66dhsncm23wkwm5l5377we9e8lnlknnkwje5eefkccusqm6rqt8\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InvoiceStatusRequest {
    pub invoice: ::std::string::String,
}
impl InvoiceStatusRequest {
    pub fn builder() -> builder::InvoiceStatusRequest {
        Default::default()
    }
}
#[doc = "`InvoiceStatusResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"status\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"status\": {"]
#[doc = "      \"$ref\": \"#/$defs/InvoiceStatus\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InvoiceStatusResponse {
    pub status: InvoiceStatus,
}
impl InvoiceStatusResponse {
    pub fn builder() -> builder::InvoiceStatusResponse {
        Default::default()
    }
}
#[doc = "`IssueAssetCfaRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"amounts\","]
#[doc = "    \"name\","]
#[doc = "    \"precision\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"amounts\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"integer\""]
#[doc = "      },"]
#[doc = "      \"example\": ["]
#[doc = "        1000,"]
#[doc = "        600"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"details\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"asset details\""]
#[doc = "    },"]
#[doc = "    \"file_digest\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03\""]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"Tether\""]
#[doc = "    },"]
#[doc = "    \"precision\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 0"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct IssueAssetCfaRequest {
    pub amounts: ::std::vec::Vec<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub details: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub file_digest: ::std::option::Option<::std::string::String>,
    pub name: ::std::string::String,
    pub precision: i64,
}
impl IssueAssetCfaRequest {
    pub fn builder() -> builder::IssueAssetCfaRequest {
        Default::default()
    }
}
#[doc = "`IssueAssetCfaResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"asset\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset\": {"]
#[doc = "      \"$ref\": \"#/$defs/AssetCFA\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct IssueAssetCfaResponse {
    pub asset: AssetCfa,
}
impl IssueAssetCfaResponse {
    pub fn builder() -> builder::IssueAssetCfaResponse {
        Default::default()
    }
}
#[doc = "`IssueAssetIfaRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"amounts\","]
#[doc = "    \"inflation_amounts\","]
#[doc = "    \"name\","]
#[doc = "    \"precision\","]
#[doc = "    \"ticker\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"amounts\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"integer\""]
#[doc = "      },"]
#[doc = "      \"minItems\": 1,"]
#[doc = "      \"example\": ["]
#[doc = "        1000,"]
#[doc = "        600"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"inflation_amounts\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"integer\""]
#[doc = "      },"]
#[doc = "      \"example\": ["]
#[doc = "        100,"]
#[doc = "        50"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"Tether\""]
#[doc = "    },"]
#[doc = "    \"precision\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 0"]
#[doc = "    },"]
#[doc = "    \"reject_list_url\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"https://some.domain/someasset/rejectlist\""]
#[doc = "    },"]
#[doc = "    \"ticker\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"USDT\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct IssueAssetIfaRequest {
    pub amounts: ::std::vec::Vec<i64>,
    pub inflation_amounts: ::std::vec::Vec<i64>,
    pub name: ::std::string::String,
    pub precision: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub reject_list_url: ::std::option::Option<::std::string::String>,
    pub ticker: ::std::string::String,
}
impl IssueAssetIfaRequest {
    pub fn builder() -> builder::IssueAssetIfaRequest {
        Default::default()
    }
}
#[doc = "`IssueAssetIfaResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"asset\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset\": {"]
#[doc = "      \"$ref\": \"#/$defs/AssetIFA\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct IssueAssetIfaResponse {
    pub asset: AssetIfa,
}
impl IssueAssetIfaResponse {
    pub fn builder() -> builder::IssueAssetIfaResponse {
        Default::default()
    }
}
#[doc = "`IssueAssetNiaRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"amounts\","]
#[doc = "    \"name\","]
#[doc = "    \"precision\","]
#[doc = "    \"ticker\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"amounts\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"integer\""]
#[doc = "      },"]
#[doc = "      \"example\": ["]
#[doc = "        1000,"]
#[doc = "        600"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"Tether\""]
#[doc = "    },"]
#[doc = "    \"precision\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 0"]
#[doc = "    },"]
#[doc = "    \"ticker\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"USDT\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct IssueAssetNiaRequest {
    pub amounts: ::std::vec::Vec<i64>,
    pub name: ::std::string::String,
    pub precision: i64,
    pub ticker: ::std::string::String,
}
impl IssueAssetNiaRequest {
    pub fn builder() -> builder::IssueAssetNiaRequest {
        Default::default()
    }
}
#[doc = "`IssueAssetNiaResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"asset\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset\": {"]
#[doc = "      \"$ref\": \"#/$defs/AssetNIA\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct IssueAssetNiaResponse {
    pub asset: AssetNia,
}
impl IssueAssetNiaResponse {
    pub fn builder() -> builder::IssueAssetNiaResponse {
        Default::default()
    }
}
#[doc = "`IssueAssetUdaRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"attachments_file_digests\","]
#[doc = "    \"name\","]
#[doc = "    \"precision\","]
#[doc = "    \"ticker\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"attachments_file_digests\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"string\""]
#[doc = "      },"]
#[doc = "      \"example\": ["]
#[doc = "        \"5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03\","]
#[doc = "        \"d7516e3a27cdf35aa9dcb323b5f556344ef7f57570be30b88de2bfd4ba339b1a\""]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"details\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"asset details\""]
#[doc = "    },"]
#[doc = "    \"media_file_digest\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"/path/to/media\""]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"Unique\""]
#[doc = "    },"]
#[doc = "    \"precision\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 0"]
#[doc = "    },"]
#[doc = "    \"ticker\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"UNI\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct IssueAssetUdaRequest {
    pub attachments_file_digests: ::std::vec::Vec<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub details: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub media_file_digest: ::std::option::Option<::std::string::String>,
    pub name: ::std::string::String,
    pub precision: i64,
    pub ticker: ::std::string::String,
}
impl IssueAssetUdaRequest {
    pub fn builder() -> builder::IssueAssetUdaRequest {
        Default::default()
    }
}
#[doc = "`IssueAssetUdaResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"asset\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset\": {"]
#[doc = "      \"$ref\": \"#/$defs/AssetUDA\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct IssueAssetUdaResponse {
    pub asset: AssetUda,
}
impl IssueAssetUdaResponse {
    pub fn builder() -> builder::IssueAssetUdaResponse {
        Default::default()
    }
}
#[doc = "`KeysendRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"amt_msat\","]
#[doc = "    \"dest_pubkey\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"amt_msat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 3000000"]
#[doc = "    },"]
#[doc = "    \"asset_amount\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 42"]
#[doc = "    },"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"dest_pubkey\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"03b79a4bc1ec365524b4fab9a39eb133753646babb5a1da5c4bc94c53110b7795d\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct KeysendRequest {
    pub amt_msat: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_amount: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_id: ::std::option::Option<::std::string::String>,
    pub dest_pubkey: ::std::string::String,
}
impl KeysendRequest {
    pub fn builder() -> builder::KeysendRequest {
        Default::default()
    }
}
#[doc = "`KeysendResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"payment_hash\","]
#[doc = "    \"payment_preimage\","]
#[doc = "    \"status\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"payment_hash\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"8ffd4c0642047bc51ea01a22e6b2ede0fc001aee0e9929b2e84e41cf6589d61e\""]
#[doc = "    },"]
#[doc = "    \"payment_preimage\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"89d28bd306aa9bb906fd0ac31092d04c37c919a171b343083167e2a3cdc60578\""]
#[doc = "    },"]
#[doc = "    \"status\": {"]
#[doc = "      \"$ref\": \"#/$defs/HTLCStatus\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct KeysendResponse {
    pub payment_hash: ::std::string::String,
    pub payment_preimage: ::std::string::String,
    pub status: HtlcStatus,
}
impl KeysendResponse {
    pub fn builder() -> builder::KeysendResponse {
        Default::default()
    }
}
#[doc = "`ListAssetsRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"filter_asset_schemas\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"filter_asset_schemas\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/AssetSchema\""]
#[doc = "      },"]
#[doc = "      \"example\": ["]
#[doc = "        \"Nia\","]
#[doc = "        \"Uda\","]
#[doc = "        \"Cfa\","]
#[doc = "        \"Ifa\""]
#[doc = "      ]"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ListAssetsRequest {
    pub filter_asset_schemas: ::std::vec::Vec<AssetSchema>,
}
impl ListAssetsRequest {
    pub fn builder() -> builder::ListAssetsRequest {
        Default::default()
    }
}
#[doc = "`ListAssetsResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"cfa\","]
#[doc = "    \"ifa\","]
#[doc = "    \"nia\","]
#[doc = "    \"uda\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"cfa\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"array\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/AssetCFA\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"ifa\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"array\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/AssetIFA\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"nia\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"array\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/AssetNIA\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"uda\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"array\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/AssetUDA\""]
#[doc = "      }"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ListAssetsResponse {
    pub cfa: ::std::option::Option<::std::vec::Vec<AssetCfa>>,
    pub ifa: ::std::option::Option<::std::vec::Vec<AssetIfa>>,
    pub nia: ::std::option::Option<::std::vec::Vec<AssetNia>>,
    pub uda: ::std::option::Option<::std::vec::Vec<AssetUda>>,
}
impl ListAssetsResponse {
    pub fn builder() -> builder::ListAssetsResponse {
        Default::default()
    }
}
#[doc = "`ListChannelsResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"channels\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"channels\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/Channel\""]
#[doc = "      }"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ListChannelsResponse {
    pub channels: ::std::vec::Vec<Channel>,
}
impl ListChannelsResponse {
    pub fn builder() -> builder::ListChannelsResponse {
        Default::default()
    }
}
#[doc = "`ListPaymentsResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"payments\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"payments\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/Payment\""]
#[doc = "      }"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ListPaymentsResponse {
    pub payments: ::std::vec::Vec<Payment>,
}
impl ListPaymentsResponse {
    pub fn builder() -> builder::ListPaymentsResponse {
        Default::default()
    }
}
#[doc = "`ListPeersResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"peers\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"peers\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/Peer\""]
#[doc = "      }"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ListPeersResponse {
    pub peers: ::std::vec::Vec<Peer>,
}
impl ListPeersResponse {
    pub fn builder() -> builder::ListPeersResponse {
        Default::default()
    }
}
#[doc = "`ListSwapsResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"maker\","]
#[doc = "    \"taker\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"maker\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/Swap\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"taker\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/Swap\""]
#[doc = "      }"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ListSwapsResponse {
    pub maker: ::std::vec::Vec<Swap>,
    pub taker: ::std::vec::Vec<Swap>,
}
impl ListSwapsResponse {
    pub fn builder() -> builder::ListSwapsResponse {
        Default::default()
    }
}
#[doc = "`ListTransactionsRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"skip_sync\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"skip_sync\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ListTransactionsRequest {
    pub skip_sync: bool,
}
impl ListTransactionsRequest {
    pub fn builder() -> builder::ListTransactionsRequest {
        Default::default()
    }
}
#[doc = "`ListTransactionsResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"transactions\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"transactions\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/Transaction\""]
#[doc = "      }"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ListTransactionsResponse {
    pub transactions: ::std::vec::Vec<Transaction>,
}
impl ListTransactionsResponse {
    pub fn builder() -> builder::ListTransactionsResponse {
        Default::default()
    }
}
#[doc = "`ListTransfersRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"asset_id\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ListTransfersRequest {
    pub asset_id: ::std::string::String,
}
impl ListTransfersRequest {
    pub fn builder() -> builder::ListTransfersRequest {
        Default::default()
    }
}
#[doc = "`ListTransfersResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"transfers\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"transfers\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/Transfer\""]
#[doc = "      }"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ListTransfersResponse {
    pub transfers: ::std::vec::Vec<Transfer>,
}
impl ListTransfersResponse {
    pub fn builder() -> builder::ListTransfersResponse {
        Default::default()
    }
}
#[doc = "`ListUnspentsRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"settled_only\","]
#[doc = "    \"skip_sync\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"settled_only\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    },"]
#[doc = "    \"skip_sync\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ListUnspentsRequest {
    pub settled_only: bool,
    pub skip_sync: bool,
}
impl ListUnspentsRequest {
    pub fn builder() -> builder::ListUnspentsRequest {
        Default::default()
    }
}
#[doc = "`ListUnspentsResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"unspents\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"unspents\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/Unspent\""]
#[doc = "      }"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ListUnspentsResponse {
    pub unspents: ::std::vec::Vec<Unspent>,
}
impl ListUnspentsResponse {
    pub fn builder() -> builder::ListUnspentsResponse {
        Default::default()
    }
}
#[doc = "`LnInvoiceRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"expiry_sec\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"amt_msat\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 3000000"]
#[doc = "    },"]
#[doc = "    \"asset_amount\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 42"]
#[doc = "    },"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"expiry_sec\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 420"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct LnInvoiceRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub amt_msat: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_amount: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_id: ::std::option::Option<::std::string::String>,
    pub expiry_sec: i64,
}
impl LnInvoiceRequest {
    pub fn builder() -> builder::LnInvoiceRequest {
        Default::default()
    }
}
#[doc = "`LnInvoiceResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"invoice\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"invoice\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"lnbcrt30u1pjv6yzndqud3jxktt5w46x7unfv9kz6mn0v3jsnp4qdpc280eur52luxppv6f3nnj8l6vnd9g2hnv3qv6mjhmhvlzf6327pp5tjjasx6g9dqptea3fhm6yllq5wxzycnnvp8l6wcq3d6j2uvpryuqsp5l8az8x3g8fe05dg7cmgddld3da09nfjvky8xftwsk4cj8p2l7kfq9qyysgqcqpcxqzdylzlwfnkyw3jv344x4rzwgkk53ng0fhxy5rdduk4g5tpvea8xa6rfckkza35va28xjn2tqkhgarcxep5umm4x5k56wfcdvu95eq7qzp20vrl4xz76syapsa3c09j7lg5gerkaj63llj0ark7ph8hfketn6fkqzm8laf66dhsncm23wkwm5l5377we9e8lnlknnkwje5eefkccusqm6rqt8\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct LnInvoiceResponse {
    pub invoice: ::std::string::String,
}
impl LnInvoiceResponse {
    pub fn builder() -> builder::LnInvoiceResponse {
        Default::default()
    }
}
#[doc = "`MakerExecuteRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"payment_secret\","]
#[doc = "    \"swapstring\","]
#[doc = "    \"taker_pubkey\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"payment_secret\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"777a7756c620868199ed5fdc35bee4095b5709d543e5c2bf0494396bf27d2ea2\""]
#[doc = "    },"]
#[doc = "    \"swapstring\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"30/rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8/10/rgb:icfqnK9y-wObZKTu-XJcDL98-sKbE5Mh-OuDJhiI-brRJrzE/1715896416/9d342c6ba006e24abee84a2e034a22d5e30c1f2599fb9c3574d46d3cde3d65a2\""]
#[doc = "    },"]
#[doc = "    \"taker_pubkey\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"02270dadcd6e7ba0ef707dac72acccae1a3607453a8dd2aef36ff3be4e0d31f043\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct MakerExecuteRequest {
    pub payment_secret: ::std::string::String,
    pub swapstring: ::std::string::String,
    pub taker_pubkey: ::std::string::String,
}
impl MakerExecuteRequest {
    pub fn builder() -> builder::MakerExecuteRequest {
        Default::default()
    }
}
#[doc = "`MakerInitRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"qty_from\","]
#[doc = "    \"qty_to\","]
#[doc = "    \"timeout_sec\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"from_asset\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"qty_from\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 30"]
#[doc = "    },"]
#[doc = "    \"qty_to\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 10"]
#[doc = "    },"]
#[doc = "    \"timeout_sec\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 100"]
#[doc = "    },"]
#[doc = "    \"to_asset\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:icfqnK9y-wObZKTu-XJcDL98-sKbE5Mh-OuDJhiI-brRJrzE\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct MakerInitRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub from_asset: ::std::option::Option<::std::string::String>,
    pub qty_from: i64,
    pub qty_to: i64,
    pub timeout_sec: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub to_asset: ::std::option::Option<::std::string::String>,
}
impl MakerInitRequest {
    pub fn builder() -> builder::MakerInitRequest {
        Default::default()
    }
}
#[doc = "`MakerInitResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"payment_hash\","]
#[doc = "    \"payment_secret\","]
#[doc = "    \"swapstring\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"payment_hash\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"3febfae1e68b190c15461f4c2a3290f9af1dae63fd7d620d2bd61601869026cd\""]
#[doc = "    },"]
#[doc = "    \"payment_secret\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"777a7756c620868199ed5fdc35bee4095b5709d543e5c2bf0494396bf27d2ea2\""]
#[doc = "    },"]
#[doc = "    \"swapstring\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"30/rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8/10/rgb:icfqnK9y-wObZKTu-XJcDL98-sKbE5Mh-OuDJhiI-brRJrzE/1715896416/9d342c6ba006e24abee84a2e034a22d5e30c1f2599fb9c3574d46d3cde3d65a2\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct MakerInitResponse {
    pub payment_hash: ::std::string::String,
    pub payment_secret: ::std::string::String,
    pub swapstring: ::std::string::String,
}
impl MakerInitResponse {
    pub fn builder() -> builder::MakerInitResponse {
        Default::default()
    }
}
#[doc = "`Media`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"digest\","]
#[doc = "    \"file_path\","]
#[doc = "    \"mime\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"digest\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03\""]
#[doc = "    },"]
#[doc = "    \"file_path\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"/path/to/media\""]
#[doc = "    },"]
#[doc = "    \"mime\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"text/plain\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Media {
    pub digest: ::std::string::String,
    pub file_path: ::std::string::String,
    pub mime: ::std::string::String,
}
impl Media {
    pub fn builder() -> builder::Media {
        Default::default()
    }
}
#[doc = "`NetworkInfoResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"height\","]
#[doc = "    \"network\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"height\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 805434"]
#[doc = "    },"]
#[doc = "    \"network\": {"]
#[doc = "      \"$ref\": \"#/$defs/BitcoinNetwork\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct NetworkInfoResponse {
    pub height: i64,
    pub network: BitcoinNetwork,
}
impl NetworkInfoResponse {
    pub fn builder() -> builder::NetworkInfoResponse {
        Default::default()
    }
}
#[doc = "`NodeInfoResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"account_xpub_colored\","]
#[doc = "    \"account_xpub_vanilla\","]
#[doc = "    \"channel_asset_max_amount\","]
#[doc = "    \"channel_asset_min_amount\","]
#[doc = "    \"channel_capacity_max_sat\","]
#[doc = "    \"channel_capacity_min_sat\","]
#[doc = "    \"eventual_close_fees_sat\","]
#[doc = "    \"local_balance_sat\","]
#[doc = "    \"max_media_upload_size_mb\","]
#[doc = "    \"network_channels\","]
#[doc = "    \"network_nodes\","]
#[doc = "    \"num_channels\","]
#[doc = "    \"num_peers\","]
#[doc = "    \"num_usable_channels\","]
#[doc = "    \"pending_outbound_payments_sat\","]
#[doc = "    \"pubkey\","]
#[doc = "    \"rgb_channel_capacity_min_sat\","]
#[doc = "    \"rgb_htlc_min_msat\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"account_xpub_colored\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"tpubDDcdKhaxwVV2T6xwigti7dSY1a7LHFwZmKAaLWtNhzrvuTXqjjzo8U7YQkUuPah5yHvnk3cbXmb18ZRFwHEKTFUQmA9dij1nPVA2LCJCiEa\""]
#[doc = "    },"]
#[doc = "    \"account_xpub_vanilla\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"tpubDDfzqHEET3ksD81qshMHkw35yp6TuLP1kr5rWWeJcLAqDfMXKDJzmDwAnda6DCqw7kkkhPphuDZFE2a6Sw8h5ZA5NwmtTssEnjMqN7xMzSd\""]
#[doc = "    },"]
#[doc = "    \"channel_asset_max_amount\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"format\": \"uint64\","]
#[doc = "      \"example\": 18446744073709551615"]
#[doc = "    },"]
#[doc = "    \"channel_asset_min_amount\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"format\": \"uint64\","]
#[doc = "      \"example\": 1"]
#[doc = "    },"]
#[doc = "    \"channel_capacity_max_sat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 16777215"]
#[doc = "    },"]
#[doc = "    \"channel_capacity_min_sat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 5506"]
#[doc = "    },"]
#[doc = "    \"eventual_close_fees_sat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 892"]
#[doc = "    },"]
#[doc = "    \"local_balance_sat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 28616"]
#[doc = "    },"]
#[doc = "    \"max_media_upload_size_mb\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 5"]
#[doc = "    },"]
#[doc = "    \"network_channels\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 7812821"]
#[doc = "    },"]
#[doc = "    \"network_nodes\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 987226"]
#[doc = "    },"]
#[doc = "    \"num_channels\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1"]
#[doc = "    },"]
#[doc = "    \"num_peers\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1"]
#[doc = "    },"]
#[doc = "    \"num_usable_channels\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 0"]
#[doc = "    },"]
#[doc = "    \"pending_outbound_payments_sat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 7852"]
#[doc = "    },"]
#[doc = "    \"pubkey\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"02270dadcd6e7ba0ef707dac72acccae1a3607453a8dd2aef36ff3be4e0d31f043\""]
#[doc = "    },"]
#[doc = "    \"rgb_channel_capacity_min_sat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 30010"]
#[doc = "    },"]
#[doc = "    \"rgb_htlc_min_msat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 3000000"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct NodeInfoResponse {
    pub account_xpub_colored: ::std::string::String,
    pub account_xpub_vanilla: ::std::string::String,
    pub channel_asset_max_amount: u64,
    pub channel_asset_min_amount: u64,
    pub channel_capacity_max_sat: i64,
    pub channel_capacity_min_sat: i64,
    pub eventual_close_fees_sat: i64,
    pub local_balance_sat: i64,
    pub max_media_upload_size_mb: i64,
    pub network_channels: i64,
    pub network_nodes: i64,
    pub num_channels: i64,
    pub num_peers: i64,
    pub num_usable_channels: i64,
    pub pending_outbound_payments_sat: i64,
    pub pubkey: ::std::string::String,
    pub rgb_channel_capacity_min_sat: i64,
    pub rgb_htlc_min_msat: i64,
}
impl NodeInfoResponse {
    pub fn builder() -> builder::NodeInfoResponse {
        Default::default()
    }
}
#[doc = "`OpenChannelRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"capacity_sat\","]
#[doc = "    \"peer_pubkey_and_opt_addr\","]
#[doc = "    \"public\","]
#[doc = "    \"push_msat\","]
#[doc = "    \"with_anchors\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset_amount\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 333"]
#[doc = "    },"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"capacity_sat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 30010"]
#[doc = "    },"]
#[doc = "    \"fee_base_msat\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 1000"]
#[doc = "    },"]
#[doc = "    \"fee_proportional_millionths\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 0"]
#[doc = "    },"]
#[doc = "    \"peer_pubkey_and_opt_addr\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"03b79a4bc1ec365524b4fab9a39eb133753646babb5a1da5c4bc94c53110b7795d@localhost:9736\""]
#[doc = "    },"]
#[doc = "    \"public\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": true"]
#[doc = "    },"]
#[doc = "    \"push_asset_amount\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 100"]
#[doc = "    },"]
#[doc = "    \"push_msat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1394000"]
#[doc = "    },"]
#[doc = "    \"temporary_channel_id\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"a8b60c8ce3067b5fc881d4831323e24751daec3b64353c8df3205ec5d838f1c5\""]
#[doc = "    },"]
#[doc = "    \"with_anchors\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": true"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OpenChannelRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_amount: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_id: ::std::option::Option<::std::string::String>,
    pub capacity_sat: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub fee_base_msat: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub fee_proportional_millionths: ::std::option::Option<i64>,
    pub peer_pubkey_and_opt_addr: ::std::string::String,
    pub public: bool,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub push_asset_amount: ::std::option::Option<i64>,
    pub push_msat: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub temporary_channel_id: ::std::option::Option<::std::string::String>,
    pub with_anchors: bool,
}
impl OpenChannelRequest {
    pub fn builder() -> builder::OpenChannelRequest {
        Default::default()
    }
}
#[doc = "`OpenChannelResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"temporary_channel_id\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"temporary_channel_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"a8b60c8ce3067b5fc881d4831323e24751daec3b64353c8df3205ec5d838f1c5\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct OpenChannelResponse {
    pub temporary_channel_id: ::std::string::String,
}
impl OpenChannelResponse {
    pub fn builder() -> builder::OpenChannelResponse {
        Default::default()
    }
}
#[doc = "`Payment`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"created_at\","]
#[doc = "    \"inbound\","]
#[doc = "    \"payee_pubkey\","]
#[doc = "    \"payment_hash\","]
#[doc = "    \"status\","]
#[doc = "    \"updated_at\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"amt_msat\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 3000000"]
#[doc = "    },"]
#[doc = "    \"asset_amount\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 42"]
#[doc = "    },"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"created_at\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691160765"]
#[doc = "    },"]
#[doc = "    \"inbound\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": true"]
#[doc = "    },"]
#[doc = "    \"payee_pubkey\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"03b79a4bc1ec365524b4fab9a39eb133753646babb5a1da5c4bc94c53110b7795d\""]
#[doc = "    },"]
#[doc = "    \"payment_hash\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"3febfae1e68b190c15461f4c2a3290f9af1dae63fd7d620d2bd61601869026cd\""]
#[doc = "    },"]
#[doc = "    \"preimage\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"89d28bd306aa9bb906fd0ac31092d04c37c919a171b343083167e2a3cdc60578\""]
#[doc = "    },"]
#[doc = "    \"status\": {"]
#[doc = "      \"$ref\": \"#/$defs/HTLCStatus\""]
#[doc = "    },"]
#[doc = "    \"updated_at\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691162674"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Payment {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub amt_msat: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_amount: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_id: ::std::option::Option<::std::string::String>,
    pub created_at: i64,
    pub inbound: bool,
    pub payee_pubkey: ::std::string::String,
    pub payment_hash: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub preimage: ::std::option::Option<::std::string::String>,
    pub status: HtlcStatus,
    pub updated_at: i64,
}
impl Payment {
    pub fn builder() -> builder::Payment {
        Default::default()
    }
}
#[doc = "`Peer`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"pubkey\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"pubkey\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"03b79a4bc1ec365524b4fab9a39eb133753646babb5a1da5c4bc94c53110b7795d\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Peer {
    pub pubkey: ::std::string::String,
}
impl Peer {
    pub fn builder() -> builder::Peer {
        Default::default()
    }
}
#[doc = "`PostAssetMediaRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"file\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"file\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"format\": \"binary\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct PostAssetMediaRequest {
    pub file: ::std::string::String,
}
impl PostAssetMediaRequest {
    pub fn builder() -> builder::PostAssetMediaRequest {
        Default::default()
    }
}
#[doc = "`PostAssetMediaResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"digest\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"digest\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct PostAssetMediaResponse {
    pub digest: ::std::string::String,
}
impl PostAssetMediaResponse {
    pub fn builder() -> builder::PostAssetMediaResponse {
        Default::default()
    }
}
#[doc = "`ProofOfReserves`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"proof\","]
#[doc = "    \"utxo\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"proof\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"integer\""]
#[doc = "      },"]
#[doc = "      \"example\": ["]
#[doc = "        6,"]
#[doc = "        36,"]
#[doc = "        87,"]
#[doc = "        13,"]
#[doc = "        5,"]
#[doc = "        17"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"utxo\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"efed66f5309396ff43c8a09941c8103d9d5bbffd473ad9f13013ac89fb6b4671:0\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct ProofOfReserves {
    pub proof: ::std::vec::Vec<i64>,
    pub utxo: ::std::string::String,
}
impl ProofOfReserves {
    pub fn builder() -> builder::ProofOfReserves {
        Default::default()
    }
}
#[doc = "`Recipient`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"assignment\","]
#[doc = "    \"recipient_id\","]
#[doc = "    \"transport_endpoints\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"assignment\": {"]
#[doc = "      \"$ref\": \"#/$defs/Assignment\""]
#[doc = "    },"]
#[doc = "    \"recipient_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"bcrt:utxob:2FZsSuk-iyVQLVuU4-Gc6J4qkE8-mLS17N4jd-MEx6cWz9F-MFkyE1n\""]
#[doc = "    },"]
#[doc = "    \"transport_endpoints\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"string\","]
#[doc = "        \"example\": \"rpc://127.0.0.1:3000/json-rpc\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"witness_data\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/WitnessData\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Recipient {
    pub assignment: Assignment,
    pub recipient_id: ::std::string::String,
    pub transport_endpoints: ::std::vec::Vec<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub witness_data: ::std::option::Option<WitnessData>,
}
impl Recipient {
    pub fn builder() -> builder::Recipient {
        Default::default()
    }
}
#[doc = "`RecipientType`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"Blind\","]
#[doc = "    \"Witness\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum RecipientType {
    Blind,
    Witness,
}
impl ::std::fmt::Display for RecipientType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Blind => f.write_str("Blind"),
            Self::Witness => f.write_str("Witness"),
        }
    }
}
impl ::std::str::FromStr for RecipientType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Blind" => Ok(Self::Blind),
            "Witness" => Ok(Self::Witness),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for RecipientType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for RecipientType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for RecipientType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`RefreshFilter`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"incoming\","]
#[doc = "    \"status\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"incoming\": {"]
#[doc = "      \"type\": \"boolean\""]
#[doc = "    },"]
#[doc = "    \"status\": {"]
#[doc = "      \"$ref\": \"#/$defs/RefreshTransferStatus\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct RefreshFilter {
    pub incoming: bool,
    pub status: RefreshTransferStatus,
}
impl RefreshFilter {
    pub fn builder() -> builder::RefreshFilter {
        Default::default()
    }
}
#[doc = "`RefreshRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"filter\","]
#[doc = "    \"skip_sync\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"rgb:2dkSTbr-jFhznbPmo-TQafzswCN-av4gTsJjX-ttx6CNou5-M98k8Zd\","]
#[doc = "      \"nullable\": true"]
#[doc = "    },"]
#[doc = "    \"filter\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/RefreshFilter\""]
#[doc = "      },"]
#[doc = "      \"example\": []"]
#[doc = "    },"]
#[doc = "    \"skip_sync\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct RefreshRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_id: ::std::option::Option<::std::string::String>,
    pub filter: ::std::vec::Vec<RefreshFilter>,
    pub skip_sync: bool,
}
impl RefreshRequest {
    pub fn builder() -> builder::RefreshRequest {
        Default::default()
    }
}
#[doc = "`RefreshTransferStatus`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"WaitingCounterparty\","]
#[doc = "    \"WaitingConfirmations\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum RefreshTransferStatus {
    WaitingCounterparty,
    WaitingConfirmations,
}
impl ::std::fmt::Display for RefreshTransferStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::WaitingCounterparty => f.write_str("WaitingCounterparty"),
            Self::WaitingConfirmations => f.write_str("WaitingConfirmations"),
        }
    }
}
impl ::std::str::FromStr for RefreshTransferStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "WaitingCounterparty" => Ok(Self::WaitingCounterparty),
            "WaitingConfirmations" => Ok(Self::WaitingConfirmations),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for RefreshTransferStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for RefreshTransferStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for RefreshTransferStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`RestoreRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"backup_path\","]
#[doc = "    \"password\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"backup_path\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"/path/to/the/backup/file\""]
#[doc = "    },"]
#[doc = "    \"password\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"nodepassword\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct RestoreRequest {
    pub backup_path: ::std::string::String,
    pub password: ::std::string::String,
}
impl RestoreRequest {
    pub fn builder() -> builder::RestoreRequest {
        Default::default()
    }
}
#[doc = "`RevokeTokenRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"token\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"token\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"EnYKDBgDIggKBggGEgIYDRIkCAASICqCgqtFMIJ1eLCM3raDzqg9UqV-6nJWzGjjJG0S5IIUGkBpF-itmppHcdcSrSCiKklz9VZT4UmIND_0RFc32Imq3bLR_Y7GYaSpJo5lJfU1cA2BG_hy7P1UN4g5jKTKS88GIiIKIAUKXrrx0Ca-rMZa537VOFw2X8q_KVQ6OC4Z0ztro0sQ\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct RevokeTokenRequest {
    pub token: ::std::string::String,
}
impl RevokeTokenRequest {
    pub fn builder() -> builder::RevokeTokenRequest {
        Default::default()
    }
}
#[doc = "`RgbAllocation`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"assignment\","]
#[doc = "    \"settled\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"assignment\": {"]
#[doc = "      \"$ref\": \"#/$defs/Assignment\""]
#[doc = "    },"]
#[doc = "    \"settled\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct RgbAllocation {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_id: ::std::option::Option<::std::string::String>,
    pub assignment: Assignment,
    pub settled: bool,
}
impl RgbAllocation {
    pub fn builder() -> builder::RgbAllocation {
        Default::default()
    }
}
#[doc = "`RgbInvoiceRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"min_confirmations\","]
#[doc = "    \"witness\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"assignment\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/Assignment\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ],"]
#[doc = "      \"example\": null"]
#[doc = "    },"]
#[doc = "    \"expiration_timestamp\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": null"]
#[doc = "    },"]
#[doc = "    \"min_confirmations\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1"]
#[doc = "    },"]
#[doc = "    \"witness\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct RgbInvoiceRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_id: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub assignment: ::std::option::Option<Assignment>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub expiration_timestamp: ::std::option::Option<i64>,
    pub min_confirmations: i64,
    pub witness: bool,
}
impl RgbInvoiceRequest {
    pub fn builder() -> builder::RgbInvoiceRequest {
        Default::default()
    }
}
#[doc = "`RgbInvoiceResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"batch_transfer_idx\","]
#[doc = "    \"invoice\","]
#[doc = "    \"recipient_id\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"batch_transfer_idx\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1"]
#[doc = "    },"]
#[doc = "    \"expiration_timestamp\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 1695811760"]
#[doc = "    },"]
#[doc = "    \"invoice\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"rgb:~/~/~/bcrt:utxob:cbgHUJ4e-7QyKY4U-Jsj5AZw-oI0gxZh-7fxQY2_-tFFUAZN-4CgpX?expiry=1695811760&endpoints=rpc://127.0.0.1:3000/json-rpc\""]
#[doc = "    },"]
#[doc = "    \"recipient_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"bcrt:utxob:cbgHUJ4e-7QyKY4U-Jsj5AZw-oI0gxZh-7fxQY2_-tFFUAZN-4CgpX\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct RgbInvoiceResponse {
    pub batch_transfer_idx: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub expiration_timestamp: ::std::option::Option<i64>,
    pub invoice: ::std::string::String,
    pub recipient_id: ::std::string::String,
}
impl RgbInvoiceResponse {
    pub fn builder() -> builder::RgbInvoiceResponse {
        Default::default()
    }
}
#[doc = "`SendBtcRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"address\","]
#[doc = "    \"amount\","]
#[doc = "    \"fee_rate\","]
#[doc = "    \"skip_sync\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"address\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"bcrt1qwxht5tut39dws8tjcf649tp908r8fr2j75c94k\""]
#[doc = "    },"]
#[doc = "    \"amount\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 16900"]
#[doc = "    },"]
#[doc = "    \"fee_rate\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 5"]
#[doc = "    },"]
#[doc = "    \"skip_sync\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SendBtcRequest {
    pub address: ::std::string::String,
    pub amount: i64,
    pub fee_rate: i64,
    pub skip_sync: bool,
}
impl SendBtcRequest {
    pub fn builder() -> builder::SendBtcRequest {
        Default::default()
    }
}
#[doc = "`SendBtcResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"txid\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"txid\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"7c2c95b9c2aa0a7d140495b664de7973b76561de833f0dd84def3efa08941664\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SendBtcResponse {
    pub txid: ::std::string::String,
}
impl SendBtcResponse {
    pub fn builder() -> builder::SendBtcResponse {
        Default::default()
    }
}
#[doc = "`SendOnionMessageRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"data\","]
#[doc = "    \"node_ids\","]
#[doc = "    \"tlv_type\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"data\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"message to send\""]
#[doc = "    },"]
#[doc = "    \"node_ids\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"string\","]
#[doc = "        \"example\": \"03b79a4bc1ec365524b4fab9a39eb133753646babb5a1da5c4bc94c53110b7795d\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"tlv_type\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 77"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SendOnionMessageRequest {
    pub data: ::std::string::String,
    pub node_ids: ::std::vec::Vec<::std::string::String>,
    pub tlv_type: i64,
}
impl SendOnionMessageRequest {
    pub fn builder() -> builder::SendOnionMessageRequest {
        Default::default()
    }
}
#[doc = "`SendPaymentRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"invoice\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"amt_msat\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 3000000"]
#[doc = "    },"]
#[doc = "    \"asset_amount\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 100"]
#[doc = "    },"]
#[doc = "    \"asset_id\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"invoice\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"lnbcrt30u1pjv6yzndqud3jxktt5w46x7unfv9kz6mn0v3jsnp4qdpc280eur52luxppv6f3nnj8l6vnd9g2hnv3qv6mjhmhvlzf6327pp5tjjasx6g9dqptea3fhm6yllq5wxzycnnvp8l6wcq3d6j2uvpryuqsp5l8az8x3g8fe05dg7cmgddld3da09nfjvky8xftwsk4cj8p2l7kfq9qyysgqcqpcxqzdylzlwfnkyw3jv344x4rzwgkk53ng0fhxy5rdduk4g5tpvea8xa6rfckkza35va28xjn2tqkhgarcxep5umm4x5k56wfcdvu95eq7qzp20vrl4xz76syapsa3c09j7lg5gerkaj63llj0ark7ph8hfketn6fkqzm8laf66dhsncm23wkwm5l5377we9e8lnlknnkwje5eefkccusqm6rqt8\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SendPaymentRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub amt_msat: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_amount: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub asset_id: ::std::option::Option<::std::string::String>,
    pub invoice: ::std::string::String,
}
impl SendPaymentRequest {
    pub fn builder() -> builder::SendPaymentRequest {
        Default::default()
    }
}
#[doc = "`SendPaymentResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"payment_id\","]
#[doc = "    \"status\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"payment_hash\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"3febfae1e68b190c15461f4c2a3290f9af1dae63fd7d620d2bd61601869026cd\""]
#[doc = "    },"]
#[doc = "    \"payment_id\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"3febfae1e68b190c15461f4c2a3290f9af1dae63fd7d620d2bd61601869026cd\""]
#[doc = "    },"]
#[doc = "    \"payment_secret\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"777a7756c620868199ed5fdc35bee4095b5709d543e5c2bf0494396bf27d2ea2\""]
#[doc = "    },"]
#[doc = "    \"status\": {"]
#[doc = "      \"$ref\": \"#/$defs/HTLCStatus\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SendPaymentResponse {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub payment_hash: ::std::option::Option<::std::string::String>,
    pub payment_id: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub payment_secret: ::std::option::Option<::std::string::String>,
    pub status: HtlcStatus,
}
impl SendPaymentResponse {
    pub fn builder() -> builder::SendPaymentResponse {
        Default::default()
    }
}
#[doc = "`SendRgbRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"donation\","]
#[doc = "    \"fee_rate\","]
#[doc = "    \"min_confirmations\","]
#[doc = "    \"recipient_map\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"donation\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    },"]
#[doc = "    \"expiration_timestamp\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": null"]
#[doc = "    },"]
#[doc = "    \"fee_rate\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 5"]
#[doc = "    },"]
#[doc = "    \"min_confirmations\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1"]
#[doc = "    },"]
#[doc = "    \"recipient_map\": {"]
#[doc = "      \"type\": \"object\","]
#[doc = "      \"additionalProperties\": {"]
#[doc = "        \"type\": \"array\","]
#[doc = "        \"items\": {"]
#[doc = "          \"$ref\": \"#/$defs/Recipient\""]
#[doc = "        }"]
#[doc = "      },"]
#[doc = "      \"example\": {"]
#[doc = "        \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\": ["]
#[doc = "          {"]
#[doc = "            \"assignment\": {"]
#[doc = "              \"type\": \"Fungible\","]
#[doc = "              \"value\": 400"]
#[doc = "            },"]
#[doc = "            \"recipient_id\": \"utxob:2FjRqgQ-eEWCVHY5-zmpFtYzT-gGm3MdR-sTnxNcS-7RtUbY9-4NYuuh\","]
#[doc = "            \"transport_endpoints\": ["]
#[doc = "              \"rpc://127.0.0.1:3000/json-rpc\""]
#[doc = "            ]"]
#[doc = "          },"]
#[doc = "          {"]
#[doc = "            \"assignment\": {"]
#[doc = "              \"type\": \"Fungible\","]
#[doc = "              \"value\": 200"]
#[doc = "            },"]
#[doc = "            \"recipient_id\": \"utxob:3GkRrhR-fFXDLIZ6-0anqGuzU-hHn4NeS-tUoyOdT-8SuVcZ0-5OZvvi\","]
#[doc = "            \"transport_endpoints\": ["]
#[doc = "              \"rpc://127.0.0.1:3000/json-rpc\""]
#[doc = "            ]"]
#[doc = "          }"]
#[doc = "        ],"]
#[doc = "        \"rgb:d8qDVS5X-ICVG2uM-CPr3yO4-lfBhgjt-7FN1EPE-ApY1LcM\": ["]
#[doc = "          {"]
#[doc = "            \"assignment\": {"]
#[doc = "              \"type\": \"Fungible\","]
#[doc = "              \"value\": 100"]
#[doc = "            },"]
#[doc = "            \"recipient_id\": \"utxob:4HlSsiS-gGYEMKA7-1borHvaV-iIo5OfT-uVpzPeU-9TvWdA1-6PAwwj\","]
#[doc = "            \"transport_endpoints\": ["]
#[doc = "              \"rpc://127.0.0.1:3000/json-rpc\""]
#[doc = "            ]"]
#[doc = "          }"]
#[doc = "        ]"]
#[doc = "      }"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SendRgbRequest {
    pub donation: bool,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub expiration_timestamp: ::std::option::Option<i64>,
    pub fee_rate: i64,
    pub min_confirmations: i64,
    pub recipient_map:
        ::std::collections::HashMap<::std::string::String, ::std::vec::Vec<Recipient>>,
}
impl SendRgbRequest {
    pub fn builder() -> builder::SendRgbRequest {
        Default::default()
    }
}
#[doc = "`SendRgbResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"txid\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"txid\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"7c2c95b9c2aa0a7d140495b664de7973b76561de833f0dd84def3efa08941664\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SendRgbResponse {
    pub txid: ::std::string::String,
}
impl SendRgbResponse {
    pub fn builder() -> builder::SendRgbResponse {
        Default::default()
    }
}
#[doc = "`SignMessageRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"message\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"message\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"message to sign\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SignMessageRequest {
    pub message: ::std::string::String,
}
impl SignMessageRequest {
    pub fn builder() -> builder::SignMessageRequest {
        Default::default()
    }
}
#[doc = "`SignMessageResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"signed_message\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"signed_message\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"signed message\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SignMessageResponse {
    pub signed_message: ::std::string::String,
}
impl SignMessageResponse {
    pub fn builder() -> builder::SignMessageResponse {
        Default::default()
    }
}
#[doc = "`Swap`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"expires_at\","]
#[doc = "    \"payment_hash\","]
#[doc = "    \"qty_from\","]
#[doc = "    \"qty_to\","]
#[doc = "    \"requested_at\","]
#[doc = "    \"status\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"completed_at\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 1691171075"]
#[doc = "    },"]
#[doc = "    \"expires_at\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691172703"]
#[doc = "    },"]
#[doc = "    \"from_asset\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8\""]
#[doc = "    },"]
#[doc = "    \"initiated_at\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 1691168512"]
#[doc = "    },"]
#[doc = "    \"payment_hash\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"7c2c95b9c2aa0a7d140495b664de7973b76561de833f0dd84def3efa08941664\""]
#[doc = "    },"]
#[doc = "    \"qty_from\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 30"]
#[doc = "    },"]
#[doc = "    \"qty_to\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 10"]
#[doc = "    },"]
#[doc = "    \"requested_at\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691160765"]
#[doc = "    },"]
#[doc = "    \"status\": {"]
#[doc = "      \"$ref\": \"#/$defs/SwapStatus\""]
#[doc = "    },"]
#[doc = "    \"to_asset\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rgb:icfqnK9y-wObZKTu-XJcDL98-sKbE5Mh-OuDJhiI-brRJrzE\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Swap {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub completed_at: ::std::option::Option<i64>,
    pub expires_at: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub from_asset: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub initiated_at: ::std::option::Option<i64>,
    pub payment_hash: ::std::string::String,
    pub qty_from: i64,
    pub qty_to: i64,
    pub requested_at: i64,
    pub status: SwapStatus,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub to_asset: ::std::option::Option<::std::string::String>,
}
impl Swap {
    pub fn builder() -> builder::Swap {
        Default::default()
    }
}
#[doc = "`SwapStatus`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"Waiting\","]
#[doc = "    \"Pending\","]
#[doc = "    \"Succeeded\","]
#[doc = "    \"Expired\","]
#[doc = "    \"Failed\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum SwapStatus {
    Waiting,
    Pending,
    Succeeded,
    Expired,
    Failed,
}
impl ::std::fmt::Display for SwapStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Waiting => f.write_str("Waiting"),
            Self::Pending => f.write_str("Pending"),
            Self::Succeeded => f.write_str("Succeeded"),
            Self::Expired => f.write_str("Expired"),
            Self::Failed => f.write_str("Failed"),
        }
    }
}
impl ::std::str::FromStr for SwapStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Waiting" => Ok(Self::Waiting),
            "Pending" => Ok(Self::Pending),
            "Succeeded" => Ok(Self::Succeeded),
            "Expired" => Ok(Self::Expired),
            "Failed" => Ok(Self::Failed),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SwapStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for SwapStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SwapStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SyncKeychain`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"oneOf\": ["]
#[doc = "    {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"enum\": ["]
#[doc = "        \"Colored\""]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"type\": \"object\","]
#[doc = "      \"required\": ["]
#[doc = "        \"Vanilla\""]
#[doc = "      ],"]
#[doc = "      \"properties\": {"]
#[doc = "        \"Vanilla\": {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"required\": ["]
#[doc = "            \"lookback\""]
#[doc = "          ],"]
#[doc = "          \"properties\": {"]
#[doc = "            \"lookback\": {"]
#[doc = "              \"type\": \"integer\","]
#[doc = "              \"example\": 20"]
#[doc = "            }"]
#[doc = "          }"]
#[doc = "        }"]
#[doc = "      }"]
#[doc = "    }"]
#[doc = "  ],"]
#[doc = "  \"example\": {"]
#[doc = "    \"Vanilla\": {"]
#[doc = "      \"lookback\": 20"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub enum SyncKeychain {
    Colored,
    Vanilla { lookback: i64 },
}
#[doc = "`SyncOptions`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"keychain\","]
#[doc = "    \"strategy\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"keychain\": {"]
#[doc = "      \"$ref\": \"#/$defs/SyncKeychain\""]
#[doc = "    },"]
#[doc = "    \"strategy\": {"]
#[doc = "      \"$ref\": \"#/$defs/SyncStrategy\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SyncOptions {
    pub keychain: SyncKeychain,
    pub strategy: SyncStrategy,
}
impl SyncOptions {
    pub fn builder() -> builder::SyncOptions {
        Default::default()
    }
}
#[doc = "`SyncRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"options\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"options\": {"]
#[doc = "      \"$ref\": \"#/$defs/SyncOptions\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SyncRequest {
    pub options: SyncOptions,
}
impl SyncRequest {
    pub fn builder() -> builder::SyncRequest {
        Default::default()
    }
}
#[doc = "`SyncStrategy`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"FastSync\","]
#[doc = "    \"FullSync\","]
#[doc = "    \"FullScan\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum SyncStrategy {
    FastSync,
    FullSync,
    FullScan,
}
impl ::std::fmt::Display for SyncStrategy {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::FastSync => f.write_str("FastSync"),
            Self::FullSync => f.write_str("FullSync"),
            Self::FullScan => f.write_str("FullScan"),
        }
    }
}
impl ::std::str::FromStr for SyncStrategy {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "FastSync" => Ok(Self::FastSync),
            "FullSync" => Ok(Self::FullSync),
            "FullScan" => Ok(Self::FullScan),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SyncStrategy {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for SyncStrategy {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SyncStrategy {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`TakerRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"swapstring\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"swapstring\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"30/rgb:CJkb4YZw-jRiz2sk-~PARPio-wtVYI1c-XAEYCqO-wTfvRZ8/10/rgb:icfqnK9y-wObZKTu-XJcDL98-sKbE5Mh-OuDJhiI-brRJrzE/1715896416/9d342c6ba006e24abee84a2e034a22d5e30c1f2599fb9c3574d46d3cde3d65a2\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct TakerRequest {
    pub swapstring: ::std::string::String,
}
impl TakerRequest {
    pub fn builder() -> builder::TakerRequest {
        Default::default()
    }
}
#[doc = "`Token`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"attachments\","]
#[doc = "    \"index\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"attachments\": {"]
#[doc = "      \"type\": \"object\","]
#[doc = "      \"additionalProperties\": {"]
#[doc = "        \"$ref\": \"#/$defs/Media\""]
#[doc = "      },"]
#[doc = "      \"example\": {"]
#[doc = "        \"0\": {"]
#[doc = "          \"digest\": \"5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03\","]
#[doc = "          \"file_path\": \"path/to/attachment0\","]
#[doc = "          \"mime\": \"text/plain\""]
#[doc = "        },"]
#[doc = "        \"1\": {"]
#[doc = "          \"digest\": \"d7516e3a27cdf35aa9dcb323b5f556344ef7f57570be30b88de2bfd4ba339b1a\","]
#[doc = "          \"file_path\": \"path/to/attachment1\","]
#[doc = "          \"mime\": \"image/png\""]
#[doc = "        }"]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"details\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"token details\""]
#[doc = "    },"]
#[doc = "    \"embedded_media\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/EmbeddedMedia\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"index\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 0"]
#[doc = "    },"]
#[doc = "    \"media\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/Media\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"Token\""]
#[doc = "    },"]
#[doc = "    \"reserves\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/ProofOfReserves\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"ticker\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"TKN\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Token {
    pub attachments: ::std::collections::HashMap<::std::string::String, Media>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub details: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub embedded_media: ::std::option::Option<EmbeddedMedia>,
    pub index: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub media: ::std::option::Option<Media>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub name: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub reserves: ::std::option::Option<ProofOfReserves>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub ticker: ::std::option::Option<::std::string::String>,
}
impl Token {
    pub fn builder() -> builder::Token {
        Default::default()
    }
}
#[doc = "`TokenLight`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"attachments\","]
#[doc = "    \"embedded_media\","]
#[doc = "    \"index\","]
#[doc = "    \"reserves\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"attachments\": {"]
#[doc = "      \"type\": \"object\","]
#[doc = "      \"additionalProperties\": {"]
#[doc = "        \"$ref\": \"#/$defs/Media\""]
#[doc = "      },"]
#[doc = "      \"example\": {"]
#[doc = "        \"0\": {"]
#[doc = "          \"digest\": \"5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03\","]
#[doc = "          \"file_path\": \"path/to/attachment0\","]
#[doc = "          \"mime\": \"text/plain\""]
#[doc = "        },"]
#[doc = "        \"1\": {"]
#[doc = "          \"digest\": \"d7516e3a27cdf35aa9dcb323b5f556344ef7f57570be30b88de2bfd4ba339b1a\","]
#[doc = "          \"file_path\": \"path/to/attachment1\","]
#[doc = "          \"mime\": \"image/png\""]
#[doc = "        }"]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"details\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"token details\""]
#[doc = "    },"]
#[doc = "    \"embedded_media\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": true"]
#[doc = "    },"]
#[doc = "    \"index\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 0"]
#[doc = "    },"]
#[doc = "    \"media\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/Media\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"Token\""]
#[doc = "    },"]
#[doc = "    \"reserves\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    },"]
#[doc = "    \"ticker\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"TKN\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct TokenLight {
    pub attachments: ::std::collections::HashMap<::std::string::String, Media>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub details: ::std::option::Option<::std::string::String>,
    pub embedded_media: bool,
    pub index: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub media: ::std::option::Option<Media>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub name: ::std::option::Option<::std::string::String>,
    pub reserves: bool,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub ticker: ::std::option::Option<::std::string::String>,
}
impl TokenLight {
    pub fn builder() -> builder::TokenLight {
        Default::default()
    }
}
#[doc = "`Transaction`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"fee\","]
#[doc = "    \"received\","]
#[doc = "    \"sent\","]
#[doc = "    \"transaction_type\","]
#[doc = "    \"txid\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"confirmation_time\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/BlockTime\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"fee\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 100"]
#[doc = "    },"]
#[doc = "    \"received\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 650"]
#[doc = "    },"]
#[doc = "    \"sent\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1050"]
#[doc = "    },"]
#[doc = "    \"transaction_type\": {"]
#[doc = "      \"$ref\": \"#/$defs/TransactionType\""]
#[doc = "    },"]
#[doc = "    \"txid\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"7c2c95b9c2aa0a7d140495b664de7973b76561de833f0dd84def3efa08941664\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Transaction {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub confirmation_time: ::std::option::Option<BlockTime>,
    pub fee: i64,
    pub received: i64,
    pub sent: i64,
    pub transaction_type: TransactionType,
    pub txid: ::std::string::String,
}
impl Transaction {
    pub fn builder() -> builder::Transaction {
        Default::default()
    }
}
#[doc = "`TransactionType`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"RgbSend\","]
#[doc = "    \"Drain\","]
#[doc = "    \"CreateUtxos\","]
#[doc = "    \"SendBtc\","]
#[doc = "    \"Incoming\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum TransactionType {
    RgbSend,
    Drain,
    CreateUtxos,
    SendBtc,
    Incoming,
}
impl ::std::fmt::Display for TransactionType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::RgbSend => f.write_str("RgbSend"),
            Self::Drain => f.write_str("Drain"),
            Self::CreateUtxos => f.write_str("CreateUtxos"),
            Self::SendBtc => f.write_str("SendBtc"),
            Self::Incoming => f.write_str("Incoming"),
        }
    }
}
impl ::std::str::FromStr for TransactionType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "RgbSend" => Ok(Self::RgbSend),
            "Drain" => Ok(Self::Drain),
            "CreateUtxos" => Ok(Self::CreateUtxos),
            "SendBtc" => Ok(Self::SendBtc),
            "Incoming" => Ok(Self::Incoming),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TransactionType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for TransactionType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TransactionType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`Transfer`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"assignments\","]
#[doc = "    \"created_at\","]
#[doc = "    \"idx\","]
#[doc = "    \"kind\","]
#[doc = "    \"status\","]
#[doc = "    \"transport_endpoints\","]
#[doc = "    \"updated_at\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"assignments\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/Assignment\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"change_utxo\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": null"]
#[doc = "    },"]
#[doc = "    \"created_at\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691160765"]
#[doc = "    },"]
#[doc = "    \"expiration_timestamp\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 1691171612"]
#[doc = "    },"]
#[doc = "    \"idx\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1"]
#[doc = "    },"]
#[doc = "    \"kind\": {"]
#[doc = "      \"$ref\": \"#/$defs/TransferKind\""]
#[doc = "    },"]
#[doc = "    \"receive_utxo\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"efed66f5309396ff43c8a09941c8103d9d5bbffd473ad9f13013ac89fb6b4671:0\""]
#[doc = "    },"]
#[doc = "    \"recipient_id\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"61qsVbWtkNmU54F2i6qtB9uSmEGsPoaeypCi5uC5uctZ\""]
#[doc = "    },"]
#[doc = "    \"requested_assignment\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/Assignment\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"status\": {"]
#[doc = "      \"$ref\": \"#/$defs/TransferStatus\""]
#[doc = "    },"]
#[doc = "    \"transport_endpoints\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/TransferTransportEndpoint\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"txid\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"7c2c95b9c2aa0a7d140495b664de7973b76561de833f0dd84def3efa08941664\""]
#[doc = "    },"]
#[doc = "    \"updated_at\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1691162674"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Transfer {
    pub assignments: ::std::vec::Vec<Assignment>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub change_utxo: ::std::option::Option<::std::string::String>,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub expiration_timestamp: ::std::option::Option<i64>,
    pub idx: i64,
    pub kind: TransferKind,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub receive_utxo: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub recipient_id: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub requested_assignment: ::std::option::Option<Assignment>,
    pub status: TransferStatus,
    pub transport_endpoints: ::std::vec::Vec<TransferTransportEndpoint>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub txid: ::std::option::Option<::std::string::String>,
    pub updated_at: i64,
}
impl Transfer {
    pub fn builder() -> builder::Transfer {
        Default::default()
    }
}
#[doc = "`TransferKind`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"Issuance\","]
#[doc = "    \"ReceiveBlind\","]
#[doc = "    \"ReceiveWitness\","]
#[doc = "    \"Send\","]
#[doc = "    \"Inflation\","]
#[doc = "    \"Burn\""]
#[doc = "  ],"]
#[doc = "  \"example\": \"ReceiveBlind\""]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum TransferKind {
    Issuance,
    ReceiveBlind,
    ReceiveWitness,
    Send,
    Inflation,
    Burn,
}
impl ::std::fmt::Display for TransferKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Issuance => f.write_str("Issuance"),
            Self::ReceiveBlind => f.write_str("ReceiveBlind"),
            Self::ReceiveWitness => f.write_str("ReceiveWitness"),
            Self::Send => f.write_str("Send"),
            Self::Inflation => f.write_str("Inflation"),
            Self::Burn => f.write_str("Burn"),
        }
    }
}
impl ::std::str::FromStr for TransferKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Issuance" => Ok(Self::Issuance),
            "ReceiveBlind" => Ok(Self::ReceiveBlind),
            "ReceiveWitness" => Ok(Self::ReceiveWitness),
            "Send" => Ok(Self::Send),
            "Inflation" => Ok(Self::Inflation),
            "Burn" => Ok(Self::Burn),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TransferKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for TransferKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TransferKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`TransferStatus`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"Initiated\","]
#[doc = "    \"WaitingCounterparty\","]
#[doc = "    \"WaitingSafeHeight\","]
#[doc = "    \"WaitingConfirmations\","]
#[doc = "    \"Settled\","]
#[doc = "    \"Failed\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum TransferStatus {
    Initiated,
    WaitingCounterparty,
    WaitingSafeHeight,
    WaitingConfirmations,
    Settled,
    Failed,
}
impl ::std::fmt::Display for TransferStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Initiated => f.write_str("Initiated"),
            Self::WaitingCounterparty => f.write_str("WaitingCounterparty"),
            Self::WaitingSafeHeight => f.write_str("WaitingSafeHeight"),
            Self::WaitingConfirmations => f.write_str("WaitingConfirmations"),
            Self::Settled => f.write_str("Settled"),
            Self::Failed => f.write_str("Failed"),
        }
    }
}
impl ::std::str::FromStr for TransferStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Initiated" => Ok(Self::Initiated),
            "WaitingCounterparty" => Ok(Self::WaitingCounterparty),
            "WaitingSafeHeight" => Ok(Self::WaitingSafeHeight),
            "WaitingConfirmations" => Ok(Self::WaitingConfirmations),
            "Settled" => Ok(Self::Settled),
            "Failed" => Ok(Self::Failed),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TransferStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for TransferStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TransferStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`TransferTransportEndpoint`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"endpoint\","]
#[doc = "    \"transport_type\","]
#[doc = "    \"used\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"endpoint\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"http://127.0.0.1:3000/json-rpc\""]
#[doc = "    },"]
#[doc = "    \"transport_type\": {"]
#[doc = "      \"$ref\": \"#/$defs/TransportType\""]
#[doc = "    },"]
#[doc = "    \"used\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": false"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct TransferTransportEndpoint {
    pub endpoint: ::std::string::String,
    pub transport_type: TransportType,
    pub used: bool,
}
impl TransferTransportEndpoint {
    pub fn builder() -> builder::TransferTransportEndpoint {
        Default::default()
    }
}
#[doc = "`TransportType`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"JsonRpc\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum TransportType {
    JsonRpc,
}
impl ::std::fmt::Display for TransportType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::JsonRpc => f.write_str("JsonRpc"),
        }
    }
}
impl ::std::str::FromStr for TransportType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "JsonRpc" => Ok(Self::JsonRpc),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TransportType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for TransportType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TransportType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`UnlockRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"announce_addresses\","]
#[doc = "    \"bitcoind_rpc_host\","]
#[doc = "    \"bitcoind_rpc_password\","]
#[doc = "    \"bitcoind_rpc_port\","]
#[doc = "    \"bitcoind_rpc_username\","]
#[doc = "    \"password\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"announce_addresses\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"string\","]
#[doc = "        \"example\": \"pub.addr.example.com:9735\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"announce_alias\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"nodeAlias\""]
#[doc = "    },"]
#[doc = "    \"bitcoind_rpc_host\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"localhost\""]
#[doc = "    },"]
#[doc = "    \"bitcoind_rpc_password\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"password\""]
#[doc = "    },"]
#[doc = "    \"bitcoind_rpc_port\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 18443"]
#[doc = "    },"]
#[doc = "    \"bitcoind_rpc_username\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"user\""]
#[doc = "    },"]
#[doc = "    \"indexer_url\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"127.0.0.1:50001\""]
#[doc = "    },"]
#[doc = "    \"password\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"nodepassword\""]
#[doc = "    },"]
#[doc = "    \"proxy_endpoint\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"string\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": \"rpc://127.0.0.1:3000/json-rpc\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct UnlockRequest {
    pub announce_addresses: ::std::vec::Vec<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub announce_alias: ::std::option::Option<::std::string::String>,
    pub bitcoind_rpc_host: ::std::string::String,
    pub bitcoind_rpc_password: ::std::string::String,
    pub bitcoind_rpc_port: i64,
    pub bitcoind_rpc_username: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub indexer_url: ::std::option::Option<::std::string::String>,
    pub password: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub proxy_endpoint: ::std::option::Option<::std::string::String>,
}
impl UnlockRequest {
    pub fn builder() -> builder::UnlockRequest {
        Default::default()
    }
}
#[doc = "`Unspent`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"rgb_allocations\","]
#[doc = "    \"utxo\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"rgb_allocations\": {"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/RgbAllocation\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"utxo\": {"]
#[doc = "      \"$ref\": \"#/$defs/Utxo\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Unspent {
    pub rgb_allocations: ::std::vec::Vec<RgbAllocation>,
    pub utxo: Utxo,
}
impl Unspent {
    pub fn builder() -> builder::Unspent {
        Default::default()
    }
}
#[doc = "`Utxo`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"btc_amount\","]
#[doc = "    \"colorable\","]
#[doc = "    \"outpoint\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"btc_amount\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1000"]
#[doc = "    },"]
#[doc = "    \"colorable\": {"]
#[doc = "      \"type\": \"boolean\","]
#[doc = "      \"example\": true"]
#[doc = "    },"]
#[doc = "    \"outpoint\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"example\": \"efed66f5309396ff43c8a09941c8103d9d5bbffd473ad9f13013ac89fb6b4671:0\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Utxo {
    pub btc_amount: i64,
    pub colorable: bool,
    pub outpoint: ::std::string::String,
}
impl Utxo {
    pub fn builder() -> builder::Utxo {
        Default::default()
    }
}
#[doc = "`WitnessData`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"amount_sat\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"amount_sat\": {"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"example\": 1000"]
#[doc = "    },"]
#[doc = "    \"blinding\": {"]
#[doc = "      \"type\": ["]
#[doc = "        \"integer\","]
#[doc = "        \"null\""]
#[doc = "      ],"]
#[doc = "      \"example\": 439017309"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct WitnessData {
    pub amount_sat: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub blinding: ::std::option::Option<i64>,
}
impl WitnessData {
    pub fn builder() -> builder::WitnessData {
        Default::default()
    }
}
#[doc = r" Types for composing complex structures."]
pub mod builder {
    #[derive(Clone, Debug)]
    pub struct AddressResponse {
        address: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for AddressResponse {
        fn default() -> Self {
            Self {
                address: Err("no value supplied for address".to_string()),
            }
        }
    }
    impl AddressResponse {
        pub fn address<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.address = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for address: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AddressResponse> for super::AddressResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AddressResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                address: value.address?,
            })
        }
    }
    impl ::std::convert::From<super::AddressResponse> for AddressResponse {
        fn from(value: super::AddressResponse) -> Self {
            Self {
                address: Ok(value.address),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AssetBalanceRequest {
        asset_id: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for AssetBalanceRequest {
        fn default() -> Self {
            Self {
                asset_id: Err("no value supplied for asset_id".to_string()),
            }
        }
    }
    impl AssetBalanceRequest {
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AssetBalanceRequest> for super::AssetBalanceRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AssetBalanceRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset_id: value.asset_id?,
            })
        }
    }
    impl ::std::convert::From<super::AssetBalanceRequest> for AssetBalanceRequest {
        fn from(value: super::AssetBalanceRequest) -> Self {
            Self {
                asset_id: Ok(value.asset_id),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AssetBalanceResponse {
        future: ::std::result::Result<i64, ::std::string::String>,
        offchain_inbound: ::std::result::Result<i64, ::std::string::String>,
        offchain_outbound: ::std::result::Result<i64, ::std::string::String>,
        settled: ::std::result::Result<i64, ::std::string::String>,
        spendable: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for AssetBalanceResponse {
        fn default() -> Self {
            Self {
                future: Err("no value supplied for future".to_string()),
                offchain_inbound: Err("no value supplied for offchain_inbound".to_string()),
                offchain_outbound: Err("no value supplied for offchain_outbound".to_string()),
                settled: Err("no value supplied for settled".to_string()),
                spendable: Err("no value supplied for spendable".to_string()),
            }
        }
    }
    impl AssetBalanceResponse {
        pub fn future<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.future = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for future: {e}"));
            self
        }
        pub fn offchain_inbound<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.offchain_inbound = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for offchain_inbound: {e}"));
            self
        }
        pub fn offchain_outbound<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.offchain_outbound = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for offchain_outbound: {e}"));
            self
        }
        pub fn settled<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.settled = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for settled: {e}"));
            self
        }
        pub fn spendable<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.spendable = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for spendable: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AssetBalanceResponse> for super::AssetBalanceResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AssetBalanceResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                future: value.future?,
                offchain_inbound: value.offchain_inbound?,
                offchain_outbound: value.offchain_outbound?,
                settled: value.settled?,
                spendable: value.spendable?,
            })
        }
    }
    impl ::std::convert::From<super::AssetBalanceResponse> for AssetBalanceResponse {
        fn from(value: super::AssetBalanceResponse) -> Self {
            Self {
                future: Ok(value.future),
                offchain_inbound: Ok(value.offchain_inbound),
                offchain_outbound: Ok(value.offchain_outbound),
                settled: Ok(value.settled),
                spendable: Ok(value.spendable),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AssetCfa {
        added_at: ::std::result::Result<i64, ::std::string::String>,
        asset_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        balance: ::std::result::Result<super::AssetBalanceResponse, ::std::string::String>,
        details: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        issued_supply: ::std::result::Result<i64, ::std::string::String>,
        media: ::std::result::Result<::std::option::Option<super::Media>, ::std::string::String>,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        precision: ::std::result::Result<i64, ::std::string::String>,
        timestamp: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for AssetCfa {
        fn default() -> Self {
            Self {
                added_at: Err("no value supplied for added_at".to_string()),
                asset_id: Err("no value supplied for asset_id".to_string()),
                balance: Err("no value supplied for balance".to_string()),
                details: Ok(Default::default()),
                issued_supply: Err("no value supplied for issued_supply".to_string()),
                media: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                precision: Err("no value supplied for precision".to_string()),
                timestamp: Err("no value supplied for timestamp".to_string()),
            }
        }
    }
    impl AssetCfa {
        pub fn added_at<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.added_at = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for added_at: {e}"));
            self
        }
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn balance<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::AssetBalanceResponse>,
            T::Error: ::std::fmt::Display,
        {
            self.balance = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for balance: {e}"));
            self
        }
        pub fn details<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.details = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for details: {e}"));
            self
        }
        pub fn issued_supply<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.issued_supply = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for issued_supply: {e}"));
            self
        }
        pub fn media<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Media>>,
            T::Error: ::std::fmt::Display,
        {
            self.media = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for media: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn precision<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.precision = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for precision: {e}"));
            self
        }
        pub fn timestamp<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.timestamp = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for timestamp: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AssetCfa> for super::AssetCfa {
        type Error = super::error::ConversionError;
        fn try_from(value: AssetCfa) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                added_at: value.added_at?,
                asset_id: value.asset_id?,
                balance: value.balance?,
                details: value.details?,
                issued_supply: value.issued_supply?,
                media: value.media?,
                name: value.name?,
                precision: value.precision?,
                timestamp: value.timestamp?,
            })
        }
    }
    impl ::std::convert::From<super::AssetCfa> for AssetCfa {
        fn from(value: super::AssetCfa) -> Self {
            Self {
                added_at: Ok(value.added_at),
                asset_id: Ok(value.asset_id),
                balance: Ok(value.balance),
                details: Ok(value.details),
                issued_supply: Ok(value.issued_supply),
                media: Ok(value.media),
                name: Ok(value.name),
                precision: Ok(value.precision),
                timestamp: Ok(value.timestamp),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AssetIfa {
        added_at: ::std::result::Result<i64, ::std::string::String>,
        asset_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        balance: ::std::result::Result<super::AssetBalanceResponse, ::std::string::String>,
        details: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        initial_supply: ::std::result::Result<i64, ::std::string::String>,
        known_circulating_supply: ::std::result::Result<i64, ::std::string::String>,
        max_supply: ::std::result::Result<i64, ::std::string::String>,
        media: ::std::result::Result<::std::option::Option<super::Media>, ::std::string::String>,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        precision: ::std::result::Result<i64, ::std::string::String>,
        reject_list_url: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        ticker: ::std::result::Result<::std::string::String, ::std::string::String>,
        timestamp: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for AssetIfa {
        fn default() -> Self {
            Self {
                added_at: Err("no value supplied for added_at".to_string()),
                asset_id: Err("no value supplied for asset_id".to_string()),
                balance: Err("no value supplied for balance".to_string()),
                details: Ok(Default::default()),
                initial_supply: Err("no value supplied for initial_supply".to_string()),
                known_circulating_supply: Err(
                    "no value supplied for known_circulating_supply".to_string()
                ),
                max_supply: Err("no value supplied for max_supply".to_string()),
                media: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                precision: Err("no value supplied for precision".to_string()),
                reject_list_url: Ok(Default::default()),
                ticker: Err("no value supplied for ticker".to_string()),
                timestamp: Err("no value supplied for timestamp".to_string()),
            }
        }
    }
    impl AssetIfa {
        pub fn added_at<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.added_at = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for added_at: {e}"));
            self
        }
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn balance<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::AssetBalanceResponse>,
            T::Error: ::std::fmt::Display,
        {
            self.balance = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for balance: {e}"));
            self
        }
        pub fn details<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.details = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for details: {e}"));
            self
        }
        pub fn initial_supply<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.initial_supply = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for initial_supply: {e}"));
            self
        }
        pub fn known_circulating_supply<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.known_circulating_supply = value.try_into().map_err(|e| {
                format!("error converting supplied value for known_circulating_supply: {e}")
            });
            self
        }
        pub fn max_supply<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.max_supply = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for max_supply: {e}"));
            self
        }
        pub fn media<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Media>>,
            T::Error: ::std::fmt::Display,
        {
            self.media = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for media: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn precision<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.precision = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for precision: {e}"));
            self
        }
        pub fn reject_list_url<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.reject_list_url = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for reject_list_url: {e}"));
            self
        }
        pub fn ticker<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.ticker = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for ticker: {e}"));
            self
        }
        pub fn timestamp<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.timestamp = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for timestamp: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AssetIfa> for super::AssetIfa {
        type Error = super::error::ConversionError;
        fn try_from(value: AssetIfa) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                added_at: value.added_at?,
                asset_id: value.asset_id?,
                balance: value.balance?,
                details: value.details?,
                initial_supply: value.initial_supply?,
                known_circulating_supply: value.known_circulating_supply?,
                max_supply: value.max_supply?,
                media: value.media?,
                name: value.name?,
                precision: value.precision?,
                reject_list_url: value.reject_list_url?,
                ticker: value.ticker?,
                timestamp: value.timestamp?,
            })
        }
    }
    impl ::std::convert::From<super::AssetIfa> for AssetIfa {
        fn from(value: super::AssetIfa) -> Self {
            Self {
                added_at: Ok(value.added_at),
                asset_id: Ok(value.asset_id),
                balance: Ok(value.balance),
                details: Ok(value.details),
                initial_supply: Ok(value.initial_supply),
                known_circulating_supply: Ok(value.known_circulating_supply),
                max_supply: Ok(value.max_supply),
                media: Ok(value.media),
                name: Ok(value.name),
                precision: Ok(value.precision),
                reject_list_url: Ok(value.reject_list_url),
                ticker: Ok(value.ticker),
                timestamp: Ok(value.timestamp),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AssetMetadataRequest {
        asset_id: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for AssetMetadataRequest {
        fn default() -> Self {
            Self {
                asset_id: Err("no value supplied for asset_id".to_string()),
            }
        }
    }
    impl AssetMetadataRequest {
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AssetMetadataRequest> for super::AssetMetadataRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AssetMetadataRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset_id: value.asset_id?,
            })
        }
    }
    impl ::std::convert::From<super::AssetMetadataRequest> for AssetMetadataRequest {
        fn from(value: super::AssetMetadataRequest) -> Self {
            Self {
                asset_id: Ok(value.asset_id),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AssetMetadataResponse {
        asset_schema: ::std::result::Result<super::AssetSchema, ::std::string::String>,
        details: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        initial_supply: ::std::result::Result<i64, ::std::string::String>,
        known_circulating_supply: ::std::result::Result<i64, ::std::string::String>,
        max_supply: ::std::result::Result<i64, ::std::string::String>,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        precision: ::std::result::Result<i64, ::std::string::String>,
        ticker: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        timestamp: ::std::result::Result<i64, ::std::string::String>,
        token: ::std::result::Result<::std::option::Option<super::Token>, ::std::string::String>,
    }
    impl ::std::default::Default for AssetMetadataResponse {
        fn default() -> Self {
            Self {
                asset_schema: Err("no value supplied for asset_schema".to_string()),
                details: Ok(Default::default()),
                initial_supply: Err("no value supplied for initial_supply".to_string()),
                known_circulating_supply: Err(
                    "no value supplied for known_circulating_supply".to_string()
                ),
                max_supply: Err("no value supplied for max_supply".to_string()),
                name: Err("no value supplied for name".to_string()),
                precision: Err("no value supplied for precision".to_string()),
                ticker: Ok(Default::default()),
                timestamp: Err("no value supplied for timestamp".to_string()),
                token: Ok(Default::default()),
            }
        }
    }
    impl AssetMetadataResponse {
        pub fn asset_schema<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::AssetSchema>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_schema = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_schema: {e}"));
            self
        }
        pub fn details<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.details = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for details: {e}"));
            self
        }
        pub fn initial_supply<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.initial_supply = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for initial_supply: {e}"));
            self
        }
        pub fn known_circulating_supply<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.known_circulating_supply = value.try_into().map_err(|e| {
                format!("error converting supplied value for known_circulating_supply: {e}")
            });
            self
        }
        pub fn max_supply<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.max_supply = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for max_supply: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn precision<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.precision = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for precision: {e}"));
            self
        }
        pub fn ticker<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.ticker = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for ticker: {e}"));
            self
        }
        pub fn timestamp<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.timestamp = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for timestamp: {e}"));
            self
        }
        pub fn token<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Token>>,
            T::Error: ::std::fmt::Display,
        {
            self.token = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for token: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AssetMetadataResponse> for super::AssetMetadataResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AssetMetadataResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset_schema: value.asset_schema?,
                details: value.details?,
                initial_supply: value.initial_supply?,
                known_circulating_supply: value.known_circulating_supply?,
                max_supply: value.max_supply?,
                name: value.name?,
                precision: value.precision?,
                ticker: value.ticker?,
                timestamp: value.timestamp?,
                token: value.token?,
            })
        }
    }
    impl ::std::convert::From<super::AssetMetadataResponse> for AssetMetadataResponse {
        fn from(value: super::AssetMetadataResponse) -> Self {
            Self {
                asset_schema: Ok(value.asset_schema),
                details: Ok(value.details),
                initial_supply: Ok(value.initial_supply),
                known_circulating_supply: Ok(value.known_circulating_supply),
                max_supply: Ok(value.max_supply),
                name: Ok(value.name),
                precision: Ok(value.precision),
                ticker: Ok(value.ticker),
                timestamp: Ok(value.timestamp),
                token: Ok(value.token),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AssetNia {
        added_at: ::std::result::Result<i64, ::std::string::String>,
        asset_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        balance: ::std::result::Result<super::AssetBalanceResponse, ::std::string::String>,
        details: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        issued_supply: ::std::result::Result<i64, ::std::string::String>,
        media: ::std::result::Result<::std::option::Option<super::Media>, ::std::string::String>,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        precision: ::std::result::Result<i64, ::std::string::String>,
        ticker: ::std::result::Result<::std::string::String, ::std::string::String>,
        timestamp: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for AssetNia {
        fn default() -> Self {
            Self {
                added_at: Err("no value supplied for added_at".to_string()),
                asset_id: Err("no value supplied for asset_id".to_string()),
                balance: Err("no value supplied for balance".to_string()),
                details: Ok(Default::default()),
                issued_supply: Err("no value supplied for issued_supply".to_string()),
                media: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                precision: Err("no value supplied for precision".to_string()),
                ticker: Err("no value supplied for ticker".to_string()),
                timestamp: Err("no value supplied for timestamp".to_string()),
            }
        }
    }
    impl AssetNia {
        pub fn added_at<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.added_at = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for added_at: {e}"));
            self
        }
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn balance<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::AssetBalanceResponse>,
            T::Error: ::std::fmt::Display,
        {
            self.balance = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for balance: {e}"));
            self
        }
        pub fn details<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.details = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for details: {e}"));
            self
        }
        pub fn issued_supply<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.issued_supply = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for issued_supply: {e}"));
            self
        }
        pub fn media<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Media>>,
            T::Error: ::std::fmt::Display,
        {
            self.media = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for media: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn precision<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.precision = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for precision: {e}"));
            self
        }
        pub fn ticker<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.ticker = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for ticker: {e}"));
            self
        }
        pub fn timestamp<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.timestamp = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for timestamp: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AssetNia> for super::AssetNia {
        type Error = super::error::ConversionError;
        fn try_from(value: AssetNia) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                added_at: value.added_at?,
                asset_id: value.asset_id?,
                balance: value.balance?,
                details: value.details?,
                issued_supply: value.issued_supply?,
                media: value.media?,
                name: value.name?,
                precision: value.precision?,
                ticker: value.ticker?,
                timestamp: value.timestamp?,
            })
        }
    }
    impl ::std::convert::From<super::AssetNia> for AssetNia {
        fn from(value: super::AssetNia) -> Self {
            Self {
                added_at: Ok(value.added_at),
                asset_id: Ok(value.asset_id),
                balance: Ok(value.balance),
                details: Ok(value.details),
                issued_supply: Ok(value.issued_supply),
                media: Ok(value.media),
                name: Ok(value.name),
                precision: Ok(value.precision),
                ticker: Ok(value.ticker),
                timestamp: Ok(value.timestamp),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AssetUda {
        added_at: ::std::result::Result<i64, ::std::string::String>,
        asset_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        balance: ::std::result::Result<super::AssetBalanceResponse, ::std::string::String>,
        details: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        precision: ::std::result::Result<i64, ::std::string::String>,
        ticker: ::std::result::Result<::std::string::String, ::std::string::String>,
        timestamp: ::std::result::Result<i64, ::std::string::String>,
        token:
            ::std::result::Result<::std::option::Option<super::TokenLight>, ::std::string::String>,
    }
    impl ::std::default::Default for AssetUda {
        fn default() -> Self {
            Self {
                added_at: Err("no value supplied for added_at".to_string()),
                asset_id: Err("no value supplied for asset_id".to_string()),
                balance: Err("no value supplied for balance".to_string()),
                details: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                precision: Err("no value supplied for precision".to_string()),
                ticker: Err("no value supplied for ticker".to_string()),
                timestamp: Err("no value supplied for timestamp".to_string()),
                token: Ok(Default::default()),
            }
        }
    }
    impl AssetUda {
        pub fn added_at<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.added_at = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for added_at: {e}"));
            self
        }
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn balance<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::AssetBalanceResponse>,
            T::Error: ::std::fmt::Display,
        {
            self.balance = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for balance: {e}"));
            self
        }
        pub fn details<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.details = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for details: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn precision<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.precision = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for precision: {e}"));
            self
        }
        pub fn ticker<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.ticker = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for ticker: {e}"));
            self
        }
        pub fn timestamp<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.timestamp = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for timestamp: {e}"));
            self
        }
        pub fn token<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::TokenLight>>,
            T::Error: ::std::fmt::Display,
        {
            self.token = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for token: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AssetUda> for super::AssetUda {
        type Error = super::error::ConversionError;
        fn try_from(value: AssetUda) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                added_at: value.added_at?,
                asset_id: value.asset_id?,
                balance: value.balance?,
                details: value.details?,
                name: value.name?,
                precision: value.precision?,
                ticker: value.ticker?,
                timestamp: value.timestamp?,
                token: value.token?,
            })
        }
    }
    impl ::std::convert::From<super::AssetUda> for AssetUda {
        fn from(value: super::AssetUda) -> Self {
            Self {
                added_at: Ok(value.added_at),
                asset_id: Ok(value.asset_id),
                balance: Ok(value.balance),
                details: Ok(value.details),
                name: Ok(value.name),
                precision: Ok(value.precision),
                ticker: Ok(value.ticker),
                timestamp: Ok(value.timestamp),
                token: Ok(value.token),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AssignmentAny {
        type_: ::std::result::Result<super::AssignmentAnyType, ::std::string::String>,
    }
    impl ::std::default::Default for AssignmentAny {
        fn default() -> Self {
            Self {
                type_: Err("no value supplied for type_".to_string()),
            }
        }
    }
    impl AssignmentAny {
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::AssignmentAnyType>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AssignmentAny> for super::AssignmentAny {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AssignmentAny,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                type_: value.type_?,
            })
        }
    }
    impl ::std::convert::From<super::AssignmentAny> for AssignmentAny {
        fn from(value: super::AssignmentAny) -> Self {
            Self {
                type_: Ok(value.type_),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AssignmentFungible {
        type_: ::std::result::Result<super::AssignmentFungibleType, ::std::string::String>,
        value: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for AssignmentFungible {
        fn default() -> Self {
            Self {
                type_: Err("no value supplied for type_".to_string()),
                value: Err("no value supplied for value".to_string()),
            }
        }
    }
    impl AssignmentFungible {
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::AssignmentFungibleType>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {e}"));
            self
        }
        pub fn value<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.value = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for value: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AssignmentFungible> for super::AssignmentFungible {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AssignmentFungible,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                type_: value.type_?,
                value: value.value?,
            })
        }
    }
    impl ::std::convert::From<super::AssignmentFungible> for AssignmentFungible {
        fn from(value: super::AssignmentFungible) -> Self {
            Self {
                type_: Ok(value.type_),
                value: Ok(value.value),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AssignmentInflationRight {
        type_: ::std::result::Result<super::AssignmentInflationRightType, ::std::string::String>,
        value: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for AssignmentInflationRight {
        fn default() -> Self {
            Self {
                type_: Err("no value supplied for type_".to_string()),
                value: Err("no value supplied for value".to_string()),
            }
        }
    }
    impl AssignmentInflationRight {
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::AssignmentInflationRightType>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {e}"));
            self
        }
        pub fn value<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.value = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for value: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AssignmentInflationRight> for super::AssignmentInflationRight {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AssignmentInflationRight,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                type_: value.type_?,
                value: value.value?,
            })
        }
    }
    impl ::std::convert::From<super::AssignmentInflationRight> for AssignmentInflationRight {
        fn from(value: super::AssignmentInflationRight) -> Self {
            Self {
                type_: Ok(value.type_),
                value: Ok(value.value),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AssignmentNonFungible {
        type_: ::std::result::Result<super::AssignmentNonFungibleType, ::std::string::String>,
    }
    impl ::std::default::Default for AssignmentNonFungible {
        fn default() -> Self {
            Self {
                type_: Err("no value supplied for type_".to_string()),
            }
        }
    }
    impl AssignmentNonFungible {
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::AssignmentNonFungibleType>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<AssignmentNonFungible> for super::AssignmentNonFungible {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AssignmentNonFungible,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                type_: value.type_?,
            })
        }
    }
    impl ::std::convert::From<super::AssignmentNonFungible> for AssignmentNonFungible {
        fn from(value: super::AssignmentNonFungible) -> Self {
            Self {
                type_: Ok(value.type_),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct BackupRequest {
        backup_path: ::std::result::Result<::std::string::String, ::std::string::String>,
        password: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for BackupRequest {
        fn default() -> Self {
            Self {
                backup_path: Err("no value supplied for backup_path".to_string()),
                password: Err("no value supplied for password".to_string()),
            }
        }
    }
    impl BackupRequest {
        pub fn backup_path<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.backup_path = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for backup_path: {e}"));
            self
        }
        pub fn password<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.password = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for password: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<BackupRequest> for super::BackupRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: BackupRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                backup_path: value.backup_path?,
                password: value.password?,
            })
        }
    }
    impl ::std::convert::From<super::BackupRequest> for BackupRequest {
        fn from(value: super::BackupRequest) -> Self {
            Self {
                backup_path: Ok(value.backup_path),
                password: Ok(value.password),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct BlockTime {
        height: ::std::result::Result<i64, ::std::string::String>,
        timestamp: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for BlockTime {
        fn default() -> Self {
            Self {
                height: Err("no value supplied for height".to_string()),
                timestamp: Err("no value supplied for timestamp".to_string()),
            }
        }
    }
    impl BlockTime {
        pub fn height<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.height = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for height: {e}"));
            self
        }
        pub fn timestamp<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.timestamp = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for timestamp: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<BlockTime> for super::BlockTime {
        type Error = super::error::ConversionError;
        fn try_from(
            value: BlockTime,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                height: value.height?,
                timestamp: value.timestamp?,
            })
        }
    }
    impl ::std::convert::From<super::BlockTime> for BlockTime {
        fn from(value: super::BlockTime) -> Self {
            Self {
                height: Ok(value.height),
                timestamp: Ok(value.timestamp),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct BtcBalance {
        future: ::std::result::Result<i64, ::std::string::String>,
        settled: ::std::result::Result<i64, ::std::string::String>,
        spendable: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for BtcBalance {
        fn default() -> Self {
            Self {
                future: Err("no value supplied for future".to_string()),
                settled: Err("no value supplied for settled".to_string()),
                spendable: Err("no value supplied for spendable".to_string()),
            }
        }
    }
    impl BtcBalance {
        pub fn future<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.future = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for future: {e}"));
            self
        }
        pub fn settled<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.settled = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for settled: {e}"));
            self
        }
        pub fn spendable<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.spendable = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for spendable: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<BtcBalance> for super::BtcBalance {
        type Error = super::error::ConversionError;
        fn try_from(
            value: BtcBalance,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                future: value.future?,
                settled: value.settled?,
                spendable: value.spendable?,
            })
        }
    }
    impl ::std::convert::From<super::BtcBalance> for BtcBalance {
        fn from(value: super::BtcBalance) -> Self {
            Self {
                future: Ok(value.future),
                settled: Ok(value.settled),
                spendable: Ok(value.spendable),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct BtcBalanceRequest {
        skip_sync: ::std::result::Result<bool, ::std::string::String>,
    }
    impl ::std::default::Default for BtcBalanceRequest {
        fn default() -> Self {
            Self {
                skip_sync: Err("no value supplied for skip_sync".to_string()),
            }
        }
    }
    impl BtcBalanceRequest {
        pub fn skip_sync<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.skip_sync = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for skip_sync: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<BtcBalanceRequest> for super::BtcBalanceRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: BtcBalanceRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                skip_sync: value.skip_sync?,
            })
        }
    }
    impl ::std::convert::From<super::BtcBalanceRequest> for BtcBalanceRequest {
        fn from(value: super::BtcBalanceRequest) -> Self {
            Self {
                skip_sync: Ok(value.skip_sync),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct BtcBalanceResponse {
        colored: ::std::result::Result<super::BtcBalance, ::std::string::String>,
        vanilla: ::std::result::Result<super::BtcBalance, ::std::string::String>,
    }
    impl ::std::default::Default for BtcBalanceResponse {
        fn default() -> Self {
            Self {
                colored: Err("no value supplied for colored".to_string()),
                vanilla: Err("no value supplied for vanilla".to_string()),
            }
        }
    }
    impl BtcBalanceResponse {
        pub fn colored<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::BtcBalance>,
            T::Error: ::std::fmt::Display,
        {
            self.colored = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for colored: {e}"));
            self
        }
        pub fn vanilla<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::BtcBalance>,
            T::Error: ::std::fmt::Display,
        {
            self.vanilla = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for vanilla: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<BtcBalanceResponse> for super::BtcBalanceResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: BtcBalanceResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                colored: value.colored?,
                vanilla: value.vanilla?,
            })
        }
    }
    impl ::std::convert::From<super::BtcBalanceResponse> for BtcBalanceResponse {
        fn from(value: super::BtcBalanceResponse) -> Self {
            Self {
                colored: Ok(value.colored),
                vanilla: Ok(value.vanilla),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ChangePasswordRequest {
        new_password: ::std::result::Result<::std::string::String, ::std::string::String>,
        old_password: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for ChangePasswordRequest {
        fn default() -> Self {
            Self {
                new_password: Err("no value supplied for new_password".to_string()),
                old_password: Err("no value supplied for old_password".to_string()),
            }
        }
    }
    impl ChangePasswordRequest {
        pub fn new_password<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.new_password = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for new_password: {e}"));
            self
        }
        pub fn old_password<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.old_password = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for old_password: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ChangePasswordRequest> for super::ChangePasswordRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ChangePasswordRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                new_password: value.new_password?,
                old_password: value.old_password?,
            })
        }
    }
    impl ::std::convert::From<super::ChangePasswordRequest> for ChangePasswordRequest {
        fn from(value: super::ChangePasswordRequest) -> Self {
            Self {
                new_password: Ok(value.new_password),
                old_password: Ok(value.old_password),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Channel {
        asset_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        asset_local_amount:
            ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        asset_remote_amount:
            ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        capacity_sat: ::std::result::Result<i64, ::std::string::String>,
        channel_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        funding_txid: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        inbound_balance_msat: ::std::result::Result<i64, ::std::string::String>,
        is_usable: ::std::result::Result<bool, ::std::string::String>,
        local_balance_sat: ::std::result::Result<i64, ::std::string::String>,
        next_outbound_htlc_limit_msat: ::std::result::Result<i64, ::std::string::String>,
        next_outbound_htlc_minimum_msat: ::std::result::Result<i64, ::std::string::String>,
        outbound_balance_msat: ::std::result::Result<i64, ::std::string::String>,
        peer_alias: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        peer_pubkey: ::std::result::Result<::std::string::String, ::std::string::String>,
        public: ::std::result::Result<bool, ::std::string::String>,
        ready: ::std::result::Result<bool, ::std::string::String>,
        short_channel_id: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        status: ::std::result::Result<super::ChannelStatus, ::std::string::String>,
    }
    impl ::std::default::Default for Channel {
        fn default() -> Self {
            Self {
                asset_id: Ok(Default::default()),
                asset_local_amount: Ok(Default::default()),
                asset_remote_amount: Ok(Default::default()),
                capacity_sat: Err("no value supplied for capacity_sat".to_string()),
                channel_id: Err("no value supplied for channel_id".to_string()),
                funding_txid: Ok(Default::default()),
                inbound_balance_msat: Err("no value supplied for inbound_balance_msat".to_string()),
                is_usable: Err("no value supplied for is_usable".to_string()),
                local_balance_sat: Err("no value supplied for local_balance_sat".to_string()),
                next_outbound_htlc_limit_msat: Err(
                    "no value supplied for next_outbound_htlc_limit_msat".to_string(),
                ),
                next_outbound_htlc_minimum_msat: Err(
                    "no value supplied for next_outbound_htlc_minimum_msat".to_string(),
                ),
                outbound_balance_msat: Err(
                    "no value supplied for outbound_balance_msat".to_string()
                ),
                peer_alias: Ok(Default::default()),
                peer_pubkey: Err("no value supplied for peer_pubkey".to_string()),
                public: Err("no value supplied for public".to_string()),
                ready: Err("no value supplied for ready".to_string()),
                short_channel_id: Ok(Default::default()),
                status: Err("no value supplied for status".to_string()),
            }
        }
    }
    impl Channel {
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn asset_local_amount<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_local_amount = value.try_into().map_err(|e| {
                format!("error converting supplied value for asset_local_amount: {e}")
            });
            self
        }
        pub fn asset_remote_amount<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_remote_amount = value.try_into().map_err(|e| {
                format!("error converting supplied value for asset_remote_amount: {e}")
            });
            self
        }
        pub fn capacity_sat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.capacity_sat = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for capacity_sat: {e}"));
            self
        }
        pub fn channel_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.channel_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for channel_id: {e}"));
            self
        }
        pub fn funding_txid<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.funding_txid = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for funding_txid: {e}"));
            self
        }
        pub fn inbound_balance_msat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.inbound_balance_msat = value.try_into().map_err(|e| {
                format!("error converting supplied value for inbound_balance_msat: {e}")
            });
            self
        }
        pub fn is_usable<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.is_usable = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for is_usable: {e}"));
            self
        }
        pub fn local_balance_sat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.local_balance_sat = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for local_balance_sat: {e}"));
            self
        }
        pub fn next_outbound_htlc_limit_msat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.next_outbound_htlc_limit_msat = value.try_into().map_err(|e| {
                format!("error converting supplied value for next_outbound_htlc_limit_msat: {e}")
            });
            self
        }
        pub fn next_outbound_htlc_minimum_msat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.next_outbound_htlc_minimum_msat = value.try_into().map_err(|e| {
                format!("error converting supplied value for next_outbound_htlc_minimum_msat: {e}")
            });
            self
        }
        pub fn outbound_balance_msat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.outbound_balance_msat = value.try_into().map_err(|e| {
                format!("error converting supplied value for outbound_balance_msat: {e}")
            });
            self
        }
        pub fn peer_alias<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.peer_alias = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for peer_alias: {e}"));
            self
        }
        pub fn peer_pubkey<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.peer_pubkey = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for peer_pubkey: {e}"));
            self
        }
        pub fn public<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.public = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for public: {e}"));
            self
        }
        pub fn ready<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.ready = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for ready: {e}"));
            self
        }
        pub fn short_channel_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.short_channel_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for short_channel_id: {e}"));
            self
        }
        pub fn status<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::ChannelStatus>,
            T::Error: ::std::fmt::Display,
        {
            self.status = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for status: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Channel> for super::Channel {
        type Error = super::error::ConversionError;
        fn try_from(value: Channel) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset_id: value.asset_id?,
                asset_local_amount: value.asset_local_amount?,
                asset_remote_amount: value.asset_remote_amount?,
                capacity_sat: value.capacity_sat?,
                channel_id: value.channel_id?,
                funding_txid: value.funding_txid?,
                inbound_balance_msat: value.inbound_balance_msat?,
                is_usable: value.is_usable?,
                local_balance_sat: value.local_balance_sat?,
                next_outbound_htlc_limit_msat: value.next_outbound_htlc_limit_msat?,
                next_outbound_htlc_minimum_msat: value.next_outbound_htlc_minimum_msat?,
                outbound_balance_msat: value.outbound_balance_msat?,
                peer_alias: value.peer_alias?,
                peer_pubkey: value.peer_pubkey?,
                public: value.public?,
                ready: value.ready?,
                short_channel_id: value.short_channel_id?,
                status: value.status?,
            })
        }
    }
    impl ::std::convert::From<super::Channel> for Channel {
        fn from(value: super::Channel) -> Self {
            Self {
                asset_id: Ok(value.asset_id),
                asset_local_amount: Ok(value.asset_local_amount),
                asset_remote_amount: Ok(value.asset_remote_amount),
                capacity_sat: Ok(value.capacity_sat),
                channel_id: Ok(value.channel_id),
                funding_txid: Ok(value.funding_txid),
                inbound_balance_msat: Ok(value.inbound_balance_msat),
                is_usable: Ok(value.is_usable),
                local_balance_sat: Ok(value.local_balance_sat),
                next_outbound_htlc_limit_msat: Ok(value.next_outbound_htlc_limit_msat),
                next_outbound_htlc_minimum_msat: Ok(value.next_outbound_htlc_minimum_msat),
                outbound_balance_msat: Ok(value.outbound_balance_msat),
                peer_alias: Ok(value.peer_alias),
                peer_pubkey: Ok(value.peer_pubkey),
                public: Ok(value.public),
                ready: Ok(value.ready),
                short_channel_id: Ok(value.short_channel_id),
                status: Ok(value.status),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct CheckIndexerUrlRequest {
        indexer_url: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for CheckIndexerUrlRequest {
        fn default() -> Self {
            Self {
                indexer_url: Err("no value supplied for indexer_url".to_string()),
            }
        }
    }
    impl CheckIndexerUrlRequest {
        pub fn indexer_url<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.indexer_url = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for indexer_url: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<CheckIndexerUrlRequest> for super::CheckIndexerUrlRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: CheckIndexerUrlRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                indexer_url: value.indexer_url?,
            })
        }
    }
    impl ::std::convert::From<super::CheckIndexerUrlRequest> for CheckIndexerUrlRequest {
        fn from(value: super::CheckIndexerUrlRequest) -> Self {
            Self {
                indexer_url: Ok(value.indexer_url),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct CheckIndexerUrlResponse {
        indexer_protocol: ::std::result::Result<super::IndexerProtocol, ::std::string::String>,
    }
    impl ::std::default::Default for CheckIndexerUrlResponse {
        fn default() -> Self {
            Self {
                indexer_protocol: Err("no value supplied for indexer_protocol".to_string()),
            }
        }
    }
    impl CheckIndexerUrlResponse {
        pub fn indexer_protocol<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::IndexerProtocol>,
            T::Error: ::std::fmt::Display,
        {
            self.indexer_protocol = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for indexer_protocol: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<CheckIndexerUrlResponse> for super::CheckIndexerUrlResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: CheckIndexerUrlResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                indexer_protocol: value.indexer_protocol?,
            })
        }
    }
    impl ::std::convert::From<super::CheckIndexerUrlResponse> for CheckIndexerUrlResponse {
        fn from(value: super::CheckIndexerUrlResponse) -> Self {
            Self {
                indexer_protocol: Ok(value.indexer_protocol),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct CheckProxyEndpointRequest {
        proxy_endpoint: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for CheckProxyEndpointRequest {
        fn default() -> Self {
            Self {
                proxy_endpoint: Err("no value supplied for proxy_endpoint".to_string()),
            }
        }
    }
    impl CheckProxyEndpointRequest {
        pub fn proxy_endpoint<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.proxy_endpoint = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for proxy_endpoint: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<CheckProxyEndpointRequest> for super::CheckProxyEndpointRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: CheckProxyEndpointRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                proxy_endpoint: value.proxy_endpoint?,
            })
        }
    }
    impl ::std::convert::From<super::CheckProxyEndpointRequest> for CheckProxyEndpointRequest {
        fn from(value: super::CheckProxyEndpointRequest) -> Self {
            Self {
                proxy_endpoint: Ok(value.proxy_endpoint),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct CloseChannelRequest {
        channel_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        force: ::std::result::Result<bool, ::std::string::String>,
        peer_pubkey: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for CloseChannelRequest {
        fn default() -> Self {
            Self {
                channel_id: Err("no value supplied for channel_id".to_string()),
                force: Err("no value supplied for force".to_string()),
                peer_pubkey: Err("no value supplied for peer_pubkey".to_string()),
            }
        }
    }
    impl CloseChannelRequest {
        pub fn channel_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.channel_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for channel_id: {e}"));
            self
        }
        pub fn force<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.force = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for force: {e}"));
            self
        }
        pub fn peer_pubkey<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.peer_pubkey = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for peer_pubkey: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<CloseChannelRequest> for super::CloseChannelRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: CloseChannelRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                channel_id: value.channel_id?,
                force: value.force?,
                peer_pubkey: value.peer_pubkey?,
            })
        }
    }
    impl ::std::convert::From<super::CloseChannelRequest> for CloseChannelRequest {
        fn from(value: super::CloseChannelRequest) -> Self {
            Self {
                channel_id: Ok(value.channel_id),
                force: Ok(value.force),
                peer_pubkey: Ok(value.peer_pubkey),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ConnectPeerRequest {
        peer_pubkey_and_addr: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for ConnectPeerRequest {
        fn default() -> Self {
            Self {
                peer_pubkey_and_addr: Err("no value supplied for peer_pubkey_and_addr".to_string()),
            }
        }
    }
    impl ConnectPeerRequest {
        pub fn peer_pubkey_and_addr<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.peer_pubkey_and_addr = value.try_into().map_err(|e| {
                format!("error converting supplied value for peer_pubkey_and_addr: {e}")
            });
            self
        }
    }
    impl ::std::convert::TryFrom<ConnectPeerRequest> for super::ConnectPeerRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ConnectPeerRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                peer_pubkey_and_addr: value.peer_pubkey_and_addr?,
            })
        }
    }
    impl ::std::convert::From<super::ConnectPeerRequest> for ConnectPeerRequest {
        fn from(value: super::ConnectPeerRequest) -> Self {
            Self {
                peer_pubkey_and_addr: Ok(value.peer_pubkey_and_addr),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct CreateUtxosRequest {
        fee_rate: ::std::result::Result<i64, ::std::string::String>,
        num: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        size: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        skip_sync: ::std::result::Result<bool, ::std::string::String>,
        up_to: ::std::result::Result<bool, ::std::string::String>,
    }
    impl ::std::default::Default for CreateUtxosRequest {
        fn default() -> Self {
            Self {
                fee_rate: Err("no value supplied for fee_rate".to_string()),
                num: Ok(Default::default()),
                size: Ok(Default::default()),
                skip_sync: Err("no value supplied for skip_sync".to_string()),
                up_to: Err("no value supplied for up_to".to_string()),
            }
        }
    }
    impl CreateUtxosRequest {
        pub fn fee_rate<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.fee_rate = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for fee_rate: {e}"));
            self
        }
        pub fn num<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.num = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for num: {e}"));
            self
        }
        pub fn size<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.size = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for size: {e}"));
            self
        }
        pub fn skip_sync<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.skip_sync = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for skip_sync: {e}"));
            self
        }
        pub fn up_to<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.up_to = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for up_to: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<CreateUtxosRequest> for super::CreateUtxosRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: CreateUtxosRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                fee_rate: value.fee_rate?,
                num: value.num?,
                size: value.size?,
                skip_sync: value.skip_sync?,
                up_to: value.up_to?,
            })
        }
    }
    impl ::std::convert::From<super::CreateUtxosRequest> for CreateUtxosRequest {
        fn from(value: super::CreateUtxosRequest) -> Self {
            Self {
                fee_rate: Ok(value.fee_rate),
                num: Ok(value.num),
                size: Ok(value.size),
                skip_sync: Ok(value.skip_sync),
                up_to: Ok(value.up_to),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DecodeLnInvoiceRequest {
        invoice: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for DecodeLnInvoiceRequest {
        fn default() -> Self {
            Self {
                invoice: Err("no value supplied for invoice".to_string()),
            }
        }
    }
    impl DecodeLnInvoiceRequest {
        pub fn invoice<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.invoice = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for invoice: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DecodeLnInvoiceRequest> for super::DecodeLnInvoiceRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DecodeLnInvoiceRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                invoice: value.invoice?,
            })
        }
    }
    impl ::std::convert::From<super::DecodeLnInvoiceRequest> for DecodeLnInvoiceRequest {
        fn from(value: super::DecodeLnInvoiceRequest) -> Self {
            Self {
                invoice: Ok(value.invoice),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DecodeLnInvoiceResponse {
        amt_msat: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        asset_amount: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        asset_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        expiry_sec: ::std::result::Result<i64, ::std::string::String>,
        network: ::std::result::Result<super::BitcoinNetwork, ::std::string::String>,
        payee_pubkey: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        payment_hash: ::std::result::Result<::std::string::String, ::std::string::String>,
        payment_secret: ::std::result::Result<::std::string::String, ::std::string::String>,
        timestamp: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for DecodeLnInvoiceResponse {
        fn default() -> Self {
            Self {
                amt_msat: Ok(Default::default()),
                asset_amount: Ok(Default::default()),
                asset_id: Ok(Default::default()),
                expiry_sec: Err("no value supplied for expiry_sec".to_string()),
                network: Err("no value supplied for network".to_string()),
                payee_pubkey: Ok(Default::default()),
                payment_hash: Err("no value supplied for payment_hash".to_string()),
                payment_secret: Err("no value supplied for payment_secret".to_string()),
                timestamp: Err("no value supplied for timestamp".to_string()),
            }
        }
    }
    impl DecodeLnInvoiceResponse {
        pub fn amt_msat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.amt_msat = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for amt_msat: {e}"));
            self
        }
        pub fn asset_amount<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_amount = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_amount: {e}"));
            self
        }
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn expiry_sec<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.expiry_sec = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for expiry_sec: {e}"));
            self
        }
        pub fn network<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::BitcoinNetwork>,
            T::Error: ::std::fmt::Display,
        {
            self.network = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for network: {e}"));
            self
        }
        pub fn payee_pubkey<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.payee_pubkey = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payee_pubkey: {e}"));
            self
        }
        pub fn payment_hash<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_hash = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_hash: {e}"));
            self
        }
        pub fn payment_secret<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_secret = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_secret: {e}"));
            self
        }
        pub fn timestamp<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.timestamp = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for timestamp: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DecodeLnInvoiceResponse> for super::DecodeLnInvoiceResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DecodeLnInvoiceResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                amt_msat: value.amt_msat?,
                asset_amount: value.asset_amount?,
                asset_id: value.asset_id?,
                expiry_sec: value.expiry_sec?,
                network: value.network?,
                payee_pubkey: value.payee_pubkey?,
                payment_hash: value.payment_hash?,
                payment_secret: value.payment_secret?,
                timestamp: value.timestamp?,
            })
        }
    }
    impl ::std::convert::From<super::DecodeLnInvoiceResponse> for DecodeLnInvoiceResponse {
        fn from(value: super::DecodeLnInvoiceResponse) -> Self {
            Self {
                amt_msat: Ok(value.amt_msat),
                asset_amount: Ok(value.asset_amount),
                asset_id: Ok(value.asset_id),
                expiry_sec: Ok(value.expiry_sec),
                network: Ok(value.network),
                payee_pubkey: Ok(value.payee_pubkey),
                payment_hash: Ok(value.payment_hash),
                payment_secret: Ok(value.payment_secret),
                timestamp: Ok(value.timestamp),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DecodeRgbInvoiceRequest {
        invoice: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for DecodeRgbInvoiceRequest {
        fn default() -> Self {
            Self {
                invoice: Err("no value supplied for invoice".to_string()),
            }
        }
    }
    impl DecodeRgbInvoiceRequest {
        pub fn invoice<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.invoice = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for invoice: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DecodeRgbInvoiceRequest> for super::DecodeRgbInvoiceRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DecodeRgbInvoiceRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                invoice: value.invoice?,
            })
        }
    }
    impl ::std::convert::From<super::DecodeRgbInvoiceRequest> for DecodeRgbInvoiceRequest {
        fn from(value: super::DecodeRgbInvoiceRequest) -> Self {
            Self {
                invoice: Ok(value.invoice),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DecodeRgbInvoiceResponse {
        asset_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        asset_schema:
            ::std::result::Result<::std::option::Option<super::AssetSchema>, ::std::string::String>,
        assignment: ::std::result::Result<super::Assignment, ::std::string::String>,
        expiration_timestamp:
            ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        network: ::std::result::Result<super::BitcoinNetwork, ::std::string::String>,
        recipient_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        recipient_type: ::std::result::Result<super::RecipientType, ::std::string::String>,
        transport_endpoints:
            ::std::result::Result<::std::vec::Vec<::std::string::String>, ::std::string::String>,
    }
    impl ::std::default::Default for DecodeRgbInvoiceResponse {
        fn default() -> Self {
            Self {
                asset_id: Ok(Default::default()),
                asset_schema: Ok(Default::default()),
                assignment: Err("no value supplied for assignment".to_string()),
                expiration_timestamp: Ok(Default::default()),
                network: Err("no value supplied for network".to_string()),
                recipient_id: Err("no value supplied for recipient_id".to_string()),
                recipient_type: Err("no value supplied for recipient_type".to_string()),
                transport_endpoints: Err("no value supplied for transport_endpoints".to_string()),
            }
        }
    }
    impl DecodeRgbInvoiceResponse {
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn asset_schema<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::AssetSchema>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_schema = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_schema: {e}"));
            self
        }
        pub fn assignment<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::Assignment>,
            T::Error: ::std::fmt::Display,
        {
            self.assignment = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for assignment: {e}"));
            self
        }
        pub fn expiration_timestamp<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.expiration_timestamp = value.try_into().map_err(|e| {
                format!("error converting supplied value for expiration_timestamp: {e}")
            });
            self
        }
        pub fn network<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::BitcoinNetwork>,
            T::Error: ::std::fmt::Display,
        {
            self.network = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for network: {e}"));
            self
        }
        pub fn recipient_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.recipient_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for recipient_id: {e}"));
            self
        }
        pub fn recipient_type<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::RecipientType>,
            T::Error: ::std::fmt::Display,
        {
            self.recipient_type = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for recipient_type: {e}"));
            self
        }
        pub fn transport_endpoints<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.transport_endpoints = value.try_into().map_err(|e| {
                format!("error converting supplied value for transport_endpoints: {e}")
            });
            self
        }
    }
    impl ::std::convert::TryFrom<DecodeRgbInvoiceResponse> for super::DecodeRgbInvoiceResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DecodeRgbInvoiceResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset_id: value.asset_id?,
                asset_schema: value.asset_schema?,
                assignment: value.assignment?,
                expiration_timestamp: value.expiration_timestamp?,
                network: value.network?,
                recipient_id: value.recipient_id?,
                recipient_type: value.recipient_type?,
                transport_endpoints: value.transport_endpoints?,
            })
        }
    }
    impl ::std::convert::From<super::DecodeRgbInvoiceResponse> for DecodeRgbInvoiceResponse {
        fn from(value: super::DecodeRgbInvoiceResponse) -> Self {
            Self {
                asset_id: Ok(value.asset_id),
                asset_schema: Ok(value.asset_schema),
                assignment: Ok(value.assignment),
                expiration_timestamp: Ok(value.expiration_timestamp),
                network: Ok(value.network),
                recipient_id: Ok(value.recipient_id),
                recipient_type: Ok(value.recipient_type),
                transport_endpoints: Ok(value.transport_endpoints),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DecodeSwapstringRequest {
        swapstring: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for DecodeSwapstringRequest {
        fn default() -> Self {
            Self {
                swapstring: Err("no value supplied for swapstring".to_string()),
            }
        }
    }
    impl DecodeSwapstringRequest {
        pub fn swapstring<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.swapstring = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for swapstring: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DecodeSwapstringRequest> for super::DecodeSwapstringRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DecodeSwapstringRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                swapstring: value.swapstring?,
            })
        }
    }
    impl ::std::convert::From<super::DecodeSwapstringRequest> for DecodeSwapstringRequest {
        fn from(value: super::DecodeSwapstringRequest) -> Self {
            Self {
                swapstring: Ok(value.swapstring),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DecodeSwapstringResponse {
        expiry: ::std::result::Result<i64, ::std::string::String>,
        from_asset: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        payment_hash: ::std::result::Result<::std::string::String, ::std::string::String>,
        qty_from: ::std::result::Result<i64, ::std::string::String>,
        qty_to: ::std::result::Result<i64, ::std::string::String>,
        to_asset: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for DecodeSwapstringResponse {
        fn default() -> Self {
            Self {
                expiry: Err("no value supplied for expiry".to_string()),
                from_asset: Ok(Default::default()),
                payment_hash: Err("no value supplied for payment_hash".to_string()),
                qty_from: Err("no value supplied for qty_from".to_string()),
                qty_to: Err("no value supplied for qty_to".to_string()),
                to_asset: Ok(Default::default()),
            }
        }
    }
    impl DecodeSwapstringResponse {
        pub fn expiry<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.expiry = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for expiry: {e}"));
            self
        }
        pub fn from_asset<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.from_asset = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for from_asset: {e}"));
            self
        }
        pub fn payment_hash<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_hash = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_hash: {e}"));
            self
        }
        pub fn qty_from<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.qty_from = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for qty_from: {e}"));
            self
        }
        pub fn qty_to<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.qty_to = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for qty_to: {e}"));
            self
        }
        pub fn to_asset<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.to_asset = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for to_asset: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DecodeSwapstringResponse> for super::DecodeSwapstringResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DecodeSwapstringResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                expiry: value.expiry?,
                from_asset: value.from_asset?,
                payment_hash: value.payment_hash?,
                qty_from: value.qty_from?,
                qty_to: value.qty_to?,
                to_asset: value.to_asset?,
            })
        }
    }
    impl ::std::convert::From<super::DecodeSwapstringResponse> for DecodeSwapstringResponse {
        fn from(value: super::DecodeSwapstringResponse) -> Self {
            Self {
                expiry: Ok(value.expiry),
                from_asset: Ok(value.from_asset),
                payment_hash: Ok(value.payment_hash),
                qty_from: Ok(value.qty_from),
                qty_to: Ok(value.qty_to),
                to_asset: Ok(value.to_asset),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DisconnectPeerRequest {
        peer_pubkey: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for DisconnectPeerRequest {
        fn default() -> Self {
            Self {
                peer_pubkey: Err("no value supplied for peer_pubkey".to_string()),
            }
        }
    }
    impl DisconnectPeerRequest {
        pub fn peer_pubkey<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.peer_pubkey = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for peer_pubkey: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<DisconnectPeerRequest> for super::DisconnectPeerRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: DisconnectPeerRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                peer_pubkey: value.peer_pubkey?,
            })
        }
    }
    impl ::std::convert::From<super::DisconnectPeerRequest> for DisconnectPeerRequest {
        fn from(value: super::DisconnectPeerRequest) -> Self {
            Self {
                peer_pubkey: Ok(value.peer_pubkey),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct EmbeddedMedia {
        data: ::std::result::Result<::std::vec::Vec<i64>, ::std::string::String>,
        mime: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for EmbeddedMedia {
        fn default() -> Self {
            Self {
                data: Err("no value supplied for data".to_string()),
                mime: Err("no value supplied for mime".to_string()),
            }
        }
    }
    impl EmbeddedMedia {
        pub fn data<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.data = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data: {e}"));
            self
        }
        pub fn mime<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.mime = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for mime: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<EmbeddedMedia> for super::EmbeddedMedia {
        type Error = super::error::ConversionError;
        fn try_from(
            value: EmbeddedMedia,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                data: value.data?,
                mime: value.mime?,
            })
        }
    }
    impl ::std::convert::From<super::EmbeddedMedia> for EmbeddedMedia {
        fn from(value: super::EmbeddedMedia) -> Self {
            Self {
                data: Ok(value.data),
                mime: Ok(value.mime),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct EstimateFeeRequest {
        blocks: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for EstimateFeeRequest {
        fn default() -> Self {
            Self {
                blocks: Err("no value supplied for blocks".to_string()),
            }
        }
    }
    impl EstimateFeeRequest {
        pub fn blocks<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.blocks = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for blocks: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<EstimateFeeRequest> for super::EstimateFeeRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: EstimateFeeRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                blocks: value.blocks?,
            })
        }
    }
    impl ::std::convert::From<super::EstimateFeeRequest> for EstimateFeeRequest {
        fn from(value: super::EstimateFeeRequest) -> Self {
            Self {
                blocks: Ok(value.blocks),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct EstimateFeeResponse {
        fee_rate: ::std::result::Result<f64, ::std::string::String>,
    }
    impl ::std::default::Default for EstimateFeeResponse {
        fn default() -> Self {
            Self {
                fee_rate: Err("no value supplied for fee_rate".to_string()),
            }
        }
    }
    impl EstimateFeeResponse {
        pub fn fee_rate<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<f64>,
            T::Error: ::std::fmt::Display,
        {
            self.fee_rate = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for fee_rate: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<EstimateFeeResponse> for super::EstimateFeeResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: EstimateFeeResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                fee_rate: value.fee_rate?,
            })
        }
    }
    impl ::std::convert::From<super::EstimateFeeResponse> for EstimateFeeResponse {
        fn from(value: super::EstimateFeeResponse) -> Self {
            Self {
                fee_rate: Ok(value.fee_rate),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct FailTransfersRequest {
        batch_transfer_idx:
            ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        no_asset_only: ::std::result::Result<bool, ::std::string::String>,
        skip_sync: ::std::result::Result<bool, ::std::string::String>,
    }
    impl ::std::default::Default for FailTransfersRequest {
        fn default() -> Self {
            Self {
                batch_transfer_idx: Ok(Default::default()),
                no_asset_only: Err("no value supplied for no_asset_only".to_string()),
                skip_sync: Err("no value supplied for skip_sync".to_string()),
            }
        }
    }
    impl FailTransfersRequest {
        pub fn batch_transfer_idx<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.batch_transfer_idx = value.try_into().map_err(|e| {
                format!("error converting supplied value for batch_transfer_idx: {e}")
            });
            self
        }
        pub fn no_asset_only<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.no_asset_only = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for no_asset_only: {e}"));
            self
        }
        pub fn skip_sync<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.skip_sync = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for skip_sync: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<FailTransfersRequest> for super::FailTransfersRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: FailTransfersRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                batch_transfer_idx: value.batch_transfer_idx?,
                no_asset_only: value.no_asset_only?,
                skip_sync: value.skip_sync?,
            })
        }
    }
    impl ::std::convert::From<super::FailTransfersRequest> for FailTransfersRequest {
        fn from(value: super::FailTransfersRequest) -> Self {
            Self {
                batch_transfer_idx: Ok(value.batch_transfer_idx),
                no_asset_only: Ok(value.no_asset_only),
                skip_sync: Ok(value.skip_sync),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct FailTransfersResponse {
        transfers_changed: ::std::result::Result<bool, ::std::string::String>,
    }
    impl ::std::default::Default for FailTransfersResponse {
        fn default() -> Self {
            Self {
                transfers_changed: Err("no value supplied for transfers_changed".to_string()),
            }
        }
    }
    impl FailTransfersResponse {
        pub fn transfers_changed<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.transfers_changed = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for transfers_changed: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<FailTransfersResponse> for super::FailTransfersResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: FailTransfersResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                transfers_changed: value.transfers_changed?,
            })
        }
    }
    impl ::std::convert::From<super::FailTransfersResponse> for FailTransfersResponse {
        fn from(value: super::FailTransfersResponse) -> Self {
            Self {
                transfers_changed: Ok(value.transfers_changed),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct GetAssetMediaRequest {
        digest: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for GetAssetMediaRequest {
        fn default() -> Self {
            Self {
                digest: Err("no value supplied for digest".to_string()),
            }
        }
    }
    impl GetAssetMediaRequest {
        pub fn digest<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.digest = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for digest: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<GetAssetMediaRequest> for super::GetAssetMediaRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GetAssetMediaRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                digest: value.digest?,
            })
        }
    }
    impl ::std::convert::From<super::GetAssetMediaRequest> for GetAssetMediaRequest {
        fn from(value: super::GetAssetMediaRequest) -> Self {
            Self {
                digest: Ok(value.digest),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct GetAssetMediaResponse {
        bytes_hex: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for GetAssetMediaResponse {
        fn default() -> Self {
            Self {
                bytes_hex: Err("no value supplied for bytes_hex".to_string()),
            }
        }
    }
    impl GetAssetMediaResponse {
        pub fn bytes_hex<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.bytes_hex = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for bytes_hex: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<GetAssetMediaResponse> for super::GetAssetMediaResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GetAssetMediaResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                bytes_hex: value.bytes_hex?,
            })
        }
    }
    impl ::std::convert::From<super::GetAssetMediaResponse> for GetAssetMediaResponse {
        fn from(value: super::GetAssetMediaResponse) -> Self {
            Self {
                bytes_hex: Ok(value.bytes_hex),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct GetChannelIdRequest {
        temporary_channel_id: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for GetChannelIdRequest {
        fn default() -> Self {
            Self {
                temporary_channel_id: Err("no value supplied for temporary_channel_id".to_string()),
            }
        }
    }
    impl GetChannelIdRequest {
        pub fn temporary_channel_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.temporary_channel_id = value.try_into().map_err(|e| {
                format!("error converting supplied value for temporary_channel_id: {e}")
            });
            self
        }
    }
    impl ::std::convert::TryFrom<GetChannelIdRequest> for super::GetChannelIdRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GetChannelIdRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                temporary_channel_id: value.temporary_channel_id?,
            })
        }
    }
    impl ::std::convert::From<super::GetChannelIdRequest> for GetChannelIdRequest {
        fn from(value: super::GetChannelIdRequest) -> Self {
            Self {
                temporary_channel_id: Ok(value.temporary_channel_id),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct GetChannelIdResponse {
        channel_id: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for GetChannelIdResponse {
        fn default() -> Self {
            Self {
                channel_id: Err("no value supplied for channel_id".to_string()),
            }
        }
    }
    impl GetChannelIdResponse {
        pub fn channel_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.channel_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for channel_id: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<GetChannelIdResponse> for super::GetChannelIdResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GetChannelIdResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                channel_id: value.channel_id?,
            })
        }
    }
    impl ::std::convert::From<super::GetChannelIdResponse> for GetChannelIdResponse {
        fn from(value: super::GetChannelIdResponse) -> Self {
            Self {
                channel_id: Ok(value.channel_id),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct GetPaymentRequest {
        payment_hash: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for GetPaymentRequest {
        fn default() -> Self {
            Self {
                payment_hash: Err("no value supplied for payment_hash".to_string()),
            }
        }
    }
    impl GetPaymentRequest {
        pub fn payment_hash<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_hash = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_hash: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<GetPaymentRequest> for super::GetPaymentRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GetPaymentRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                payment_hash: value.payment_hash?,
            })
        }
    }
    impl ::std::convert::From<super::GetPaymentRequest> for GetPaymentRequest {
        fn from(value: super::GetPaymentRequest) -> Self {
            Self {
                payment_hash: Ok(value.payment_hash),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct GetPaymentResponse {
        payment: ::std::result::Result<super::Payment, ::std::string::String>,
    }
    impl ::std::default::Default for GetPaymentResponse {
        fn default() -> Self {
            Self {
                payment: Err("no value supplied for payment".to_string()),
            }
        }
    }
    impl GetPaymentResponse {
        pub fn payment<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::Payment>,
            T::Error: ::std::fmt::Display,
        {
            self.payment = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<GetPaymentResponse> for super::GetPaymentResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GetPaymentResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                payment: value.payment?,
            })
        }
    }
    impl ::std::convert::From<super::GetPaymentResponse> for GetPaymentResponse {
        fn from(value: super::GetPaymentResponse) -> Self {
            Self {
                payment: Ok(value.payment),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct GetSwapRequest {
        payment_hash: ::std::result::Result<::std::string::String, ::std::string::String>,
        taker: ::std::result::Result<bool, ::std::string::String>,
    }
    impl ::std::default::Default for GetSwapRequest {
        fn default() -> Self {
            Self {
                payment_hash: Err("no value supplied for payment_hash".to_string()),
                taker: Err("no value supplied for taker".to_string()),
            }
        }
    }
    impl GetSwapRequest {
        pub fn payment_hash<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_hash = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_hash: {e}"));
            self
        }
        pub fn taker<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.taker = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for taker: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<GetSwapRequest> for super::GetSwapRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GetSwapRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                payment_hash: value.payment_hash?,
                taker: value.taker?,
            })
        }
    }
    impl ::std::convert::From<super::GetSwapRequest> for GetSwapRequest {
        fn from(value: super::GetSwapRequest) -> Self {
            Self {
                payment_hash: Ok(value.payment_hash),
                taker: Ok(value.taker),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct GetSwapResponse {
        swap: ::std::result::Result<super::Swap, ::std::string::String>,
    }
    impl ::std::default::Default for GetSwapResponse {
        fn default() -> Self {
            Self {
                swap: Err("no value supplied for swap".to_string()),
            }
        }
    }
    impl GetSwapResponse {
        pub fn swap<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::Swap>,
            T::Error: ::std::fmt::Display,
        {
            self.swap = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for swap: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<GetSwapResponse> for super::GetSwapResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GetSwapResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self { swap: value.swap? })
        }
    }
    impl ::std::convert::From<super::GetSwapResponse> for GetSwapResponse {
        fn from(value: super::GetSwapResponse) -> Self {
            Self {
                swap: Ok(value.swap),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct InflateRequest {
        asset_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        fee_rate: ::std::result::Result<i64, ::std::string::String>,
        inflation_amounts: ::std::result::Result<::std::vec::Vec<i64>, ::std::string::String>,
        min_confirmations: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for InflateRequest {
        fn default() -> Self {
            Self {
                asset_id: Err("no value supplied for asset_id".to_string()),
                fee_rate: Err("no value supplied for fee_rate".to_string()),
                inflation_amounts: Err("no value supplied for inflation_amounts".to_string()),
                min_confirmations: Err("no value supplied for min_confirmations".to_string()),
            }
        }
    }
    impl InflateRequest {
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn fee_rate<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.fee_rate = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for fee_rate: {e}"));
            self
        }
        pub fn inflation_amounts<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.inflation_amounts = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for inflation_amounts: {e}"));
            self
        }
        pub fn min_confirmations<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.min_confirmations = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for min_confirmations: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<InflateRequest> for super::InflateRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: InflateRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset_id: value.asset_id?,
                fee_rate: value.fee_rate?,
                inflation_amounts: value.inflation_amounts?,
                min_confirmations: value.min_confirmations?,
            })
        }
    }
    impl ::std::convert::From<super::InflateRequest> for InflateRequest {
        fn from(value: super::InflateRequest) -> Self {
            Self {
                asset_id: Ok(value.asset_id),
                fee_rate: Ok(value.fee_rate),
                inflation_amounts: Ok(value.inflation_amounts),
                min_confirmations: Ok(value.min_confirmations),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct InflateResponse {
        txid: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for InflateResponse {
        fn default() -> Self {
            Self {
                txid: Err("no value supplied for txid".to_string()),
            }
        }
    }
    impl InflateResponse {
        pub fn txid<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.txid = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for txid: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<InflateResponse> for super::InflateResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: InflateResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self { txid: value.txid? })
        }
    }
    impl ::std::convert::From<super::InflateResponse> for InflateResponse {
        fn from(value: super::InflateResponse) -> Self {
            Self {
                txid: Ok(value.txid),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct InitRequest {
        mnemonic: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        password: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for InitRequest {
        fn default() -> Self {
            Self {
                mnemonic: Ok(Default::default()),
                password: Err("no value supplied for password".to_string()),
            }
        }
    }
    impl InitRequest {
        pub fn mnemonic<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.mnemonic = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for mnemonic: {e}"));
            self
        }
        pub fn password<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.password = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for password: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<InitRequest> for super::InitRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: InitRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                mnemonic: value.mnemonic?,
                password: value.password?,
            })
        }
    }
    impl ::std::convert::From<super::InitRequest> for InitRequest {
        fn from(value: super::InitRequest) -> Self {
            Self {
                mnemonic: Ok(value.mnemonic),
                password: Ok(value.password),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct InitResponse {
        mnemonic: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for InitResponse {
        fn default() -> Self {
            Self {
                mnemonic: Err("no value supplied for mnemonic".to_string()),
            }
        }
    }
    impl InitResponse {
        pub fn mnemonic<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.mnemonic = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for mnemonic: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<InitResponse> for super::InitResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: InitResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                mnemonic: value.mnemonic?,
            })
        }
    }
    impl ::std::convert::From<super::InitResponse> for InitResponse {
        fn from(value: super::InitResponse) -> Self {
            Self {
                mnemonic: Ok(value.mnemonic),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct InvoiceStatusRequest {
        invoice: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for InvoiceStatusRequest {
        fn default() -> Self {
            Self {
                invoice: Err("no value supplied for invoice".to_string()),
            }
        }
    }
    impl InvoiceStatusRequest {
        pub fn invoice<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.invoice = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for invoice: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<InvoiceStatusRequest> for super::InvoiceStatusRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: InvoiceStatusRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                invoice: value.invoice?,
            })
        }
    }
    impl ::std::convert::From<super::InvoiceStatusRequest> for InvoiceStatusRequest {
        fn from(value: super::InvoiceStatusRequest) -> Self {
            Self {
                invoice: Ok(value.invoice),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct InvoiceStatusResponse {
        status: ::std::result::Result<super::InvoiceStatus, ::std::string::String>,
    }
    impl ::std::default::Default for InvoiceStatusResponse {
        fn default() -> Self {
            Self {
                status: Err("no value supplied for status".to_string()),
            }
        }
    }
    impl InvoiceStatusResponse {
        pub fn status<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::InvoiceStatus>,
            T::Error: ::std::fmt::Display,
        {
            self.status = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for status: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<InvoiceStatusResponse> for super::InvoiceStatusResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: InvoiceStatusResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                status: value.status?,
            })
        }
    }
    impl ::std::convert::From<super::InvoiceStatusResponse> for InvoiceStatusResponse {
        fn from(value: super::InvoiceStatusResponse) -> Self {
            Self {
                status: Ok(value.status),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct IssueAssetCfaRequest {
        amounts: ::std::result::Result<::std::vec::Vec<i64>, ::std::string::String>,
        details: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        file_digest: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        precision: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for IssueAssetCfaRequest {
        fn default() -> Self {
            Self {
                amounts: Err("no value supplied for amounts".to_string()),
                details: Ok(Default::default()),
                file_digest: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                precision: Err("no value supplied for precision".to_string()),
            }
        }
    }
    impl IssueAssetCfaRequest {
        pub fn amounts<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.amounts = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for amounts: {e}"));
            self
        }
        pub fn details<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.details = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for details: {e}"));
            self
        }
        pub fn file_digest<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.file_digest = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for file_digest: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn precision<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.precision = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for precision: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<IssueAssetCfaRequest> for super::IssueAssetCfaRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: IssueAssetCfaRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                amounts: value.amounts?,
                details: value.details?,
                file_digest: value.file_digest?,
                name: value.name?,
                precision: value.precision?,
            })
        }
    }
    impl ::std::convert::From<super::IssueAssetCfaRequest> for IssueAssetCfaRequest {
        fn from(value: super::IssueAssetCfaRequest) -> Self {
            Self {
                amounts: Ok(value.amounts),
                details: Ok(value.details),
                file_digest: Ok(value.file_digest),
                name: Ok(value.name),
                precision: Ok(value.precision),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct IssueAssetCfaResponse {
        asset: ::std::result::Result<super::AssetCfa, ::std::string::String>,
    }
    impl ::std::default::Default for IssueAssetCfaResponse {
        fn default() -> Self {
            Self {
                asset: Err("no value supplied for asset".to_string()),
            }
        }
    }
    impl IssueAssetCfaResponse {
        pub fn asset<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::AssetCfa>,
            T::Error: ::std::fmt::Display,
        {
            self.asset = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<IssueAssetCfaResponse> for super::IssueAssetCfaResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: IssueAssetCfaResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset: value.asset?,
            })
        }
    }
    impl ::std::convert::From<super::IssueAssetCfaResponse> for IssueAssetCfaResponse {
        fn from(value: super::IssueAssetCfaResponse) -> Self {
            Self {
                asset: Ok(value.asset),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct IssueAssetIfaRequest {
        amounts: ::std::result::Result<::std::vec::Vec<i64>, ::std::string::String>,
        inflation_amounts: ::std::result::Result<::std::vec::Vec<i64>, ::std::string::String>,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        precision: ::std::result::Result<i64, ::std::string::String>,
        reject_list_url: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        ticker: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for IssueAssetIfaRequest {
        fn default() -> Self {
            Self {
                amounts: Err("no value supplied for amounts".to_string()),
                inflation_amounts: Err("no value supplied for inflation_amounts".to_string()),
                name: Err("no value supplied for name".to_string()),
                precision: Err("no value supplied for precision".to_string()),
                reject_list_url: Ok(Default::default()),
                ticker: Err("no value supplied for ticker".to_string()),
            }
        }
    }
    impl IssueAssetIfaRequest {
        pub fn amounts<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.amounts = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for amounts: {e}"));
            self
        }
        pub fn inflation_amounts<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.inflation_amounts = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for inflation_amounts: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn precision<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.precision = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for precision: {e}"));
            self
        }
        pub fn reject_list_url<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.reject_list_url = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for reject_list_url: {e}"));
            self
        }
        pub fn ticker<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.ticker = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for ticker: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<IssueAssetIfaRequest> for super::IssueAssetIfaRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: IssueAssetIfaRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                amounts: value.amounts?,
                inflation_amounts: value.inflation_amounts?,
                name: value.name?,
                precision: value.precision?,
                reject_list_url: value.reject_list_url?,
                ticker: value.ticker?,
            })
        }
    }
    impl ::std::convert::From<super::IssueAssetIfaRequest> for IssueAssetIfaRequest {
        fn from(value: super::IssueAssetIfaRequest) -> Self {
            Self {
                amounts: Ok(value.amounts),
                inflation_amounts: Ok(value.inflation_amounts),
                name: Ok(value.name),
                precision: Ok(value.precision),
                reject_list_url: Ok(value.reject_list_url),
                ticker: Ok(value.ticker),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct IssueAssetIfaResponse {
        asset: ::std::result::Result<super::AssetIfa, ::std::string::String>,
    }
    impl ::std::default::Default for IssueAssetIfaResponse {
        fn default() -> Self {
            Self {
                asset: Err("no value supplied for asset".to_string()),
            }
        }
    }
    impl IssueAssetIfaResponse {
        pub fn asset<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::AssetIfa>,
            T::Error: ::std::fmt::Display,
        {
            self.asset = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<IssueAssetIfaResponse> for super::IssueAssetIfaResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: IssueAssetIfaResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset: value.asset?,
            })
        }
    }
    impl ::std::convert::From<super::IssueAssetIfaResponse> for IssueAssetIfaResponse {
        fn from(value: super::IssueAssetIfaResponse) -> Self {
            Self {
                asset: Ok(value.asset),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct IssueAssetNiaRequest {
        amounts: ::std::result::Result<::std::vec::Vec<i64>, ::std::string::String>,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        precision: ::std::result::Result<i64, ::std::string::String>,
        ticker: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for IssueAssetNiaRequest {
        fn default() -> Self {
            Self {
                amounts: Err("no value supplied for amounts".to_string()),
                name: Err("no value supplied for name".to_string()),
                precision: Err("no value supplied for precision".to_string()),
                ticker: Err("no value supplied for ticker".to_string()),
            }
        }
    }
    impl IssueAssetNiaRequest {
        pub fn amounts<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.amounts = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for amounts: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn precision<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.precision = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for precision: {e}"));
            self
        }
        pub fn ticker<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.ticker = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for ticker: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<IssueAssetNiaRequest> for super::IssueAssetNiaRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: IssueAssetNiaRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                amounts: value.amounts?,
                name: value.name?,
                precision: value.precision?,
                ticker: value.ticker?,
            })
        }
    }
    impl ::std::convert::From<super::IssueAssetNiaRequest> for IssueAssetNiaRequest {
        fn from(value: super::IssueAssetNiaRequest) -> Self {
            Self {
                amounts: Ok(value.amounts),
                name: Ok(value.name),
                precision: Ok(value.precision),
                ticker: Ok(value.ticker),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct IssueAssetNiaResponse {
        asset: ::std::result::Result<super::AssetNia, ::std::string::String>,
    }
    impl ::std::default::Default for IssueAssetNiaResponse {
        fn default() -> Self {
            Self {
                asset: Err("no value supplied for asset".to_string()),
            }
        }
    }
    impl IssueAssetNiaResponse {
        pub fn asset<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::AssetNia>,
            T::Error: ::std::fmt::Display,
        {
            self.asset = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<IssueAssetNiaResponse> for super::IssueAssetNiaResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: IssueAssetNiaResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset: value.asset?,
            })
        }
    }
    impl ::std::convert::From<super::IssueAssetNiaResponse> for IssueAssetNiaResponse {
        fn from(value: super::IssueAssetNiaResponse) -> Self {
            Self {
                asset: Ok(value.asset),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct IssueAssetUdaRequest {
        attachments_file_digests:
            ::std::result::Result<::std::vec::Vec<::std::string::String>, ::std::string::String>,
        details: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        media_file_digest: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        precision: ::std::result::Result<i64, ::std::string::String>,
        ticker: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for IssueAssetUdaRequest {
        fn default() -> Self {
            Self {
                attachments_file_digests: Err(
                    "no value supplied for attachments_file_digests".to_string()
                ),
                details: Ok(Default::default()),
                media_file_digest: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                precision: Err("no value supplied for precision".to_string()),
                ticker: Err("no value supplied for ticker".to_string()),
            }
        }
    }
    impl IssueAssetUdaRequest {
        pub fn attachments_file_digests<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.attachments_file_digests = value.try_into().map_err(|e| {
                format!("error converting supplied value for attachments_file_digests: {e}")
            });
            self
        }
        pub fn details<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.details = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for details: {e}"));
            self
        }
        pub fn media_file_digest<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.media_file_digest = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for media_file_digest: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn precision<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.precision = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for precision: {e}"));
            self
        }
        pub fn ticker<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.ticker = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for ticker: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<IssueAssetUdaRequest> for super::IssueAssetUdaRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: IssueAssetUdaRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                attachments_file_digests: value.attachments_file_digests?,
                details: value.details?,
                media_file_digest: value.media_file_digest?,
                name: value.name?,
                precision: value.precision?,
                ticker: value.ticker?,
            })
        }
    }
    impl ::std::convert::From<super::IssueAssetUdaRequest> for IssueAssetUdaRequest {
        fn from(value: super::IssueAssetUdaRequest) -> Self {
            Self {
                attachments_file_digests: Ok(value.attachments_file_digests),
                details: Ok(value.details),
                media_file_digest: Ok(value.media_file_digest),
                name: Ok(value.name),
                precision: Ok(value.precision),
                ticker: Ok(value.ticker),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct IssueAssetUdaResponse {
        asset: ::std::result::Result<super::AssetUda, ::std::string::String>,
    }
    impl ::std::default::Default for IssueAssetUdaResponse {
        fn default() -> Self {
            Self {
                asset: Err("no value supplied for asset".to_string()),
            }
        }
    }
    impl IssueAssetUdaResponse {
        pub fn asset<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::AssetUda>,
            T::Error: ::std::fmt::Display,
        {
            self.asset = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<IssueAssetUdaResponse> for super::IssueAssetUdaResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: IssueAssetUdaResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset: value.asset?,
            })
        }
    }
    impl ::std::convert::From<super::IssueAssetUdaResponse> for IssueAssetUdaResponse {
        fn from(value: super::IssueAssetUdaResponse) -> Self {
            Self {
                asset: Ok(value.asset),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct KeysendRequest {
        amt_msat: ::std::result::Result<i64, ::std::string::String>,
        asset_amount: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        asset_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        dest_pubkey: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for KeysendRequest {
        fn default() -> Self {
            Self {
                amt_msat: Err("no value supplied for amt_msat".to_string()),
                asset_amount: Ok(Default::default()),
                asset_id: Ok(Default::default()),
                dest_pubkey: Err("no value supplied for dest_pubkey".to_string()),
            }
        }
    }
    impl KeysendRequest {
        pub fn amt_msat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.amt_msat = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for amt_msat: {e}"));
            self
        }
        pub fn asset_amount<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_amount = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_amount: {e}"));
            self
        }
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn dest_pubkey<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.dest_pubkey = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for dest_pubkey: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<KeysendRequest> for super::KeysendRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: KeysendRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                amt_msat: value.amt_msat?,
                asset_amount: value.asset_amount?,
                asset_id: value.asset_id?,
                dest_pubkey: value.dest_pubkey?,
            })
        }
    }
    impl ::std::convert::From<super::KeysendRequest> for KeysendRequest {
        fn from(value: super::KeysendRequest) -> Self {
            Self {
                amt_msat: Ok(value.amt_msat),
                asset_amount: Ok(value.asset_amount),
                asset_id: Ok(value.asset_id),
                dest_pubkey: Ok(value.dest_pubkey),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct KeysendResponse {
        payment_hash: ::std::result::Result<::std::string::String, ::std::string::String>,
        payment_preimage: ::std::result::Result<::std::string::String, ::std::string::String>,
        status: ::std::result::Result<super::HtlcStatus, ::std::string::String>,
    }
    impl ::std::default::Default for KeysendResponse {
        fn default() -> Self {
            Self {
                payment_hash: Err("no value supplied for payment_hash".to_string()),
                payment_preimage: Err("no value supplied for payment_preimage".to_string()),
                status: Err("no value supplied for status".to_string()),
            }
        }
    }
    impl KeysendResponse {
        pub fn payment_hash<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_hash = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_hash: {e}"));
            self
        }
        pub fn payment_preimage<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_preimage = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_preimage: {e}"));
            self
        }
        pub fn status<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::HtlcStatus>,
            T::Error: ::std::fmt::Display,
        {
            self.status = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for status: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<KeysendResponse> for super::KeysendResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: KeysendResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                payment_hash: value.payment_hash?,
                payment_preimage: value.payment_preimage?,
                status: value.status?,
            })
        }
    }
    impl ::std::convert::From<super::KeysendResponse> for KeysendResponse {
        fn from(value: super::KeysendResponse) -> Self {
            Self {
                payment_hash: Ok(value.payment_hash),
                payment_preimage: Ok(value.payment_preimage),
                status: Ok(value.status),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ListAssetsRequest {
        filter_asset_schemas:
            ::std::result::Result<::std::vec::Vec<super::AssetSchema>, ::std::string::String>,
    }
    impl ::std::default::Default for ListAssetsRequest {
        fn default() -> Self {
            Self {
                filter_asset_schemas: Err("no value supplied for filter_asset_schemas".to_string()),
            }
        }
    }
    impl ListAssetsRequest {
        pub fn filter_asset_schemas<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::AssetSchema>>,
            T::Error: ::std::fmt::Display,
        {
            self.filter_asset_schemas = value.try_into().map_err(|e| {
                format!("error converting supplied value for filter_asset_schemas: {e}")
            });
            self
        }
    }
    impl ::std::convert::TryFrom<ListAssetsRequest> for super::ListAssetsRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ListAssetsRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                filter_asset_schemas: value.filter_asset_schemas?,
            })
        }
    }
    impl ::std::convert::From<super::ListAssetsRequest> for ListAssetsRequest {
        fn from(value: super::ListAssetsRequest) -> Self {
            Self {
                filter_asset_schemas: Ok(value.filter_asset_schemas),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ListAssetsResponse {
        cfa: ::std::result::Result<
            ::std::option::Option<::std::vec::Vec<super::AssetCfa>>,
            ::std::string::String,
        >,
        ifa: ::std::result::Result<
            ::std::option::Option<::std::vec::Vec<super::AssetIfa>>,
            ::std::string::String,
        >,
        nia: ::std::result::Result<
            ::std::option::Option<::std::vec::Vec<super::AssetNia>>,
            ::std::string::String,
        >,
        uda: ::std::result::Result<
            ::std::option::Option<::std::vec::Vec<super::AssetUda>>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for ListAssetsResponse {
        fn default() -> Self {
            Self {
                cfa: Err("no value supplied for cfa".to_string()),
                ifa: Err("no value supplied for ifa".to_string()),
                nia: Err("no value supplied for nia".to_string()),
                uda: Err("no value supplied for uda".to_string()),
            }
        }
    }
    impl ListAssetsResponse {
        pub fn cfa<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::vec::Vec<super::AssetCfa>>>,
            T::Error: ::std::fmt::Display,
        {
            self.cfa = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for cfa: {e}"));
            self
        }
        pub fn ifa<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::vec::Vec<super::AssetIfa>>>,
            T::Error: ::std::fmt::Display,
        {
            self.ifa = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for ifa: {e}"));
            self
        }
        pub fn nia<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::vec::Vec<super::AssetNia>>>,
            T::Error: ::std::fmt::Display,
        {
            self.nia = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for nia: {e}"));
            self
        }
        pub fn uda<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::vec::Vec<super::AssetUda>>>,
            T::Error: ::std::fmt::Display,
        {
            self.uda = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for uda: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ListAssetsResponse> for super::ListAssetsResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ListAssetsResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                cfa: value.cfa?,
                ifa: value.ifa?,
                nia: value.nia?,
                uda: value.uda?,
            })
        }
    }
    impl ::std::convert::From<super::ListAssetsResponse> for ListAssetsResponse {
        fn from(value: super::ListAssetsResponse) -> Self {
            Self {
                cfa: Ok(value.cfa),
                ifa: Ok(value.ifa),
                nia: Ok(value.nia),
                uda: Ok(value.uda),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ListChannelsResponse {
        channels: ::std::result::Result<::std::vec::Vec<super::Channel>, ::std::string::String>,
    }
    impl ::std::default::Default for ListChannelsResponse {
        fn default() -> Self {
            Self {
                channels: Err("no value supplied for channels".to_string()),
            }
        }
    }
    impl ListChannelsResponse {
        pub fn channels<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Channel>>,
            T::Error: ::std::fmt::Display,
        {
            self.channels = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for channels: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ListChannelsResponse> for super::ListChannelsResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ListChannelsResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                channels: value.channels?,
            })
        }
    }
    impl ::std::convert::From<super::ListChannelsResponse> for ListChannelsResponse {
        fn from(value: super::ListChannelsResponse) -> Self {
            Self {
                channels: Ok(value.channels),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ListPaymentsResponse {
        payments: ::std::result::Result<::std::vec::Vec<super::Payment>, ::std::string::String>,
    }
    impl ::std::default::Default for ListPaymentsResponse {
        fn default() -> Self {
            Self {
                payments: Err("no value supplied for payments".to_string()),
            }
        }
    }
    impl ListPaymentsResponse {
        pub fn payments<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Payment>>,
            T::Error: ::std::fmt::Display,
        {
            self.payments = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payments: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ListPaymentsResponse> for super::ListPaymentsResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ListPaymentsResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                payments: value.payments?,
            })
        }
    }
    impl ::std::convert::From<super::ListPaymentsResponse> for ListPaymentsResponse {
        fn from(value: super::ListPaymentsResponse) -> Self {
            Self {
                payments: Ok(value.payments),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ListPeersResponse {
        peers: ::std::result::Result<::std::vec::Vec<super::Peer>, ::std::string::String>,
    }
    impl ::std::default::Default for ListPeersResponse {
        fn default() -> Self {
            Self {
                peers: Err("no value supplied for peers".to_string()),
            }
        }
    }
    impl ListPeersResponse {
        pub fn peers<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Peer>>,
            T::Error: ::std::fmt::Display,
        {
            self.peers = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for peers: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ListPeersResponse> for super::ListPeersResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ListPeersResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                peers: value.peers?,
            })
        }
    }
    impl ::std::convert::From<super::ListPeersResponse> for ListPeersResponse {
        fn from(value: super::ListPeersResponse) -> Self {
            Self {
                peers: Ok(value.peers),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ListSwapsResponse {
        maker: ::std::result::Result<::std::vec::Vec<super::Swap>, ::std::string::String>,
        taker: ::std::result::Result<::std::vec::Vec<super::Swap>, ::std::string::String>,
    }
    impl ::std::default::Default for ListSwapsResponse {
        fn default() -> Self {
            Self {
                maker: Err("no value supplied for maker".to_string()),
                taker: Err("no value supplied for taker".to_string()),
            }
        }
    }
    impl ListSwapsResponse {
        pub fn maker<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Swap>>,
            T::Error: ::std::fmt::Display,
        {
            self.maker = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for maker: {e}"));
            self
        }
        pub fn taker<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Swap>>,
            T::Error: ::std::fmt::Display,
        {
            self.taker = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for taker: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ListSwapsResponse> for super::ListSwapsResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ListSwapsResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                maker: value.maker?,
                taker: value.taker?,
            })
        }
    }
    impl ::std::convert::From<super::ListSwapsResponse> for ListSwapsResponse {
        fn from(value: super::ListSwapsResponse) -> Self {
            Self {
                maker: Ok(value.maker),
                taker: Ok(value.taker),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ListTransactionsRequest {
        skip_sync: ::std::result::Result<bool, ::std::string::String>,
    }
    impl ::std::default::Default for ListTransactionsRequest {
        fn default() -> Self {
            Self {
                skip_sync: Err("no value supplied for skip_sync".to_string()),
            }
        }
    }
    impl ListTransactionsRequest {
        pub fn skip_sync<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.skip_sync = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for skip_sync: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ListTransactionsRequest> for super::ListTransactionsRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ListTransactionsRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                skip_sync: value.skip_sync?,
            })
        }
    }
    impl ::std::convert::From<super::ListTransactionsRequest> for ListTransactionsRequest {
        fn from(value: super::ListTransactionsRequest) -> Self {
            Self {
                skip_sync: Ok(value.skip_sync),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ListTransactionsResponse {
        transactions:
            ::std::result::Result<::std::vec::Vec<super::Transaction>, ::std::string::String>,
    }
    impl ::std::default::Default for ListTransactionsResponse {
        fn default() -> Self {
            Self {
                transactions: Err("no value supplied for transactions".to_string()),
            }
        }
    }
    impl ListTransactionsResponse {
        pub fn transactions<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Transaction>>,
            T::Error: ::std::fmt::Display,
        {
            self.transactions = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for transactions: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ListTransactionsResponse> for super::ListTransactionsResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ListTransactionsResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                transactions: value.transactions?,
            })
        }
    }
    impl ::std::convert::From<super::ListTransactionsResponse> for ListTransactionsResponse {
        fn from(value: super::ListTransactionsResponse) -> Self {
            Self {
                transactions: Ok(value.transactions),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ListTransfersRequest {
        asset_id: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for ListTransfersRequest {
        fn default() -> Self {
            Self {
                asset_id: Err("no value supplied for asset_id".to_string()),
            }
        }
    }
    impl ListTransfersRequest {
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ListTransfersRequest> for super::ListTransfersRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ListTransfersRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset_id: value.asset_id?,
            })
        }
    }
    impl ::std::convert::From<super::ListTransfersRequest> for ListTransfersRequest {
        fn from(value: super::ListTransfersRequest) -> Self {
            Self {
                asset_id: Ok(value.asset_id),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ListTransfersResponse {
        transfers: ::std::result::Result<::std::vec::Vec<super::Transfer>, ::std::string::String>,
    }
    impl ::std::default::Default for ListTransfersResponse {
        fn default() -> Self {
            Self {
                transfers: Err("no value supplied for transfers".to_string()),
            }
        }
    }
    impl ListTransfersResponse {
        pub fn transfers<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Transfer>>,
            T::Error: ::std::fmt::Display,
        {
            self.transfers = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for transfers: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ListTransfersResponse> for super::ListTransfersResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ListTransfersResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                transfers: value.transfers?,
            })
        }
    }
    impl ::std::convert::From<super::ListTransfersResponse> for ListTransfersResponse {
        fn from(value: super::ListTransfersResponse) -> Self {
            Self {
                transfers: Ok(value.transfers),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ListUnspentsRequest {
        settled_only: ::std::result::Result<bool, ::std::string::String>,
        skip_sync: ::std::result::Result<bool, ::std::string::String>,
    }
    impl ::std::default::Default for ListUnspentsRequest {
        fn default() -> Self {
            Self {
                settled_only: Err("no value supplied for settled_only".to_string()),
                skip_sync: Err("no value supplied for skip_sync".to_string()),
            }
        }
    }
    impl ListUnspentsRequest {
        pub fn settled_only<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.settled_only = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for settled_only: {e}"));
            self
        }
        pub fn skip_sync<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.skip_sync = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for skip_sync: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ListUnspentsRequest> for super::ListUnspentsRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ListUnspentsRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                settled_only: value.settled_only?,
                skip_sync: value.skip_sync?,
            })
        }
    }
    impl ::std::convert::From<super::ListUnspentsRequest> for ListUnspentsRequest {
        fn from(value: super::ListUnspentsRequest) -> Self {
            Self {
                settled_only: Ok(value.settled_only),
                skip_sync: Ok(value.skip_sync),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ListUnspentsResponse {
        unspents: ::std::result::Result<::std::vec::Vec<super::Unspent>, ::std::string::String>,
    }
    impl ::std::default::Default for ListUnspentsResponse {
        fn default() -> Self {
            Self {
                unspents: Err("no value supplied for unspents".to_string()),
            }
        }
    }
    impl ListUnspentsResponse {
        pub fn unspents<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Unspent>>,
            T::Error: ::std::fmt::Display,
        {
            self.unspents = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for unspents: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ListUnspentsResponse> for super::ListUnspentsResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ListUnspentsResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                unspents: value.unspents?,
            })
        }
    }
    impl ::std::convert::From<super::ListUnspentsResponse> for ListUnspentsResponse {
        fn from(value: super::ListUnspentsResponse) -> Self {
            Self {
                unspents: Ok(value.unspents),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct LnInvoiceRequest {
        amt_msat: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        asset_amount: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        asset_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        expiry_sec: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for LnInvoiceRequest {
        fn default() -> Self {
            Self {
                amt_msat: Ok(Default::default()),
                asset_amount: Ok(Default::default()),
                asset_id: Ok(Default::default()),
                expiry_sec: Err("no value supplied for expiry_sec".to_string()),
            }
        }
    }
    impl LnInvoiceRequest {
        pub fn amt_msat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.amt_msat = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for amt_msat: {e}"));
            self
        }
        pub fn asset_amount<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_amount = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_amount: {e}"));
            self
        }
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn expiry_sec<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.expiry_sec = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for expiry_sec: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<LnInvoiceRequest> for super::LnInvoiceRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: LnInvoiceRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                amt_msat: value.amt_msat?,
                asset_amount: value.asset_amount?,
                asset_id: value.asset_id?,
                expiry_sec: value.expiry_sec?,
            })
        }
    }
    impl ::std::convert::From<super::LnInvoiceRequest> for LnInvoiceRequest {
        fn from(value: super::LnInvoiceRequest) -> Self {
            Self {
                amt_msat: Ok(value.amt_msat),
                asset_amount: Ok(value.asset_amount),
                asset_id: Ok(value.asset_id),
                expiry_sec: Ok(value.expiry_sec),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct LnInvoiceResponse {
        invoice: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for LnInvoiceResponse {
        fn default() -> Self {
            Self {
                invoice: Err("no value supplied for invoice".to_string()),
            }
        }
    }
    impl LnInvoiceResponse {
        pub fn invoice<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.invoice = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for invoice: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<LnInvoiceResponse> for super::LnInvoiceResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: LnInvoiceResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                invoice: value.invoice?,
            })
        }
    }
    impl ::std::convert::From<super::LnInvoiceResponse> for LnInvoiceResponse {
        fn from(value: super::LnInvoiceResponse) -> Self {
            Self {
                invoice: Ok(value.invoice),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct MakerExecuteRequest {
        payment_secret: ::std::result::Result<::std::string::String, ::std::string::String>,
        swapstring: ::std::result::Result<::std::string::String, ::std::string::String>,
        taker_pubkey: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for MakerExecuteRequest {
        fn default() -> Self {
            Self {
                payment_secret: Err("no value supplied for payment_secret".to_string()),
                swapstring: Err("no value supplied for swapstring".to_string()),
                taker_pubkey: Err("no value supplied for taker_pubkey".to_string()),
            }
        }
    }
    impl MakerExecuteRequest {
        pub fn payment_secret<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_secret = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_secret: {e}"));
            self
        }
        pub fn swapstring<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.swapstring = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for swapstring: {e}"));
            self
        }
        pub fn taker_pubkey<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.taker_pubkey = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for taker_pubkey: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<MakerExecuteRequest> for super::MakerExecuteRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: MakerExecuteRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                payment_secret: value.payment_secret?,
                swapstring: value.swapstring?,
                taker_pubkey: value.taker_pubkey?,
            })
        }
    }
    impl ::std::convert::From<super::MakerExecuteRequest> for MakerExecuteRequest {
        fn from(value: super::MakerExecuteRequest) -> Self {
            Self {
                payment_secret: Ok(value.payment_secret),
                swapstring: Ok(value.swapstring),
                taker_pubkey: Ok(value.taker_pubkey),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct MakerInitRequest {
        from_asset: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        qty_from: ::std::result::Result<i64, ::std::string::String>,
        qty_to: ::std::result::Result<i64, ::std::string::String>,
        timeout_sec: ::std::result::Result<i64, ::std::string::String>,
        to_asset: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for MakerInitRequest {
        fn default() -> Self {
            Self {
                from_asset: Ok(Default::default()),
                qty_from: Err("no value supplied for qty_from".to_string()),
                qty_to: Err("no value supplied for qty_to".to_string()),
                timeout_sec: Err("no value supplied for timeout_sec".to_string()),
                to_asset: Ok(Default::default()),
            }
        }
    }
    impl MakerInitRequest {
        pub fn from_asset<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.from_asset = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for from_asset: {e}"));
            self
        }
        pub fn qty_from<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.qty_from = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for qty_from: {e}"));
            self
        }
        pub fn qty_to<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.qty_to = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for qty_to: {e}"));
            self
        }
        pub fn timeout_sec<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.timeout_sec = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for timeout_sec: {e}"));
            self
        }
        pub fn to_asset<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.to_asset = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for to_asset: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<MakerInitRequest> for super::MakerInitRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: MakerInitRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                from_asset: value.from_asset?,
                qty_from: value.qty_from?,
                qty_to: value.qty_to?,
                timeout_sec: value.timeout_sec?,
                to_asset: value.to_asset?,
            })
        }
    }
    impl ::std::convert::From<super::MakerInitRequest> for MakerInitRequest {
        fn from(value: super::MakerInitRequest) -> Self {
            Self {
                from_asset: Ok(value.from_asset),
                qty_from: Ok(value.qty_from),
                qty_to: Ok(value.qty_to),
                timeout_sec: Ok(value.timeout_sec),
                to_asset: Ok(value.to_asset),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct MakerInitResponse {
        payment_hash: ::std::result::Result<::std::string::String, ::std::string::String>,
        payment_secret: ::std::result::Result<::std::string::String, ::std::string::String>,
        swapstring: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for MakerInitResponse {
        fn default() -> Self {
            Self {
                payment_hash: Err("no value supplied for payment_hash".to_string()),
                payment_secret: Err("no value supplied for payment_secret".to_string()),
                swapstring: Err("no value supplied for swapstring".to_string()),
            }
        }
    }
    impl MakerInitResponse {
        pub fn payment_hash<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_hash = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_hash: {e}"));
            self
        }
        pub fn payment_secret<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_secret = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_secret: {e}"));
            self
        }
        pub fn swapstring<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.swapstring = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for swapstring: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<MakerInitResponse> for super::MakerInitResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: MakerInitResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                payment_hash: value.payment_hash?,
                payment_secret: value.payment_secret?,
                swapstring: value.swapstring?,
            })
        }
    }
    impl ::std::convert::From<super::MakerInitResponse> for MakerInitResponse {
        fn from(value: super::MakerInitResponse) -> Self {
            Self {
                payment_hash: Ok(value.payment_hash),
                payment_secret: Ok(value.payment_secret),
                swapstring: Ok(value.swapstring),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Media {
        digest: ::std::result::Result<::std::string::String, ::std::string::String>,
        file_path: ::std::result::Result<::std::string::String, ::std::string::String>,
        mime: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for Media {
        fn default() -> Self {
            Self {
                digest: Err("no value supplied for digest".to_string()),
                file_path: Err("no value supplied for file_path".to_string()),
                mime: Err("no value supplied for mime".to_string()),
            }
        }
    }
    impl Media {
        pub fn digest<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.digest = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for digest: {e}"));
            self
        }
        pub fn file_path<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.file_path = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for file_path: {e}"));
            self
        }
        pub fn mime<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.mime = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for mime: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Media> for super::Media {
        type Error = super::error::ConversionError;
        fn try_from(value: Media) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                digest: value.digest?,
                file_path: value.file_path?,
                mime: value.mime?,
            })
        }
    }
    impl ::std::convert::From<super::Media> for Media {
        fn from(value: super::Media) -> Self {
            Self {
                digest: Ok(value.digest),
                file_path: Ok(value.file_path),
                mime: Ok(value.mime),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct NetworkInfoResponse {
        height: ::std::result::Result<i64, ::std::string::String>,
        network: ::std::result::Result<super::BitcoinNetwork, ::std::string::String>,
    }
    impl ::std::default::Default for NetworkInfoResponse {
        fn default() -> Self {
            Self {
                height: Err("no value supplied for height".to_string()),
                network: Err("no value supplied for network".to_string()),
            }
        }
    }
    impl NetworkInfoResponse {
        pub fn height<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.height = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for height: {e}"));
            self
        }
        pub fn network<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::BitcoinNetwork>,
            T::Error: ::std::fmt::Display,
        {
            self.network = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for network: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<NetworkInfoResponse> for super::NetworkInfoResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: NetworkInfoResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                height: value.height?,
                network: value.network?,
            })
        }
    }
    impl ::std::convert::From<super::NetworkInfoResponse> for NetworkInfoResponse {
        fn from(value: super::NetworkInfoResponse) -> Self {
            Self {
                height: Ok(value.height),
                network: Ok(value.network),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct NodeInfoResponse {
        account_xpub_colored: ::std::result::Result<::std::string::String, ::std::string::String>,
        account_xpub_vanilla: ::std::result::Result<::std::string::String, ::std::string::String>,
        channel_asset_max_amount: ::std::result::Result<u64, ::std::string::String>,
        channel_asset_min_amount: ::std::result::Result<u64, ::std::string::String>,
        channel_capacity_max_sat: ::std::result::Result<i64, ::std::string::String>,
        channel_capacity_min_sat: ::std::result::Result<i64, ::std::string::String>,
        eventual_close_fees_sat: ::std::result::Result<i64, ::std::string::String>,
        local_balance_sat: ::std::result::Result<i64, ::std::string::String>,
        max_media_upload_size_mb: ::std::result::Result<i64, ::std::string::String>,
        network_channels: ::std::result::Result<i64, ::std::string::String>,
        network_nodes: ::std::result::Result<i64, ::std::string::String>,
        num_channels: ::std::result::Result<i64, ::std::string::String>,
        num_peers: ::std::result::Result<i64, ::std::string::String>,
        num_usable_channels: ::std::result::Result<i64, ::std::string::String>,
        pending_outbound_payments_sat: ::std::result::Result<i64, ::std::string::String>,
        pubkey: ::std::result::Result<::std::string::String, ::std::string::String>,
        rgb_channel_capacity_min_sat: ::std::result::Result<i64, ::std::string::String>,
        rgb_htlc_min_msat: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for NodeInfoResponse {
        fn default() -> Self {
            Self {
                account_xpub_colored: Err("no value supplied for account_xpub_colored".to_string()),
                account_xpub_vanilla: Err("no value supplied for account_xpub_vanilla".to_string()),
                channel_asset_max_amount: Err(
                    "no value supplied for channel_asset_max_amount".to_string()
                ),
                channel_asset_min_amount: Err(
                    "no value supplied for channel_asset_min_amount".to_string()
                ),
                channel_capacity_max_sat: Err(
                    "no value supplied for channel_capacity_max_sat".to_string()
                ),
                channel_capacity_min_sat: Err(
                    "no value supplied for channel_capacity_min_sat".to_string()
                ),
                eventual_close_fees_sat: Err(
                    "no value supplied for eventual_close_fees_sat".to_string()
                ),
                local_balance_sat: Err("no value supplied for local_balance_sat".to_string()),
                max_media_upload_size_mb: Err(
                    "no value supplied for max_media_upload_size_mb".to_string()
                ),
                network_channels: Err("no value supplied for network_channels".to_string()),
                network_nodes: Err("no value supplied for network_nodes".to_string()),
                num_channels: Err("no value supplied for num_channels".to_string()),
                num_peers: Err("no value supplied for num_peers".to_string()),
                num_usable_channels: Err("no value supplied for num_usable_channels".to_string()),
                pending_outbound_payments_sat: Err(
                    "no value supplied for pending_outbound_payments_sat".to_string(),
                ),
                pubkey: Err("no value supplied for pubkey".to_string()),
                rgb_channel_capacity_min_sat: Err(
                    "no value supplied for rgb_channel_capacity_min_sat".to_string(),
                ),
                rgb_htlc_min_msat: Err("no value supplied for rgb_htlc_min_msat".to_string()),
            }
        }
    }
    impl NodeInfoResponse {
        pub fn account_xpub_colored<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.account_xpub_colored = value.try_into().map_err(|e| {
                format!("error converting supplied value for account_xpub_colored: {e}")
            });
            self
        }
        pub fn account_xpub_vanilla<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.account_xpub_vanilla = value.try_into().map_err(|e| {
                format!("error converting supplied value for account_xpub_vanilla: {e}")
            });
            self
        }
        pub fn channel_asset_max_amount<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<u64>,
            T::Error: ::std::fmt::Display,
        {
            self.channel_asset_max_amount = value.try_into().map_err(|e| {
                format!("error converting supplied value for channel_asset_max_amount: {e}")
            });
            self
        }
        pub fn channel_asset_min_amount<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<u64>,
            T::Error: ::std::fmt::Display,
        {
            self.channel_asset_min_amount = value.try_into().map_err(|e| {
                format!("error converting supplied value for channel_asset_min_amount: {e}")
            });
            self
        }
        pub fn channel_capacity_max_sat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.channel_capacity_max_sat = value.try_into().map_err(|e| {
                format!("error converting supplied value for channel_capacity_max_sat: {e}")
            });
            self
        }
        pub fn channel_capacity_min_sat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.channel_capacity_min_sat = value.try_into().map_err(|e| {
                format!("error converting supplied value for channel_capacity_min_sat: {e}")
            });
            self
        }
        pub fn eventual_close_fees_sat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.eventual_close_fees_sat = value.try_into().map_err(|e| {
                format!("error converting supplied value for eventual_close_fees_sat: {e}")
            });
            self
        }
        pub fn local_balance_sat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.local_balance_sat = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for local_balance_sat: {e}"));
            self
        }
        pub fn max_media_upload_size_mb<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.max_media_upload_size_mb = value.try_into().map_err(|e| {
                format!("error converting supplied value for max_media_upload_size_mb: {e}")
            });
            self
        }
        pub fn network_channels<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.network_channels = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for network_channels: {e}"));
            self
        }
        pub fn network_nodes<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.network_nodes = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for network_nodes: {e}"));
            self
        }
        pub fn num_channels<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.num_channels = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for num_channels: {e}"));
            self
        }
        pub fn num_peers<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.num_peers = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for num_peers: {e}"));
            self
        }
        pub fn num_usable_channels<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.num_usable_channels = value.try_into().map_err(|e| {
                format!("error converting supplied value for num_usable_channels: {e}")
            });
            self
        }
        pub fn pending_outbound_payments_sat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.pending_outbound_payments_sat = value.try_into().map_err(|e| {
                format!("error converting supplied value for pending_outbound_payments_sat: {e}")
            });
            self
        }
        pub fn pubkey<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.pubkey = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for pubkey: {e}"));
            self
        }
        pub fn rgb_channel_capacity_min_sat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.rgb_channel_capacity_min_sat = value.try_into().map_err(|e| {
                format!("error converting supplied value for rgb_channel_capacity_min_sat: {e}")
            });
            self
        }
        pub fn rgb_htlc_min_msat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.rgb_htlc_min_msat = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for rgb_htlc_min_msat: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<NodeInfoResponse> for super::NodeInfoResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: NodeInfoResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                account_xpub_colored: value.account_xpub_colored?,
                account_xpub_vanilla: value.account_xpub_vanilla?,
                channel_asset_max_amount: value.channel_asset_max_amount?,
                channel_asset_min_amount: value.channel_asset_min_amount?,
                channel_capacity_max_sat: value.channel_capacity_max_sat?,
                channel_capacity_min_sat: value.channel_capacity_min_sat?,
                eventual_close_fees_sat: value.eventual_close_fees_sat?,
                local_balance_sat: value.local_balance_sat?,
                max_media_upload_size_mb: value.max_media_upload_size_mb?,
                network_channels: value.network_channels?,
                network_nodes: value.network_nodes?,
                num_channels: value.num_channels?,
                num_peers: value.num_peers?,
                num_usable_channels: value.num_usable_channels?,
                pending_outbound_payments_sat: value.pending_outbound_payments_sat?,
                pubkey: value.pubkey?,
                rgb_channel_capacity_min_sat: value.rgb_channel_capacity_min_sat?,
                rgb_htlc_min_msat: value.rgb_htlc_min_msat?,
            })
        }
    }
    impl ::std::convert::From<super::NodeInfoResponse> for NodeInfoResponse {
        fn from(value: super::NodeInfoResponse) -> Self {
            Self {
                account_xpub_colored: Ok(value.account_xpub_colored),
                account_xpub_vanilla: Ok(value.account_xpub_vanilla),
                channel_asset_max_amount: Ok(value.channel_asset_max_amount),
                channel_asset_min_amount: Ok(value.channel_asset_min_amount),
                channel_capacity_max_sat: Ok(value.channel_capacity_max_sat),
                channel_capacity_min_sat: Ok(value.channel_capacity_min_sat),
                eventual_close_fees_sat: Ok(value.eventual_close_fees_sat),
                local_balance_sat: Ok(value.local_balance_sat),
                max_media_upload_size_mb: Ok(value.max_media_upload_size_mb),
                network_channels: Ok(value.network_channels),
                network_nodes: Ok(value.network_nodes),
                num_channels: Ok(value.num_channels),
                num_peers: Ok(value.num_peers),
                num_usable_channels: Ok(value.num_usable_channels),
                pending_outbound_payments_sat: Ok(value.pending_outbound_payments_sat),
                pubkey: Ok(value.pubkey),
                rgb_channel_capacity_min_sat: Ok(value.rgb_channel_capacity_min_sat),
                rgb_htlc_min_msat: Ok(value.rgb_htlc_min_msat),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct OpenChannelRequest {
        asset_amount: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        asset_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        capacity_sat: ::std::result::Result<i64, ::std::string::String>,
        fee_base_msat: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        fee_proportional_millionths:
            ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        peer_pubkey_and_opt_addr:
            ::std::result::Result<::std::string::String, ::std::string::String>,
        public: ::std::result::Result<bool, ::std::string::String>,
        push_asset_amount: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        push_msat: ::std::result::Result<i64, ::std::string::String>,
        temporary_channel_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        with_anchors: ::std::result::Result<bool, ::std::string::String>,
    }
    impl ::std::default::Default for OpenChannelRequest {
        fn default() -> Self {
            Self {
                asset_amount: Ok(Default::default()),
                asset_id: Ok(Default::default()),
                capacity_sat: Err("no value supplied for capacity_sat".to_string()),
                fee_base_msat: Ok(Default::default()),
                fee_proportional_millionths: Ok(Default::default()),
                peer_pubkey_and_opt_addr: Err(
                    "no value supplied for peer_pubkey_and_opt_addr".to_string()
                ),
                public: Err("no value supplied for public".to_string()),
                push_asset_amount: Ok(Default::default()),
                push_msat: Err("no value supplied for push_msat".to_string()),
                temporary_channel_id: Ok(Default::default()),
                with_anchors: Err("no value supplied for with_anchors".to_string()),
            }
        }
    }
    impl OpenChannelRequest {
        pub fn asset_amount<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_amount = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_amount: {e}"));
            self
        }
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn capacity_sat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.capacity_sat = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for capacity_sat: {e}"));
            self
        }
        pub fn fee_base_msat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.fee_base_msat = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for fee_base_msat: {e}"));
            self
        }
        pub fn fee_proportional_millionths<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.fee_proportional_millionths = value.try_into().map_err(|e| {
                format!("error converting supplied value for fee_proportional_millionths: {e}")
            });
            self
        }
        pub fn peer_pubkey_and_opt_addr<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.peer_pubkey_and_opt_addr = value.try_into().map_err(|e| {
                format!("error converting supplied value for peer_pubkey_and_opt_addr: {e}")
            });
            self
        }
        pub fn public<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.public = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for public: {e}"));
            self
        }
        pub fn push_asset_amount<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.push_asset_amount = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for push_asset_amount: {e}"));
            self
        }
        pub fn push_msat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.push_msat = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for push_msat: {e}"));
            self
        }
        pub fn temporary_channel_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.temporary_channel_id = value.try_into().map_err(|e| {
                format!("error converting supplied value for temporary_channel_id: {e}")
            });
            self
        }
        pub fn with_anchors<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.with_anchors = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for with_anchors: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<OpenChannelRequest> for super::OpenChannelRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: OpenChannelRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset_amount: value.asset_amount?,
                asset_id: value.asset_id?,
                capacity_sat: value.capacity_sat?,
                fee_base_msat: value.fee_base_msat?,
                fee_proportional_millionths: value.fee_proportional_millionths?,
                peer_pubkey_and_opt_addr: value.peer_pubkey_and_opt_addr?,
                public: value.public?,
                push_asset_amount: value.push_asset_amount?,
                push_msat: value.push_msat?,
                temporary_channel_id: value.temporary_channel_id?,
                with_anchors: value.with_anchors?,
            })
        }
    }
    impl ::std::convert::From<super::OpenChannelRequest> for OpenChannelRequest {
        fn from(value: super::OpenChannelRequest) -> Self {
            Self {
                asset_amount: Ok(value.asset_amount),
                asset_id: Ok(value.asset_id),
                capacity_sat: Ok(value.capacity_sat),
                fee_base_msat: Ok(value.fee_base_msat),
                fee_proportional_millionths: Ok(value.fee_proportional_millionths),
                peer_pubkey_and_opt_addr: Ok(value.peer_pubkey_and_opt_addr),
                public: Ok(value.public),
                push_asset_amount: Ok(value.push_asset_amount),
                push_msat: Ok(value.push_msat),
                temporary_channel_id: Ok(value.temporary_channel_id),
                with_anchors: Ok(value.with_anchors),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct OpenChannelResponse {
        temporary_channel_id: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for OpenChannelResponse {
        fn default() -> Self {
            Self {
                temporary_channel_id: Err("no value supplied for temporary_channel_id".to_string()),
            }
        }
    }
    impl OpenChannelResponse {
        pub fn temporary_channel_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.temporary_channel_id = value.try_into().map_err(|e| {
                format!("error converting supplied value for temporary_channel_id: {e}")
            });
            self
        }
    }
    impl ::std::convert::TryFrom<OpenChannelResponse> for super::OpenChannelResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: OpenChannelResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                temporary_channel_id: value.temporary_channel_id?,
            })
        }
    }
    impl ::std::convert::From<super::OpenChannelResponse> for OpenChannelResponse {
        fn from(value: super::OpenChannelResponse) -> Self {
            Self {
                temporary_channel_id: Ok(value.temporary_channel_id),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Payment {
        amt_msat: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        asset_amount: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        asset_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        created_at: ::std::result::Result<i64, ::std::string::String>,
        inbound: ::std::result::Result<bool, ::std::string::String>,
        payee_pubkey: ::std::result::Result<::std::string::String, ::std::string::String>,
        payment_hash: ::std::result::Result<::std::string::String, ::std::string::String>,
        preimage: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        status: ::std::result::Result<super::HtlcStatus, ::std::string::String>,
        updated_at: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for Payment {
        fn default() -> Self {
            Self {
                amt_msat: Ok(Default::default()),
                asset_amount: Ok(Default::default()),
                asset_id: Ok(Default::default()),
                created_at: Err("no value supplied for created_at".to_string()),
                inbound: Err("no value supplied for inbound".to_string()),
                payee_pubkey: Err("no value supplied for payee_pubkey".to_string()),
                payment_hash: Err("no value supplied for payment_hash".to_string()),
                preimage: Ok(Default::default()),
                status: Err("no value supplied for status".to_string()),
                updated_at: Err("no value supplied for updated_at".to_string()),
            }
        }
    }
    impl Payment {
        pub fn amt_msat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.amt_msat = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for amt_msat: {e}"));
            self
        }
        pub fn asset_amount<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_amount = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_amount: {e}"));
            self
        }
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn created_at<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.created_at = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for created_at: {e}"));
            self
        }
        pub fn inbound<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.inbound = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for inbound: {e}"));
            self
        }
        pub fn payee_pubkey<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.payee_pubkey = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payee_pubkey: {e}"));
            self
        }
        pub fn payment_hash<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_hash = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_hash: {e}"));
            self
        }
        pub fn preimage<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.preimage = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for preimage: {e}"));
            self
        }
        pub fn status<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::HtlcStatus>,
            T::Error: ::std::fmt::Display,
        {
            self.status = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for status: {e}"));
            self
        }
        pub fn updated_at<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.updated_at = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for updated_at: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Payment> for super::Payment {
        type Error = super::error::ConversionError;
        fn try_from(value: Payment) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                amt_msat: value.amt_msat?,
                asset_amount: value.asset_amount?,
                asset_id: value.asset_id?,
                created_at: value.created_at?,
                inbound: value.inbound?,
                payee_pubkey: value.payee_pubkey?,
                payment_hash: value.payment_hash?,
                preimage: value.preimage?,
                status: value.status?,
                updated_at: value.updated_at?,
            })
        }
    }
    impl ::std::convert::From<super::Payment> for Payment {
        fn from(value: super::Payment) -> Self {
            Self {
                amt_msat: Ok(value.amt_msat),
                asset_amount: Ok(value.asset_amount),
                asset_id: Ok(value.asset_id),
                created_at: Ok(value.created_at),
                inbound: Ok(value.inbound),
                payee_pubkey: Ok(value.payee_pubkey),
                payment_hash: Ok(value.payment_hash),
                preimage: Ok(value.preimage),
                status: Ok(value.status),
                updated_at: Ok(value.updated_at),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Peer {
        pubkey: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for Peer {
        fn default() -> Self {
            Self {
                pubkey: Err("no value supplied for pubkey".to_string()),
            }
        }
    }
    impl Peer {
        pub fn pubkey<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.pubkey = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for pubkey: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Peer> for super::Peer {
        type Error = super::error::ConversionError;
        fn try_from(value: Peer) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                pubkey: value.pubkey?,
            })
        }
    }
    impl ::std::convert::From<super::Peer> for Peer {
        fn from(value: super::Peer) -> Self {
            Self {
                pubkey: Ok(value.pubkey),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct PostAssetMediaRequest {
        file: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for PostAssetMediaRequest {
        fn default() -> Self {
            Self {
                file: Err("no value supplied for file".to_string()),
            }
        }
    }
    impl PostAssetMediaRequest {
        pub fn file<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.file = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for file: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<PostAssetMediaRequest> for super::PostAssetMediaRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: PostAssetMediaRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self { file: value.file? })
        }
    }
    impl ::std::convert::From<super::PostAssetMediaRequest> for PostAssetMediaRequest {
        fn from(value: super::PostAssetMediaRequest) -> Self {
            Self {
                file: Ok(value.file),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct PostAssetMediaResponse {
        digest: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for PostAssetMediaResponse {
        fn default() -> Self {
            Self {
                digest: Err("no value supplied for digest".to_string()),
            }
        }
    }
    impl PostAssetMediaResponse {
        pub fn digest<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.digest = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for digest: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<PostAssetMediaResponse> for super::PostAssetMediaResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: PostAssetMediaResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                digest: value.digest?,
            })
        }
    }
    impl ::std::convert::From<super::PostAssetMediaResponse> for PostAssetMediaResponse {
        fn from(value: super::PostAssetMediaResponse) -> Self {
            Self {
                digest: Ok(value.digest),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct ProofOfReserves {
        proof: ::std::result::Result<::std::vec::Vec<i64>, ::std::string::String>,
        utxo: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for ProofOfReserves {
        fn default() -> Self {
            Self {
                proof: Err("no value supplied for proof".to_string()),
                utxo: Err("no value supplied for utxo".to_string()),
            }
        }
    }
    impl ProofOfReserves {
        pub fn proof<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.proof = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for proof: {e}"));
            self
        }
        pub fn utxo<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.utxo = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for utxo: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<ProofOfReserves> for super::ProofOfReserves {
        type Error = super::error::ConversionError;
        fn try_from(
            value: ProofOfReserves,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                proof: value.proof?,
                utxo: value.utxo?,
            })
        }
    }
    impl ::std::convert::From<super::ProofOfReserves> for ProofOfReserves {
        fn from(value: super::ProofOfReserves) -> Self {
            Self {
                proof: Ok(value.proof),
                utxo: Ok(value.utxo),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Recipient {
        assignment: ::std::result::Result<super::Assignment, ::std::string::String>,
        recipient_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        transport_endpoints:
            ::std::result::Result<::std::vec::Vec<::std::string::String>, ::std::string::String>,
        witness_data:
            ::std::result::Result<::std::option::Option<super::WitnessData>, ::std::string::String>,
    }
    impl ::std::default::Default for Recipient {
        fn default() -> Self {
            Self {
                assignment: Err("no value supplied for assignment".to_string()),
                recipient_id: Err("no value supplied for recipient_id".to_string()),
                transport_endpoints: Err("no value supplied for transport_endpoints".to_string()),
                witness_data: Ok(Default::default()),
            }
        }
    }
    impl Recipient {
        pub fn assignment<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::Assignment>,
            T::Error: ::std::fmt::Display,
        {
            self.assignment = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for assignment: {e}"));
            self
        }
        pub fn recipient_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.recipient_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for recipient_id: {e}"));
            self
        }
        pub fn transport_endpoints<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.transport_endpoints = value.try_into().map_err(|e| {
                format!("error converting supplied value for transport_endpoints: {e}")
            });
            self
        }
        pub fn witness_data<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::WitnessData>>,
            T::Error: ::std::fmt::Display,
        {
            self.witness_data = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for witness_data: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Recipient> for super::Recipient {
        type Error = super::error::ConversionError;
        fn try_from(
            value: Recipient,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                assignment: value.assignment?,
                recipient_id: value.recipient_id?,
                transport_endpoints: value.transport_endpoints?,
                witness_data: value.witness_data?,
            })
        }
    }
    impl ::std::convert::From<super::Recipient> for Recipient {
        fn from(value: super::Recipient) -> Self {
            Self {
                assignment: Ok(value.assignment),
                recipient_id: Ok(value.recipient_id),
                transport_endpoints: Ok(value.transport_endpoints),
                witness_data: Ok(value.witness_data),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct RefreshFilter {
        incoming: ::std::result::Result<bool, ::std::string::String>,
        status: ::std::result::Result<super::RefreshTransferStatus, ::std::string::String>,
    }
    impl ::std::default::Default for RefreshFilter {
        fn default() -> Self {
            Self {
                incoming: Err("no value supplied for incoming".to_string()),
                status: Err("no value supplied for status".to_string()),
            }
        }
    }
    impl RefreshFilter {
        pub fn incoming<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.incoming = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for incoming: {e}"));
            self
        }
        pub fn status<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::RefreshTransferStatus>,
            T::Error: ::std::fmt::Display,
        {
            self.status = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for status: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<RefreshFilter> for super::RefreshFilter {
        type Error = super::error::ConversionError;
        fn try_from(
            value: RefreshFilter,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                incoming: value.incoming?,
                status: value.status?,
            })
        }
    }
    impl ::std::convert::From<super::RefreshFilter> for RefreshFilter {
        fn from(value: super::RefreshFilter) -> Self {
            Self {
                incoming: Ok(value.incoming),
                status: Ok(value.status),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct RefreshRequest {
        asset_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        filter: ::std::result::Result<::std::vec::Vec<super::RefreshFilter>, ::std::string::String>,
        skip_sync: ::std::result::Result<bool, ::std::string::String>,
    }
    impl ::std::default::Default for RefreshRequest {
        fn default() -> Self {
            Self {
                asset_id: Ok(Default::default()),
                filter: Err("no value supplied for filter".to_string()),
                skip_sync: Err("no value supplied for skip_sync".to_string()),
            }
        }
    }
    impl RefreshRequest {
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn filter<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::RefreshFilter>>,
            T::Error: ::std::fmt::Display,
        {
            self.filter = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for filter: {e}"));
            self
        }
        pub fn skip_sync<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.skip_sync = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for skip_sync: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<RefreshRequest> for super::RefreshRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: RefreshRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset_id: value.asset_id?,
                filter: value.filter?,
                skip_sync: value.skip_sync?,
            })
        }
    }
    impl ::std::convert::From<super::RefreshRequest> for RefreshRequest {
        fn from(value: super::RefreshRequest) -> Self {
            Self {
                asset_id: Ok(value.asset_id),
                filter: Ok(value.filter),
                skip_sync: Ok(value.skip_sync),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct RestoreRequest {
        backup_path: ::std::result::Result<::std::string::String, ::std::string::String>,
        password: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for RestoreRequest {
        fn default() -> Self {
            Self {
                backup_path: Err("no value supplied for backup_path".to_string()),
                password: Err("no value supplied for password".to_string()),
            }
        }
    }
    impl RestoreRequest {
        pub fn backup_path<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.backup_path = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for backup_path: {e}"));
            self
        }
        pub fn password<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.password = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for password: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<RestoreRequest> for super::RestoreRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: RestoreRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                backup_path: value.backup_path?,
                password: value.password?,
            })
        }
    }
    impl ::std::convert::From<super::RestoreRequest> for RestoreRequest {
        fn from(value: super::RestoreRequest) -> Self {
            Self {
                backup_path: Ok(value.backup_path),
                password: Ok(value.password),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct RevokeTokenRequest {
        token: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for RevokeTokenRequest {
        fn default() -> Self {
            Self {
                token: Err("no value supplied for token".to_string()),
            }
        }
    }
    impl RevokeTokenRequest {
        pub fn token<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.token = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for token: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<RevokeTokenRequest> for super::RevokeTokenRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: RevokeTokenRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                token: value.token?,
            })
        }
    }
    impl ::std::convert::From<super::RevokeTokenRequest> for RevokeTokenRequest {
        fn from(value: super::RevokeTokenRequest) -> Self {
            Self {
                token: Ok(value.token),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct RgbAllocation {
        asset_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        assignment: ::std::result::Result<super::Assignment, ::std::string::String>,
        settled: ::std::result::Result<bool, ::std::string::String>,
    }
    impl ::std::default::Default for RgbAllocation {
        fn default() -> Self {
            Self {
                asset_id: Ok(Default::default()),
                assignment: Err("no value supplied for assignment".to_string()),
                settled: Err("no value supplied for settled".to_string()),
            }
        }
    }
    impl RgbAllocation {
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn assignment<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::Assignment>,
            T::Error: ::std::fmt::Display,
        {
            self.assignment = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for assignment: {e}"));
            self
        }
        pub fn settled<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.settled = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for settled: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<RgbAllocation> for super::RgbAllocation {
        type Error = super::error::ConversionError;
        fn try_from(
            value: RgbAllocation,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset_id: value.asset_id?,
                assignment: value.assignment?,
                settled: value.settled?,
            })
        }
    }
    impl ::std::convert::From<super::RgbAllocation> for RgbAllocation {
        fn from(value: super::RgbAllocation) -> Self {
            Self {
                asset_id: Ok(value.asset_id),
                assignment: Ok(value.assignment),
                settled: Ok(value.settled),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct RgbInvoiceRequest {
        asset_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        assignment:
            ::std::result::Result<::std::option::Option<super::Assignment>, ::std::string::String>,
        expiration_timestamp:
            ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        min_confirmations: ::std::result::Result<i64, ::std::string::String>,
        witness: ::std::result::Result<bool, ::std::string::String>,
    }
    impl ::std::default::Default for RgbInvoiceRequest {
        fn default() -> Self {
            Self {
                asset_id: Ok(Default::default()),
                assignment: Ok(Default::default()),
                expiration_timestamp: Ok(Default::default()),
                min_confirmations: Err("no value supplied for min_confirmations".to_string()),
                witness: Err("no value supplied for witness".to_string()),
            }
        }
    }
    impl RgbInvoiceRequest {
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn assignment<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Assignment>>,
            T::Error: ::std::fmt::Display,
        {
            self.assignment = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for assignment: {e}"));
            self
        }
        pub fn expiration_timestamp<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.expiration_timestamp = value.try_into().map_err(|e| {
                format!("error converting supplied value for expiration_timestamp: {e}")
            });
            self
        }
        pub fn min_confirmations<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.min_confirmations = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for min_confirmations: {e}"));
            self
        }
        pub fn witness<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.witness = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for witness: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<RgbInvoiceRequest> for super::RgbInvoiceRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: RgbInvoiceRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                asset_id: value.asset_id?,
                assignment: value.assignment?,
                expiration_timestamp: value.expiration_timestamp?,
                min_confirmations: value.min_confirmations?,
                witness: value.witness?,
            })
        }
    }
    impl ::std::convert::From<super::RgbInvoiceRequest> for RgbInvoiceRequest {
        fn from(value: super::RgbInvoiceRequest) -> Self {
            Self {
                asset_id: Ok(value.asset_id),
                assignment: Ok(value.assignment),
                expiration_timestamp: Ok(value.expiration_timestamp),
                min_confirmations: Ok(value.min_confirmations),
                witness: Ok(value.witness),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct RgbInvoiceResponse {
        batch_transfer_idx: ::std::result::Result<i64, ::std::string::String>,
        expiration_timestamp:
            ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        invoice: ::std::result::Result<::std::string::String, ::std::string::String>,
        recipient_id: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for RgbInvoiceResponse {
        fn default() -> Self {
            Self {
                batch_transfer_idx: Err("no value supplied for batch_transfer_idx".to_string()),
                expiration_timestamp: Ok(Default::default()),
                invoice: Err("no value supplied for invoice".to_string()),
                recipient_id: Err("no value supplied for recipient_id".to_string()),
            }
        }
    }
    impl RgbInvoiceResponse {
        pub fn batch_transfer_idx<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.batch_transfer_idx = value.try_into().map_err(|e| {
                format!("error converting supplied value for batch_transfer_idx: {e}")
            });
            self
        }
        pub fn expiration_timestamp<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.expiration_timestamp = value.try_into().map_err(|e| {
                format!("error converting supplied value for expiration_timestamp: {e}")
            });
            self
        }
        pub fn invoice<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.invoice = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for invoice: {e}"));
            self
        }
        pub fn recipient_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.recipient_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for recipient_id: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<RgbInvoiceResponse> for super::RgbInvoiceResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: RgbInvoiceResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                batch_transfer_idx: value.batch_transfer_idx?,
                expiration_timestamp: value.expiration_timestamp?,
                invoice: value.invoice?,
                recipient_id: value.recipient_id?,
            })
        }
    }
    impl ::std::convert::From<super::RgbInvoiceResponse> for RgbInvoiceResponse {
        fn from(value: super::RgbInvoiceResponse) -> Self {
            Self {
                batch_transfer_idx: Ok(value.batch_transfer_idx),
                expiration_timestamp: Ok(value.expiration_timestamp),
                invoice: Ok(value.invoice),
                recipient_id: Ok(value.recipient_id),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SendBtcRequest {
        address: ::std::result::Result<::std::string::String, ::std::string::String>,
        amount: ::std::result::Result<i64, ::std::string::String>,
        fee_rate: ::std::result::Result<i64, ::std::string::String>,
        skip_sync: ::std::result::Result<bool, ::std::string::String>,
    }
    impl ::std::default::Default for SendBtcRequest {
        fn default() -> Self {
            Self {
                address: Err("no value supplied for address".to_string()),
                amount: Err("no value supplied for amount".to_string()),
                fee_rate: Err("no value supplied for fee_rate".to_string()),
                skip_sync: Err("no value supplied for skip_sync".to_string()),
            }
        }
    }
    impl SendBtcRequest {
        pub fn address<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.address = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for address: {e}"));
            self
        }
        pub fn amount<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.amount = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for amount: {e}"));
            self
        }
        pub fn fee_rate<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.fee_rate = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for fee_rate: {e}"));
            self
        }
        pub fn skip_sync<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.skip_sync = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for skip_sync: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SendBtcRequest> for super::SendBtcRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SendBtcRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                address: value.address?,
                amount: value.amount?,
                fee_rate: value.fee_rate?,
                skip_sync: value.skip_sync?,
            })
        }
    }
    impl ::std::convert::From<super::SendBtcRequest> for SendBtcRequest {
        fn from(value: super::SendBtcRequest) -> Self {
            Self {
                address: Ok(value.address),
                amount: Ok(value.amount),
                fee_rate: Ok(value.fee_rate),
                skip_sync: Ok(value.skip_sync),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SendBtcResponse {
        txid: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for SendBtcResponse {
        fn default() -> Self {
            Self {
                txid: Err("no value supplied for txid".to_string()),
            }
        }
    }
    impl SendBtcResponse {
        pub fn txid<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.txid = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for txid: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SendBtcResponse> for super::SendBtcResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SendBtcResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self { txid: value.txid? })
        }
    }
    impl ::std::convert::From<super::SendBtcResponse> for SendBtcResponse {
        fn from(value: super::SendBtcResponse) -> Self {
            Self {
                txid: Ok(value.txid),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SendOnionMessageRequest {
        data: ::std::result::Result<::std::string::String, ::std::string::String>,
        node_ids:
            ::std::result::Result<::std::vec::Vec<::std::string::String>, ::std::string::String>,
        tlv_type: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for SendOnionMessageRequest {
        fn default() -> Self {
            Self {
                data: Err("no value supplied for data".to_string()),
                node_ids: Err("no value supplied for node_ids".to_string()),
                tlv_type: Err("no value supplied for tlv_type".to_string()),
            }
        }
    }
    impl SendOnionMessageRequest {
        pub fn data<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.data = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data: {e}"));
            self
        }
        pub fn node_ids<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.node_ids = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for node_ids: {e}"));
            self
        }
        pub fn tlv_type<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.tlv_type = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tlv_type: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SendOnionMessageRequest> for super::SendOnionMessageRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SendOnionMessageRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                data: value.data?,
                node_ids: value.node_ids?,
                tlv_type: value.tlv_type?,
            })
        }
    }
    impl ::std::convert::From<super::SendOnionMessageRequest> for SendOnionMessageRequest {
        fn from(value: super::SendOnionMessageRequest) -> Self {
            Self {
                data: Ok(value.data),
                node_ids: Ok(value.node_ids),
                tlv_type: Ok(value.tlv_type),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SendPaymentRequest {
        amt_msat: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        asset_amount: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        asset_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        invoice: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for SendPaymentRequest {
        fn default() -> Self {
            Self {
                amt_msat: Ok(Default::default()),
                asset_amount: Ok(Default::default()),
                asset_id: Ok(Default::default()),
                invoice: Err("no value supplied for invoice".to_string()),
            }
        }
    }
    impl SendPaymentRequest {
        pub fn amt_msat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.amt_msat = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for amt_msat: {e}"));
            self
        }
        pub fn asset_amount<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_amount = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_amount: {e}"));
            self
        }
        pub fn asset_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.asset_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for asset_id: {e}"));
            self
        }
        pub fn invoice<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.invoice = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for invoice: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SendPaymentRequest> for super::SendPaymentRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SendPaymentRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                amt_msat: value.amt_msat?,
                asset_amount: value.asset_amount?,
                asset_id: value.asset_id?,
                invoice: value.invoice?,
            })
        }
    }
    impl ::std::convert::From<super::SendPaymentRequest> for SendPaymentRequest {
        fn from(value: super::SendPaymentRequest) -> Self {
            Self {
                amt_msat: Ok(value.amt_msat),
                asset_amount: Ok(value.asset_amount),
                asset_id: Ok(value.asset_id),
                invoice: Ok(value.invoice),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SendPaymentResponse {
        payment_hash: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        payment_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        payment_secret: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        status: ::std::result::Result<super::HtlcStatus, ::std::string::String>,
    }
    impl ::std::default::Default for SendPaymentResponse {
        fn default() -> Self {
            Self {
                payment_hash: Ok(Default::default()),
                payment_id: Err("no value supplied for payment_id".to_string()),
                payment_secret: Ok(Default::default()),
                status: Err("no value supplied for status".to_string()),
            }
        }
    }
    impl SendPaymentResponse {
        pub fn payment_hash<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_hash = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_hash: {e}"));
            self
        }
        pub fn payment_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_id: {e}"));
            self
        }
        pub fn payment_secret<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_secret = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_secret: {e}"));
            self
        }
        pub fn status<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::HtlcStatus>,
            T::Error: ::std::fmt::Display,
        {
            self.status = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for status: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SendPaymentResponse> for super::SendPaymentResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SendPaymentResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                payment_hash: value.payment_hash?,
                payment_id: value.payment_id?,
                payment_secret: value.payment_secret?,
                status: value.status?,
            })
        }
    }
    impl ::std::convert::From<super::SendPaymentResponse> for SendPaymentResponse {
        fn from(value: super::SendPaymentResponse) -> Self {
            Self {
                payment_hash: Ok(value.payment_hash),
                payment_id: Ok(value.payment_id),
                payment_secret: Ok(value.payment_secret),
                status: Ok(value.status),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SendRgbRequest {
        donation: ::std::result::Result<bool, ::std::string::String>,
        expiration_timestamp:
            ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        fee_rate: ::std::result::Result<i64, ::std::string::String>,
        min_confirmations: ::std::result::Result<i64, ::std::string::String>,
        recipient_map: ::std::result::Result<
            ::std::collections::HashMap<::std::string::String, ::std::vec::Vec<super::Recipient>>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for SendRgbRequest {
        fn default() -> Self {
            Self {
                donation: Err("no value supplied for donation".to_string()),
                expiration_timestamp: Ok(Default::default()),
                fee_rate: Err("no value supplied for fee_rate".to_string()),
                min_confirmations: Err("no value supplied for min_confirmations".to_string()),
                recipient_map: Err("no value supplied for recipient_map".to_string()),
            }
        }
    }
    impl SendRgbRequest {
        pub fn donation<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.donation = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for donation: {e}"));
            self
        }
        pub fn expiration_timestamp<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.expiration_timestamp = value.try_into().map_err(|e| {
                format!("error converting supplied value for expiration_timestamp: {e}")
            });
            self
        }
        pub fn fee_rate<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.fee_rate = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for fee_rate: {e}"));
            self
        }
        pub fn min_confirmations<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.min_confirmations = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for min_confirmations: {e}"));
            self
        }
        pub fn recipient_map<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::collections::HashMap<
                    ::std::string::String,
                    ::std::vec::Vec<super::Recipient>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.recipient_map = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for recipient_map: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SendRgbRequest> for super::SendRgbRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SendRgbRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                donation: value.donation?,
                expiration_timestamp: value.expiration_timestamp?,
                fee_rate: value.fee_rate?,
                min_confirmations: value.min_confirmations?,
                recipient_map: value.recipient_map?,
            })
        }
    }
    impl ::std::convert::From<super::SendRgbRequest> for SendRgbRequest {
        fn from(value: super::SendRgbRequest) -> Self {
            Self {
                donation: Ok(value.donation),
                expiration_timestamp: Ok(value.expiration_timestamp),
                fee_rate: Ok(value.fee_rate),
                min_confirmations: Ok(value.min_confirmations),
                recipient_map: Ok(value.recipient_map),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SendRgbResponse {
        txid: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for SendRgbResponse {
        fn default() -> Self {
            Self {
                txid: Err("no value supplied for txid".to_string()),
            }
        }
    }
    impl SendRgbResponse {
        pub fn txid<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.txid = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for txid: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SendRgbResponse> for super::SendRgbResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SendRgbResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self { txid: value.txid? })
        }
    }
    impl ::std::convert::From<super::SendRgbResponse> for SendRgbResponse {
        fn from(value: super::SendRgbResponse) -> Self {
            Self {
                txid: Ok(value.txid),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SignMessageRequest {
        message: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for SignMessageRequest {
        fn default() -> Self {
            Self {
                message: Err("no value supplied for message".to_string()),
            }
        }
    }
    impl SignMessageRequest {
        pub fn message<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.message = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for message: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SignMessageRequest> for super::SignMessageRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SignMessageRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                message: value.message?,
            })
        }
    }
    impl ::std::convert::From<super::SignMessageRequest> for SignMessageRequest {
        fn from(value: super::SignMessageRequest) -> Self {
            Self {
                message: Ok(value.message),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SignMessageResponse {
        signed_message: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for SignMessageResponse {
        fn default() -> Self {
            Self {
                signed_message: Err("no value supplied for signed_message".to_string()),
            }
        }
    }
    impl SignMessageResponse {
        pub fn signed_message<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.signed_message = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for signed_message: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SignMessageResponse> for super::SignMessageResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SignMessageResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                signed_message: value.signed_message?,
            })
        }
    }
    impl ::std::convert::From<super::SignMessageResponse> for SignMessageResponse {
        fn from(value: super::SignMessageResponse) -> Self {
            Self {
                signed_message: Ok(value.signed_message),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Swap {
        completed_at: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        expires_at: ::std::result::Result<i64, ::std::string::String>,
        from_asset: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        initiated_at: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        payment_hash: ::std::result::Result<::std::string::String, ::std::string::String>,
        qty_from: ::std::result::Result<i64, ::std::string::String>,
        qty_to: ::std::result::Result<i64, ::std::string::String>,
        requested_at: ::std::result::Result<i64, ::std::string::String>,
        status: ::std::result::Result<super::SwapStatus, ::std::string::String>,
        to_asset: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for Swap {
        fn default() -> Self {
            Self {
                completed_at: Ok(Default::default()),
                expires_at: Err("no value supplied for expires_at".to_string()),
                from_asset: Ok(Default::default()),
                initiated_at: Ok(Default::default()),
                payment_hash: Err("no value supplied for payment_hash".to_string()),
                qty_from: Err("no value supplied for qty_from".to_string()),
                qty_to: Err("no value supplied for qty_to".to_string()),
                requested_at: Err("no value supplied for requested_at".to_string()),
                status: Err("no value supplied for status".to_string()),
                to_asset: Ok(Default::default()),
            }
        }
    }
    impl Swap {
        pub fn completed_at<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.completed_at = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for completed_at: {e}"));
            self
        }
        pub fn expires_at<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.expires_at = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for expires_at: {e}"));
            self
        }
        pub fn from_asset<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.from_asset = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for from_asset: {e}"));
            self
        }
        pub fn initiated_at<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.initiated_at = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for initiated_at: {e}"));
            self
        }
        pub fn payment_hash<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.payment_hash = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for payment_hash: {e}"));
            self
        }
        pub fn qty_from<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.qty_from = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for qty_from: {e}"));
            self
        }
        pub fn qty_to<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.qty_to = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for qty_to: {e}"));
            self
        }
        pub fn requested_at<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.requested_at = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for requested_at: {e}"));
            self
        }
        pub fn status<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::SwapStatus>,
            T::Error: ::std::fmt::Display,
        {
            self.status = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for status: {e}"));
            self
        }
        pub fn to_asset<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.to_asset = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for to_asset: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Swap> for super::Swap {
        type Error = super::error::ConversionError;
        fn try_from(value: Swap) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                completed_at: value.completed_at?,
                expires_at: value.expires_at?,
                from_asset: value.from_asset?,
                initiated_at: value.initiated_at?,
                payment_hash: value.payment_hash?,
                qty_from: value.qty_from?,
                qty_to: value.qty_to?,
                requested_at: value.requested_at?,
                status: value.status?,
                to_asset: value.to_asset?,
            })
        }
    }
    impl ::std::convert::From<super::Swap> for Swap {
        fn from(value: super::Swap) -> Self {
            Self {
                completed_at: Ok(value.completed_at),
                expires_at: Ok(value.expires_at),
                from_asset: Ok(value.from_asset),
                initiated_at: Ok(value.initiated_at),
                payment_hash: Ok(value.payment_hash),
                qty_from: Ok(value.qty_from),
                qty_to: Ok(value.qty_to),
                requested_at: Ok(value.requested_at),
                status: Ok(value.status),
                to_asset: Ok(value.to_asset),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SyncOptions {
        keychain: ::std::result::Result<super::SyncKeychain, ::std::string::String>,
        strategy: ::std::result::Result<super::SyncStrategy, ::std::string::String>,
    }
    impl ::std::default::Default for SyncOptions {
        fn default() -> Self {
            Self {
                keychain: Err("no value supplied for keychain".to_string()),
                strategy: Err("no value supplied for strategy".to_string()),
            }
        }
    }
    impl SyncOptions {
        pub fn keychain<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::SyncKeychain>,
            T::Error: ::std::fmt::Display,
        {
            self.keychain = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for keychain: {e}"));
            self
        }
        pub fn strategy<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::SyncStrategy>,
            T::Error: ::std::fmt::Display,
        {
            self.strategy = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for strategy: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SyncOptions> for super::SyncOptions {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SyncOptions,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                keychain: value.keychain?,
                strategy: value.strategy?,
            })
        }
    }
    impl ::std::convert::From<super::SyncOptions> for SyncOptions {
        fn from(value: super::SyncOptions) -> Self {
            Self {
                keychain: Ok(value.keychain),
                strategy: Ok(value.strategy),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SyncRequest {
        options: ::std::result::Result<super::SyncOptions, ::std::string::String>,
    }
    impl ::std::default::Default for SyncRequest {
        fn default() -> Self {
            Self {
                options: Err("no value supplied for options".to_string()),
            }
        }
    }
    impl SyncRequest {
        pub fn options<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::SyncOptions>,
            T::Error: ::std::fmt::Display,
        {
            self.options = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for options: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<SyncRequest> for super::SyncRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SyncRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                options: value.options?,
            })
        }
    }
    impl ::std::convert::From<super::SyncRequest> for SyncRequest {
        fn from(value: super::SyncRequest) -> Self {
            Self {
                options: Ok(value.options),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TakerRequest {
        swapstring: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for TakerRequest {
        fn default() -> Self {
            Self {
                swapstring: Err("no value supplied for swapstring".to_string()),
            }
        }
    }
    impl TakerRequest {
        pub fn swapstring<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.swapstring = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for swapstring: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<TakerRequest> for super::TakerRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: TakerRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                swapstring: value.swapstring?,
            })
        }
    }
    impl ::std::convert::From<super::TakerRequest> for TakerRequest {
        fn from(value: super::TakerRequest) -> Self {
            Self {
                swapstring: Ok(value.swapstring),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Token {
        attachments: ::std::result::Result<
            ::std::collections::HashMap<::std::string::String, super::Media>,
            ::std::string::String,
        >,
        details: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        embedded_media: ::std::result::Result<
            ::std::option::Option<super::EmbeddedMedia>,
            ::std::string::String,
        >,
        index: ::std::result::Result<i64, ::std::string::String>,
        media: ::std::result::Result<::std::option::Option<super::Media>, ::std::string::String>,
        name: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        reserves: ::std::result::Result<
            ::std::option::Option<super::ProofOfReserves>,
            ::std::string::String,
        >,
        ticker: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for Token {
        fn default() -> Self {
            Self {
                attachments: Err("no value supplied for attachments".to_string()),
                details: Ok(Default::default()),
                embedded_media: Ok(Default::default()),
                index: Err("no value supplied for index".to_string()),
                media: Ok(Default::default()),
                name: Ok(Default::default()),
                reserves: Ok(Default::default()),
                ticker: Ok(Default::default()),
            }
        }
    }
    impl Token {
        pub fn attachments<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::collections::HashMap<::std::string::String, super::Media>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.attachments = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for attachments: {e}"));
            self
        }
        pub fn details<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.details = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for details: {e}"));
            self
        }
        pub fn embedded_media<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::EmbeddedMedia>>,
            T::Error: ::std::fmt::Display,
        {
            self.embedded_media = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for embedded_media: {e}"));
            self
        }
        pub fn index<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.index = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for index: {e}"));
            self
        }
        pub fn media<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Media>>,
            T::Error: ::std::fmt::Display,
        {
            self.media = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for media: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn reserves<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::ProofOfReserves>>,
            T::Error: ::std::fmt::Display,
        {
            self.reserves = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for reserves: {e}"));
            self
        }
        pub fn ticker<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.ticker = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for ticker: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Token> for super::Token {
        type Error = super::error::ConversionError;
        fn try_from(value: Token) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                attachments: value.attachments?,
                details: value.details?,
                embedded_media: value.embedded_media?,
                index: value.index?,
                media: value.media?,
                name: value.name?,
                reserves: value.reserves?,
                ticker: value.ticker?,
            })
        }
    }
    impl ::std::convert::From<super::Token> for Token {
        fn from(value: super::Token) -> Self {
            Self {
                attachments: Ok(value.attachments),
                details: Ok(value.details),
                embedded_media: Ok(value.embedded_media),
                index: Ok(value.index),
                media: Ok(value.media),
                name: Ok(value.name),
                reserves: Ok(value.reserves),
                ticker: Ok(value.ticker),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TokenLight {
        attachments: ::std::result::Result<
            ::std::collections::HashMap<::std::string::String, super::Media>,
            ::std::string::String,
        >,
        details: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        embedded_media: ::std::result::Result<bool, ::std::string::String>,
        index: ::std::result::Result<i64, ::std::string::String>,
        media: ::std::result::Result<::std::option::Option<super::Media>, ::std::string::String>,
        name: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        reserves: ::std::result::Result<bool, ::std::string::String>,
        ticker: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for TokenLight {
        fn default() -> Self {
            Self {
                attachments: Err("no value supplied for attachments".to_string()),
                details: Ok(Default::default()),
                embedded_media: Err("no value supplied for embedded_media".to_string()),
                index: Err("no value supplied for index".to_string()),
                media: Ok(Default::default()),
                name: Ok(Default::default()),
                reserves: Err("no value supplied for reserves".to_string()),
                ticker: Ok(Default::default()),
            }
        }
    }
    impl TokenLight {
        pub fn attachments<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::collections::HashMap<::std::string::String, super::Media>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.attachments = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for attachments: {e}"));
            self
        }
        pub fn details<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.details = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for details: {e}"));
            self
        }
        pub fn embedded_media<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.embedded_media = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for embedded_media: {e}"));
            self
        }
        pub fn index<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.index = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for index: {e}"));
            self
        }
        pub fn media<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Media>>,
            T::Error: ::std::fmt::Display,
        {
            self.media = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for media: {e}"));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {e}"));
            self
        }
        pub fn reserves<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.reserves = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for reserves: {e}"));
            self
        }
        pub fn ticker<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.ticker = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for ticker: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<TokenLight> for super::TokenLight {
        type Error = super::error::ConversionError;
        fn try_from(
            value: TokenLight,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                attachments: value.attachments?,
                details: value.details?,
                embedded_media: value.embedded_media?,
                index: value.index?,
                media: value.media?,
                name: value.name?,
                reserves: value.reserves?,
                ticker: value.ticker?,
            })
        }
    }
    impl ::std::convert::From<super::TokenLight> for TokenLight {
        fn from(value: super::TokenLight) -> Self {
            Self {
                attachments: Ok(value.attachments),
                details: Ok(value.details),
                embedded_media: Ok(value.embedded_media),
                index: Ok(value.index),
                media: Ok(value.media),
                name: Ok(value.name),
                reserves: Ok(value.reserves),
                ticker: Ok(value.ticker),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Transaction {
        confirmation_time:
            ::std::result::Result<::std::option::Option<super::BlockTime>, ::std::string::String>,
        fee: ::std::result::Result<i64, ::std::string::String>,
        received: ::std::result::Result<i64, ::std::string::String>,
        sent: ::std::result::Result<i64, ::std::string::String>,
        transaction_type: ::std::result::Result<super::TransactionType, ::std::string::String>,
        txid: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for Transaction {
        fn default() -> Self {
            Self {
                confirmation_time: Ok(Default::default()),
                fee: Err("no value supplied for fee".to_string()),
                received: Err("no value supplied for received".to_string()),
                sent: Err("no value supplied for sent".to_string()),
                transaction_type: Err("no value supplied for transaction_type".to_string()),
                txid: Err("no value supplied for txid".to_string()),
            }
        }
    }
    impl Transaction {
        pub fn confirmation_time<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::BlockTime>>,
            T::Error: ::std::fmt::Display,
        {
            self.confirmation_time = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for confirmation_time: {e}"));
            self
        }
        pub fn fee<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.fee = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for fee: {e}"));
            self
        }
        pub fn received<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.received = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for received: {e}"));
            self
        }
        pub fn sent<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.sent = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for sent: {e}"));
            self
        }
        pub fn transaction_type<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::TransactionType>,
            T::Error: ::std::fmt::Display,
        {
            self.transaction_type = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for transaction_type: {e}"));
            self
        }
        pub fn txid<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.txid = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for txid: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Transaction> for super::Transaction {
        type Error = super::error::ConversionError;
        fn try_from(
            value: Transaction,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                confirmation_time: value.confirmation_time?,
                fee: value.fee?,
                received: value.received?,
                sent: value.sent?,
                transaction_type: value.transaction_type?,
                txid: value.txid?,
            })
        }
    }
    impl ::std::convert::From<super::Transaction> for Transaction {
        fn from(value: super::Transaction) -> Self {
            Self {
                confirmation_time: Ok(value.confirmation_time),
                fee: Ok(value.fee),
                received: Ok(value.received),
                sent: Ok(value.sent),
                transaction_type: Ok(value.transaction_type),
                txid: Ok(value.txid),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Transfer {
        assignments:
            ::std::result::Result<::std::vec::Vec<super::Assignment>, ::std::string::String>,
        change_utxo: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        created_at: ::std::result::Result<i64, ::std::string::String>,
        expiration_timestamp:
            ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        idx: ::std::result::Result<i64, ::std::string::String>,
        kind: ::std::result::Result<super::TransferKind, ::std::string::String>,
        receive_utxo: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        recipient_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        requested_assignment:
            ::std::result::Result<::std::option::Option<super::Assignment>, ::std::string::String>,
        status: ::std::result::Result<super::TransferStatus, ::std::string::String>,
        transport_endpoints: ::std::result::Result<
            ::std::vec::Vec<super::TransferTransportEndpoint>,
            ::std::string::String,
        >,
        txid: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        updated_at: ::std::result::Result<i64, ::std::string::String>,
    }
    impl ::std::default::Default for Transfer {
        fn default() -> Self {
            Self {
                assignments: Err("no value supplied for assignments".to_string()),
                change_utxo: Ok(Default::default()),
                created_at: Err("no value supplied for created_at".to_string()),
                expiration_timestamp: Ok(Default::default()),
                idx: Err("no value supplied for idx".to_string()),
                kind: Err("no value supplied for kind".to_string()),
                receive_utxo: Ok(Default::default()),
                recipient_id: Ok(Default::default()),
                requested_assignment: Ok(Default::default()),
                status: Err("no value supplied for status".to_string()),
                transport_endpoints: Err("no value supplied for transport_endpoints".to_string()),
                txid: Ok(Default::default()),
                updated_at: Err("no value supplied for updated_at".to_string()),
            }
        }
    }
    impl Transfer {
        pub fn assignments<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Assignment>>,
            T::Error: ::std::fmt::Display,
        {
            self.assignments = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for assignments: {e}"));
            self
        }
        pub fn change_utxo<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.change_utxo = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for change_utxo: {e}"));
            self
        }
        pub fn created_at<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.created_at = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for created_at: {e}"));
            self
        }
        pub fn expiration_timestamp<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.expiration_timestamp = value.try_into().map_err(|e| {
                format!("error converting supplied value for expiration_timestamp: {e}")
            });
            self
        }
        pub fn idx<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.idx = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for idx: {e}"));
            self
        }
        pub fn kind<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::TransferKind>,
            T::Error: ::std::fmt::Display,
        {
            self.kind = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for kind: {e}"));
            self
        }
        pub fn receive_utxo<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.receive_utxo = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for receive_utxo: {e}"));
            self
        }
        pub fn recipient_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.recipient_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for recipient_id: {e}"));
            self
        }
        pub fn requested_assignment<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Assignment>>,
            T::Error: ::std::fmt::Display,
        {
            self.requested_assignment = value.try_into().map_err(|e| {
                format!("error converting supplied value for requested_assignment: {e}")
            });
            self
        }
        pub fn status<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::TransferStatus>,
            T::Error: ::std::fmt::Display,
        {
            self.status = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for status: {e}"));
            self
        }
        pub fn transport_endpoints<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::TransferTransportEndpoint>>,
            T::Error: ::std::fmt::Display,
        {
            self.transport_endpoints = value.try_into().map_err(|e| {
                format!("error converting supplied value for transport_endpoints: {e}")
            });
            self
        }
        pub fn txid<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.txid = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for txid: {e}"));
            self
        }
        pub fn updated_at<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.updated_at = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for updated_at: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Transfer> for super::Transfer {
        type Error = super::error::ConversionError;
        fn try_from(value: Transfer) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                assignments: value.assignments?,
                change_utxo: value.change_utxo?,
                created_at: value.created_at?,
                expiration_timestamp: value.expiration_timestamp?,
                idx: value.idx?,
                kind: value.kind?,
                receive_utxo: value.receive_utxo?,
                recipient_id: value.recipient_id?,
                requested_assignment: value.requested_assignment?,
                status: value.status?,
                transport_endpoints: value.transport_endpoints?,
                txid: value.txid?,
                updated_at: value.updated_at?,
            })
        }
    }
    impl ::std::convert::From<super::Transfer> for Transfer {
        fn from(value: super::Transfer) -> Self {
            Self {
                assignments: Ok(value.assignments),
                change_utxo: Ok(value.change_utxo),
                created_at: Ok(value.created_at),
                expiration_timestamp: Ok(value.expiration_timestamp),
                idx: Ok(value.idx),
                kind: Ok(value.kind),
                receive_utxo: Ok(value.receive_utxo),
                recipient_id: Ok(value.recipient_id),
                requested_assignment: Ok(value.requested_assignment),
                status: Ok(value.status),
                transport_endpoints: Ok(value.transport_endpoints),
                txid: Ok(value.txid),
                updated_at: Ok(value.updated_at),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TransferTransportEndpoint {
        endpoint: ::std::result::Result<::std::string::String, ::std::string::String>,
        transport_type: ::std::result::Result<super::TransportType, ::std::string::String>,
        used: ::std::result::Result<bool, ::std::string::String>,
    }
    impl ::std::default::Default for TransferTransportEndpoint {
        fn default() -> Self {
            Self {
                endpoint: Err("no value supplied for endpoint".to_string()),
                transport_type: Err("no value supplied for transport_type".to_string()),
                used: Err("no value supplied for used".to_string()),
            }
        }
    }
    impl TransferTransportEndpoint {
        pub fn endpoint<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.endpoint = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for endpoint: {e}"));
            self
        }
        pub fn transport_type<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::TransportType>,
            T::Error: ::std::fmt::Display,
        {
            self.transport_type = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for transport_type: {e}"));
            self
        }
        pub fn used<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.used = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for used: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<TransferTransportEndpoint> for super::TransferTransportEndpoint {
        type Error = super::error::ConversionError;
        fn try_from(
            value: TransferTransportEndpoint,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                endpoint: value.endpoint?,
                transport_type: value.transport_type?,
                used: value.used?,
            })
        }
    }
    impl ::std::convert::From<super::TransferTransportEndpoint> for TransferTransportEndpoint {
        fn from(value: super::TransferTransportEndpoint) -> Self {
            Self {
                endpoint: Ok(value.endpoint),
                transport_type: Ok(value.transport_type),
                used: Ok(value.used),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct UnlockRequest {
        announce_addresses:
            ::std::result::Result<::std::vec::Vec<::std::string::String>, ::std::string::String>,
        announce_alias: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        bitcoind_rpc_host: ::std::result::Result<::std::string::String, ::std::string::String>,
        bitcoind_rpc_password: ::std::result::Result<::std::string::String, ::std::string::String>,
        bitcoind_rpc_port: ::std::result::Result<i64, ::std::string::String>,
        bitcoind_rpc_username: ::std::result::Result<::std::string::String, ::std::string::String>,
        indexer_url: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        password: ::std::result::Result<::std::string::String, ::std::string::String>,
        proxy_endpoint: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for UnlockRequest {
        fn default() -> Self {
            Self {
                announce_addresses: Err("no value supplied for announce_addresses".to_string()),
                announce_alias: Ok(Default::default()),
                bitcoind_rpc_host: Err("no value supplied for bitcoind_rpc_host".to_string()),
                bitcoind_rpc_password: Err(
                    "no value supplied for bitcoind_rpc_password".to_string()
                ),
                bitcoind_rpc_port: Err("no value supplied for bitcoind_rpc_port".to_string()),
                bitcoind_rpc_username: Err(
                    "no value supplied for bitcoind_rpc_username".to_string()
                ),
                indexer_url: Ok(Default::default()),
                password: Err("no value supplied for password".to_string()),
                proxy_endpoint: Ok(Default::default()),
            }
        }
    }
    impl UnlockRequest {
        pub fn announce_addresses<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.announce_addresses = value.try_into().map_err(|e| {
                format!("error converting supplied value for announce_addresses: {e}")
            });
            self
        }
        pub fn announce_alias<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.announce_alias = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for announce_alias: {e}"));
            self
        }
        pub fn bitcoind_rpc_host<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.bitcoind_rpc_host = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for bitcoind_rpc_host: {e}"));
            self
        }
        pub fn bitcoind_rpc_password<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.bitcoind_rpc_password = value.try_into().map_err(|e| {
                format!("error converting supplied value for bitcoind_rpc_password: {e}")
            });
            self
        }
        pub fn bitcoind_rpc_port<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.bitcoind_rpc_port = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for bitcoind_rpc_port: {e}"));
            self
        }
        pub fn bitcoind_rpc_username<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.bitcoind_rpc_username = value.try_into().map_err(|e| {
                format!("error converting supplied value for bitcoind_rpc_username: {e}")
            });
            self
        }
        pub fn indexer_url<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.indexer_url = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for indexer_url: {e}"));
            self
        }
        pub fn password<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.password = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for password: {e}"));
            self
        }
        pub fn proxy_endpoint<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.proxy_endpoint = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for proxy_endpoint: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<UnlockRequest> for super::UnlockRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: UnlockRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                announce_addresses: value.announce_addresses?,
                announce_alias: value.announce_alias?,
                bitcoind_rpc_host: value.bitcoind_rpc_host?,
                bitcoind_rpc_password: value.bitcoind_rpc_password?,
                bitcoind_rpc_port: value.bitcoind_rpc_port?,
                bitcoind_rpc_username: value.bitcoind_rpc_username?,
                indexer_url: value.indexer_url?,
                password: value.password?,
                proxy_endpoint: value.proxy_endpoint?,
            })
        }
    }
    impl ::std::convert::From<super::UnlockRequest> for UnlockRequest {
        fn from(value: super::UnlockRequest) -> Self {
            Self {
                announce_addresses: Ok(value.announce_addresses),
                announce_alias: Ok(value.announce_alias),
                bitcoind_rpc_host: Ok(value.bitcoind_rpc_host),
                bitcoind_rpc_password: Ok(value.bitcoind_rpc_password),
                bitcoind_rpc_port: Ok(value.bitcoind_rpc_port),
                bitcoind_rpc_username: Ok(value.bitcoind_rpc_username),
                indexer_url: Ok(value.indexer_url),
                password: Ok(value.password),
                proxy_endpoint: Ok(value.proxy_endpoint),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Unspent {
        rgb_allocations:
            ::std::result::Result<::std::vec::Vec<super::RgbAllocation>, ::std::string::String>,
        utxo: ::std::result::Result<super::Utxo, ::std::string::String>,
    }
    impl ::std::default::Default for Unspent {
        fn default() -> Self {
            Self {
                rgb_allocations: Err("no value supplied for rgb_allocations".to_string()),
                utxo: Err("no value supplied for utxo".to_string()),
            }
        }
    }
    impl Unspent {
        pub fn rgb_allocations<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::RgbAllocation>>,
            T::Error: ::std::fmt::Display,
        {
            self.rgb_allocations = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for rgb_allocations: {e}"));
            self
        }
        pub fn utxo<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::Utxo>,
            T::Error: ::std::fmt::Display,
        {
            self.utxo = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for utxo: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Unspent> for super::Unspent {
        type Error = super::error::ConversionError;
        fn try_from(value: Unspent) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                rgb_allocations: value.rgb_allocations?,
                utxo: value.utxo?,
            })
        }
    }
    impl ::std::convert::From<super::Unspent> for Unspent {
        fn from(value: super::Unspent) -> Self {
            Self {
                rgb_allocations: Ok(value.rgb_allocations),
                utxo: Ok(value.utxo),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Utxo {
        btc_amount: ::std::result::Result<i64, ::std::string::String>,
        colorable: ::std::result::Result<bool, ::std::string::String>,
        outpoint: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for Utxo {
        fn default() -> Self {
            Self {
                btc_amount: Err("no value supplied for btc_amount".to_string()),
                colorable: Err("no value supplied for colorable".to_string()),
                outpoint: Err("no value supplied for outpoint".to_string()),
            }
        }
    }
    impl Utxo {
        pub fn btc_amount<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.btc_amount = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for btc_amount: {e}"));
            self
        }
        pub fn colorable<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.colorable = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for colorable: {e}"));
            self
        }
        pub fn outpoint<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.outpoint = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for outpoint: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<Utxo> for super::Utxo {
        type Error = super::error::ConversionError;
        fn try_from(value: Utxo) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                btc_amount: value.btc_amount?,
                colorable: value.colorable?,
                outpoint: value.outpoint?,
            })
        }
    }
    impl ::std::convert::From<super::Utxo> for Utxo {
        fn from(value: super::Utxo) -> Self {
            Self {
                btc_amount: Ok(value.btc_amount),
                colorable: Ok(value.colorable),
                outpoint: Ok(value.outpoint),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct WitnessData {
        amount_sat: ::std::result::Result<i64, ::std::string::String>,
        blinding: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
    }
    impl ::std::default::Default for WitnessData {
        fn default() -> Self {
            Self {
                amount_sat: Err("no value supplied for amount_sat".to_string()),
                blinding: Ok(Default::default()),
            }
        }
    }
    impl WitnessData {
        pub fn amount_sat<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.amount_sat = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for amount_sat: {e}"));
            self
        }
        pub fn blinding<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.blinding = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for blinding: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<WitnessData> for super::WitnessData {
        type Error = super::error::ConversionError;
        fn try_from(
            value: WitnessData,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                amount_sat: value.amount_sat?,
                blinding: value.blinding?,
            })
        }
    }
    impl ::std::convert::From<super::WitnessData> for WitnessData {
        fn from(value: super::WitnessData) -> Self {
            Self {
                amount_sat: Ok(value.amount_sat),
                blinding: Ok(value.blinding),
            }
        }
    }
}
