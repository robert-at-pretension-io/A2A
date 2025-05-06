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
#[doc = "JSON Schema for A2A Protocol"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"A2A Protocol Schema\","]
#[doc = "  \"description\": \"JSON Schema for A2A Protocol\""]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct A2aProtocolSchema(pub ::serde_json::Value);
impl ::std::ops::Deref for A2aProtocolSchema {
    type Target = ::serde_json::Value;
    fn deref(&self) -> &::serde_json::Value {
        &self.0
    }
}
impl ::std::convert::From<A2aProtocolSchema> for ::serde_json::Value {
    fn from(value: A2aProtocolSchema) -> Self {
        value.0
    }
}
impl ::std::convert::From<&A2aProtocolSchema> for A2aProtocolSchema {
    fn from(value: &A2aProtocolSchema) -> Self {
        value.clone()
    }
}
impl ::std::convert::From<::serde_json::Value> for A2aProtocolSchema {
    fn from(value: ::serde_json::Value) -> Self {
        Self(value)
    }
}
#[doc = "`A2aRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"A2ARequest\","]
#[doc = "  \"oneOf\": ["]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/SendTaskRequest\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/GetTaskRequest\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/CancelTaskRequest\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/SetTaskPushNotificationRequest\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/GetTaskPushNotificationRequest\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/TaskResubscriptionRequest\""]
#[doc = "    }"]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum A2aRequest {
    SendTaskRequest(SendTaskRequest),
    GetTaskRequest(GetTaskRequest),
    CancelTaskRequest(CancelTaskRequest),
    SetTaskPushNotificationRequest(SetTaskPushNotificationRequest),
    GetTaskPushNotificationRequest(GetTaskPushNotificationRequest),
    TaskResubscriptionRequest(TaskResubscriptionRequest),
}
impl ::std::convert::From<&Self> for A2aRequest {
    fn from(value: &A2aRequest) -> Self {
        value.clone()
    }
}
impl ::std::convert::From<SendTaskRequest> for A2aRequest {
    fn from(value: SendTaskRequest) -> Self {
        Self::SendTaskRequest(value)
    }
}
impl ::std::convert::From<GetTaskRequest> for A2aRequest {
    fn from(value: GetTaskRequest) -> Self {
        Self::GetTaskRequest(value)
    }
}
impl ::std::convert::From<CancelTaskRequest> for A2aRequest {
    fn from(value: CancelTaskRequest) -> Self {
        Self::CancelTaskRequest(value)
    }
}
impl ::std::convert::From<SetTaskPushNotificationRequest> for A2aRequest {
    fn from(value: SetTaskPushNotificationRequest) -> Self {
        Self::SetTaskPushNotificationRequest(value)
    }
}
impl ::std::convert::From<GetTaskPushNotificationRequest> for A2aRequest {
    fn from(value: GetTaskPushNotificationRequest) -> Self {
        Self::GetTaskPushNotificationRequest(value)
    }
}
impl ::std::convert::From<TaskResubscriptionRequest> for A2aRequest {
    fn from(value: TaskResubscriptionRequest) -> Self {
        Self::TaskResubscriptionRequest(value)
    }
}
#[doc = "`AgentAuthentication`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"AgentAuthentication\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"schemes\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"credentials\": {"]
#[doc = "      \"title\": \"Credentials\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"schemes\": {"]
#[doc = "      \"title\": \"Schemes\","]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"string\""]
#[doc = "      }"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AgentAuthentication {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub credentials: ::std::option::Option<::std::string::String>,
    pub schemes: ::std::vec::Vec<::std::string::String>,
}
impl ::std::convert::From<&AgentAuthentication> for AgentAuthentication {
    fn from(value: &AgentAuthentication) -> Self {
        value.clone()
    }
}
impl AgentAuthentication {
    pub fn builder() -> builder::AgentAuthentication {
        Default::default()
    }
}
#[doc = "`AgentCapabilities`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"AgentCapabilities\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"properties\": {"]
#[doc = "    \"pushNotifications\": {"]
#[doc = "      \"title\": \"PushNotifications\","]
#[doc = "      \"default\": false,"]
#[doc = "      \"type\": \"boolean\""]
#[doc = "    },"]
#[doc = "    \"stateTransitionHistory\": {"]
#[doc = "      \"title\": \"Statetransitionhistory\","]
#[doc = "      \"default\": false,"]
#[doc = "      \"type\": \"boolean\""]
#[doc = "    },"]
#[doc = "    \"streaming\": {"]
#[doc = "      \"title\": \"Streaming\","]
#[doc = "      \"default\": false,"]
#[doc = "      \"type\": \"boolean\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AgentCapabilities {
    #[serde(rename = "pushNotifications", default)]
    pub push_notifications: bool,
    #[serde(rename = "stateTransitionHistory", default)]
    pub state_transition_history: bool,
    #[serde(default)]
    pub streaming: bool,
}
impl ::std::convert::From<&AgentCapabilities> for AgentCapabilities {
    fn from(value: &AgentCapabilities) -> Self {
        value.clone()
    }
}
impl ::std::default::Default for AgentCapabilities {
    fn default() -> Self {
        Self {
            push_notifications: Default::default(),
            state_transition_history: Default::default(),
            streaming: Default::default(),
        }
    }
}
impl AgentCapabilities {
    pub fn builder() -> builder::AgentCapabilities {
        Default::default()
    }
}
#[doc = "`AgentCard`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"AgentCard\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"capabilities\","]
#[doc = "    \"name\","]
#[doc = "    \"skills\","]
#[doc = "    \"url\","]
#[doc = "    \"version\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"authentication\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/AgentAuthentication\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"capabilities\": {"]
#[doc = "      \"$ref\": \"#/$defs/AgentCapabilities\""]
#[doc = "    },"]
#[doc = "    \"defaultInputModes\": {"]
#[doc = "      \"title\": \"Defaultinputmodes\","]
#[doc = "      \"default\": ["]
#[doc = "        \"text\""]
#[doc = "      ],"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"string\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"defaultOutputModes\": {"]
#[doc = "      \"title\": \"Defaultoutputmodes\","]
#[doc = "      \"default\": ["]
#[doc = "        \"text\""]
#[doc = "      ],"]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"string\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"description\": {"]
#[doc = "      \"title\": \"Description\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"documentationUrl\": {"]
#[doc = "      \"title\": \"Documentationurl\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"title\": \"Name\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"provider\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/AgentProvider\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"skills\": {"]
#[doc = "      \"title\": \"Skills\","]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/AgentSkill\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"url\": {"]
#[doc = "      \"title\": \"Url\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"version\": {"]
#[doc = "      \"title\": \"Version\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AgentCard {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub authentication: ::std::option::Option<AgentAuthentication>,
    pub capabilities: AgentCapabilities,
    #[serde(
        rename = "defaultInputModes",
        default = "defaults::agent_card_default_input_modes"
    )]
    pub default_input_modes: ::std::vec::Vec<::std::string::String>,
    #[serde(
        rename = "defaultOutputModes",
        default = "defaults::agent_card_default_output_modes"
    )]
    pub default_output_modes: ::std::vec::Vec<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(
        rename = "documentationUrl",
        default,
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub documentation_url: ::std::option::Option<::std::string::String>,
    pub name: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub provider: ::std::option::Option<AgentProvider>,
    pub skills: ::std::vec::Vec<AgentSkill>,
    pub url: ::std::string::String,
    pub version: ::std::string::String,
}
impl ::std::convert::From<&AgentCard> for AgentCard {
    fn from(value: &AgentCard) -> Self {
        value.clone()
    }
}
impl AgentCard {
    pub fn builder() -> builder::AgentCard {
        Default::default()
    }
}
#[doc = "`AgentProvider`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"AgentProvider\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"organization\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"organization\": {"]
#[doc = "      \"title\": \"Organization\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"url\": {"]
#[doc = "      \"title\": \"Url\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
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
pub struct AgentProvider {
    pub organization: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub url: ::std::option::Option<::std::string::String>,
}
impl ::std::convert::From<&AgentProvider> for AgentProvider {
    fn from(value: &AgentProvider) -> Self {
        value.clone()
    }
}
impl AgentProvider {
    pub fn builder() -> builder::AgentProvider {
        Default::default()
    }
}
#[doc = "`AgentSkill`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"AgentSkill\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"id\","]
#[doc = "    \"name\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"description\": {"]
#[doc = "      \"title\": \"Description\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"examples\": {"]
#[doc = "      \"title\": \"Examples\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"array\","]
#[doc = "          \"items\": {"]
#[doc = "            \"type\": \"string\""]
#[doc = "          }"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"inputModes\": {"]
#[doc = "      \"title\": \"Inputmodes\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"array\","]
#[doc = "          \"items\": {"]
#[doc = "            \"type\": \"string\""]
#[doc = "          }"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"title\": \"Name\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"outputModes\": {"]
#[doc = "      \"title\": \"Outputmodes\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"array\","]
#[doc = "          \"items\": {"]
#[doc = "            \"type\": \"string\""]
#[doc = "          }"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"tags\": {"]
#[doc = "      \"title\": \"Tags\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"array\","]
#[doc = "          \"items\": {"]
#[doc = "            \"type\": \"string\""]
#[doc = "          }"]
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
pub struct AgentSkill {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub examples: ::std::option::Option<::std::vec::Vec<::std::string::String>>,
    pub id: ::std::string::String,
    #[serde(
        rename = "inputModes",
        default,
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub input_modes: ::std::option::Option<::std::vec::Vec<::std::string::String>>,
    pub name: ::std::string::String,
    #[serde(
        rename = "outputModes",
        default,
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub output_modes: ::std::option::Option<::std::vec::Vec<::std::string::String>>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub tags: ::std::option::Option<::std::vec::Vec<::std::string::String>>,
}
impl ::std::convert::From<&AgentSkill> for AgentSkill {
    fn from(value: &AgentSkill) -> Self {
        value.clone()
    }
}
impl AgentSkill {
    pub fn builder() -> builder::AgentSkill {
        Default::default()
    }
}
#[doc = "`Artifact`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"Artifact\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"parts\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"append\": {"]
#[doc = "      \"title\": \"Append\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"boolean\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"description\": {"]
#[doc = "      \"title\": \"Description\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"index\": {"]
#[doc = "      \"title\": \"Index\","]
#[doc = "      \"default\": 0,"]
#[doc = "      \"type\": \"integer\""]
#[doc = "    },"]
#[doc = "    \"lastChunk\": {"]
#[doc = "      \"title\": \"LastChunk\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"boolean\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"metadata\": {"]
#[doc = "      \"title\": \"Metadata\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"title\": \"Name\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"parts\": {"]
#[doc = "      \"title\": \"Parts\","]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/Part\""]
#[doc = "      }"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Artifact {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub append: ::std::option::Option<bool>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(default)]
    pub index: i64,
    #[serde(
        rename = "lastChunk",
        default,
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub last_chunk: ::std::option::Option<bool>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub metadata:
        ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub name: ::std::option::Option<::std::string::String>,
    pub parts: ::std::vec::Vec<Part>,
}
impl ::std::convert::From<&Artifact> for Artifact {
    fn from(value: &Artifact) -> Self {
        value.clone()
    }
}
impl Artifact {
    pub fn builder() -> builder::Artifact {
        Default::default()
    }
}
#[doc = "`AuthenticationInfo`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"AuthenticationInfo\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"schemes\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"credentials\": {"]
#[doc = "      \"title\": \"Credentials\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"schemes\": {"]
#[doc = "      \"title\": \"Schemes\","]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"string\""]
#[doc = "      }"]
#[doc = "    }"]
#[doc = "  },"]
#[doc = "  \"additionalProperties\": {}"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct AuthenticationInfo {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub credentials: ::std::option::Option<::std::string::String>,
    pub schemes: ::std::vec::Vec<::std::string::String>,
    #[serde(flatten)]
    pub extra: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}
impl ::std::convert::From<&AuthenticationInfo> for AuthenticationInfo {
    fn from(value: &AuthenticationInfo) -> Self {
        value.clone()
    }
}
impl AuthenticationInfo {
    pub fn builder() -> builder::AuthenticationInfo {
        Default::default()
    }
}
#[doc = "`CancelTaskRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"CancelTaskRequest\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"method\","]
#[doc = "    \"params\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"method\": {"]
#[doc = "      \"title\": \"Method\","]
#[doc = "      \"default\": \"tasks/cancel\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"tasks/cancel\""]
#[doc = "    },"]
#[doc = "    \"params\": {"]
#[doc = "      \"$ref\": \"#/$defs/TaskIdParams\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct CancelTaskRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::cancel_task_request_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    pub method: ::std::string::String,
    pub params: TaskIdParams,
}
impl ::std::convert::From<&CancelTaskRequest> for CancelTaskRequest {
    fn from(value: &CancelTaskRequest) -> Self {
        value.clone()
    }
}
impl CancelTaskRequest {
    pub fn builder() -> builder::CancelTaskRequest {
        Default::default()
    }
}
#[doc = "`CancelTaskResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"CancelTaskResponse\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"properties\": {"]
#[doc = "    \"error\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/JSONRPCError\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"result\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/Task\""]
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
pub struct CancelTaskResponse {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub error: ::std::option::Option<JsonrpcError>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::cancel_task_response_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub result: ::std::option::Option<Task>,
}
impl ::std::convert::From<&CancelTaskResponse> for CancelTaskResponse {
    fn from(value: &CancelTaskResponse) -> Self {
        value.clone()
    }
}
impl ::std::default::Default for CancelTaskResponse {
    fn default() -> Self {
        Self {
            error: Default::default(),
            id: Default::default(),
            jsonrpc: defaults::cancel_task_response_jsonrpc(),
            result: Default::default(),
        }
    }
}
impl CancelTaskResponse {
    pub fn builder() -> builder::CancelTaskResponse {
        Default::default()
    }
}
#[doc = "`DataPart`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"DataPart\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"data\","]
#[doc = "    \"type\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"data\": {"]
#[doc = "      \"title\": \"Data\","]
#[doc = "      \"type\": \"object\","]
#[doc = "      \"additionalProperties\": {}"]
#[doc = "    },"]
#[doc = "    \"metadata\": {"]
#[doc = "      \"title\": \"Metadata\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"type\": {"]
#[doc = "      \"title\": \"Type\","]
#[doc = "      \"description\": \"Type of the part\","]
#[doc = "      \"default\": \"data\","]
#[doc = "      \"examples\": ["]
#[doc = "        \"data\""]
#[doc = "      ],"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"data\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct DataPart {
    pub data: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub metadata:
        ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    #[doc = "Type of the part"]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
impl ::std::convert::From<&DataPart> for DataPart {
    fn from(value: &DataPart) -> Self {
        value.clone()
    }
}
impl DataPart {
    pub fn builder() -> builder::DataPart {
        Default::default()
    }
}
#[doc = "Represents the content of a file, either as base64 encoded bytes or a URI.\n\nEnsures that either 'bytes' or 'uri' is provided, but not both."]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"FileContent\","]
#[doc = "  \"description\": \"Represents the content of a file, either as base64 encoded bytes or a URI.\\n\\nEnsures that either 'bytes' or 'uri' is provided, but not both.\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"properties\": {"]
#[doc = "    \"bytes\": {"]
#[doc = "      \"title\": \"Bytes\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"mimeType\": {"]
#[doc = "      \"title\": \"Mimetype\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"name\": {"]
#[doc = "      \"title\": \"Name\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"uri\": {"]
#[doc = "      \"title\": \"Uri\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
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
pub struct FileContent {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub bytes: ::std::option::Option<::std::string::String>,
    #[serde(
        rename = "mimeType",
        default,
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub mime_type: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub name: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub uri: ::std::option::Option<::std::string::String>,
}
impl ::std::convert::From<&FileContent> for FileContent {
    fn from(value: &FileContent) -> Self {
        value.clone()
    }
}
impl ::std::default::Default for FileContent {
    fn default() -> Self {
        Self {
            bytes: Default::default(),
            mime_type: Default::default(),
            name: Default::default(),
            uri: Default::default(),
        }
    }
}
impl FileContent {
    pub fn builder() -> builder::FileContent {
        Default::default()
    }
}
#[doc = "`FilePart`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"FilePart\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"file\","]
#[doc = "    \"type\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"file\": {"]
#[doc = "      \"$ref\": \"#/$defs/FileContent\""]
#[doc = "    },"]
#[doc = "    \"metadata\": {"]
#[doc = "      \"title\": \"Metadata\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"type\": {"]
#[doc = "      \"title\": \"Type\","]
#[doc = "      \"description\": \"Type of the part\","]
#[doc = "      \"default\": \"file\","]
#[doc = "      \"examples\": ["]
#[doc = "        \"file\""]
#[doc = "      ],"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"file\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct FilePart {
    pub file: FileContent,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub metadata:
        ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    #[doc = "Type of the part"]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
impl ::std::convert::From<&FilePart> for FilePart {
    fn from(value: &FilePart) -> Self {
        value.clone()
    }
}
impl FilePart {
    pub fn builder() -> builder::FilePart {
        Default::default()
    }
}
#[doc = "`GetTaskPushNotificationRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"GetTaskPushNotificationRequest\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"method\","]
#[doc = "    \"params\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"method\": {"]
#[doc = "      \"title\": \"Method\","]
#[doc = "      \"default\": \"tasks/pushNotification/get\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"tasks/pushNotification/get\""]
#[doc = "    },"]
#[doc = "    \"params\": {"]
#[doc = "      \"$ref\": \"#/$defs/TaskIdParams\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct GetTaskPushNotificationRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::get_task_push_notification_request_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    pub method: ::std::string::String,
    pub params: TaskIdParams,
}
impl ::std::convert::From<&GetTaskPushNotificationRequest> for GetTaskPushNotificationRequest {
    fn from(value: &GetTaskPushNotificationRequest) -> Self {
        value.clone()
    }
}
impl GetTaskPushNotificationRequest {
    pub fn builder() -> builder::GetTaskPushNotificationRequest {
        Default::default()
    }
}
#[doc = "`GetTaskPushNotificationResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"GetTaskPushNotificationResponse\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"properties\": {"]
#[doc = "    \"error\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/JSONRPCError\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"result\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/TaskPushNotificationConfig\""]
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
pub struct GetTaskPushNotificationResponse {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub error: ::std::option::Option<JsonrpcError>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::get_task_push_notification_response_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub result: ::std::option::Option<TaskPushNotificationConfig>,
}
impl ::std::convert::From<&GetTaskPushNotificationResponse> for GetTaskPushNotificationResponse {
    fn from(value: &GetTaskPushNotificationResponse) -> Self {
        value.clone()
    }
}
impl ::std::default::Default for GetTaskPushNotificationResponse {
    fn default() -> Self {
        Self {
            error: Default::default(),
            id: Default::default(),
            jsonrpc: defaults::get_task_push_notification_response_jsonrpc(),
            result: Default::default(),
        }
    }
}
impl GetTaskPushNotificationResponse {
    pub fn builder() -> builder::GetTaskPushNotificationResponse {
        Default::default()
    }
}
#[doc = "`GetTaskRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"GetTaskRequest\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"method\","]
#[doc = "    \"params\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"method\": {"]
#[doc = "      \"title\": \"Method\","]
#[doc = "      \"default\": \"tasks/get\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"tasks/get\""]
#[doc = "    },"]
#[doc = "    \"params\": {"]
#[doc = "      \"$ref\": \"#/$defs/TaskQueryParams\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct GetTaskRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::get_task_request_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    pub method: ::std::string::String,
    pub params: TaskQueryParams,
}
impl ::std::convert::From<&GetTaskRequest> for GetTaskRequest {
    fn from(value: &GetTaskRequest) -> Self {
        value.clone()
    }
}
impl GetTaskRequest {
    pub fn builder() -> builder::GetTaskRequest {
        Default::default()
    }
}
#[doc = "`GetTaskResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"GetTaskResponse\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"properties\": {"]
#[doc = "    \"error\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/JSONRPCError\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"result\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/Task\""]
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
pub struct GetTaskResponse {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub error: ::std::option::Option<JsonrpcError>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::get_task_response_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub result: ::std::option::Option<Task>,
}
impl ::std::convert::From<&GetTaskResponse> for GetTaskResponse {
    fn from(value: &GetTaskResponse) -> Self {
        value.clone()
    }
}
impl ::std::default::Default for GetTaskResponse {
    fn default() -> Self {
        Self {
            error: Default::default(),
            id: Default::default(),
            jsonrpc: defaults::get_task_response_jsonrpc(),
            result: Default::default(),
        }
    }
}
impl GetTaskResponse {
    pub fn builder() -> builder::GetTaskResponse {
        Default::default()
    }
}
#[doc = "`Id`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"Id\","]
#[doc = "  \"anyOf\": ["]
#[doc = "    {"]
#[doc = "      \"type\": \"integer\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"type\": \"null\""]
#[doc = "    }"]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum Id {
    Variant0(i64),
    Variant1(::std::string::String),
    Variant2,
}
impl ::std::convert::From<&Self> for Id {
    fn from(value: &Id) -> Self {
        value.clone()
    }
}
impl ::std::convert::From<i64> for Id {
    fn from(value: i64) -> Self {
        Self::Variant0(value)
    }
}
#[doc = "`InternalError`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"InternalError\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"code\","]
#[doc = "    \"message\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"code\": {"]
#[doc = "      \"title\": \"Code\","]
#[doc = "      \"description\": \"Error code\","]
#[doc = "      \"default\": -32603,"]
#[doc = "      \"examples\": ["]
#[doc = "        -32603"]
#[doc = "      ],"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"const\": -32603"]
#[doc = "    },"]
#[doc = "    \"data\": {"]
#[doc = "      \"title\": \"Data\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"message\": {"]
#[doc = "      \"title\": \"Message\","]
#[doc = "      \"description\": \"A short description of the error\","]
#[doc = "      \"default\": \"Internal error\","]
#[doc = "      \"examples\": ["]
#[doc = "        \"Internal error\""]
#[doc = "      ],"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"Internal error\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InternalError {
    #[doc = "Error code"]
    pub code: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub data: ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    #[doc = "A short description of the error"]
    pub message: ::std::string::String,
}
impl ::std::convert::From<&InternalError> for InternalError {
    fn from(value: &InternalError) -> Self {
        value.clone()
    }
}
impl InternalError {
    pub fn builder() -> builder::InternalError {
        Default::default()
    }
}
#[doc = "`InvalidParamsError`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"InvalidParamsError\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"code\","]
#[doc = "    \"message\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"code\": {"]
#[doc = "      \"title\": \"Code\","]
#[doc = "      \"description\": \"Error code\","]
#[doc = "      \"default\": -32602,"]
#[doc = "      \"examples\": ["]
#[doc = "        -32602"]
#[doc = "      ],"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"const\": -32602"]
#[doc = "    },"]
#[doc = "    \"data\": {"]
#[doc = "      \"title\": \"Data\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"message\": {"]
#[doc = "      \"title\": \"Message\","]
#[doc = "      \"description\": \"A short description of the error\","]
#[doc = "      \"default\": \"Invalid parameters\","]
#[doc = "      \"examples\": ["]
#[doc = "        \"Invalid parameters\""]
#[doc = "      ],"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"Invalid parameters\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InvalidParamsError {
    #[doc = "Error code"]
    pub code: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub data: ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    #[doc = "A short description of the error"]
    pub message: ::std::string::String,
}
impl ::std::convert::From<&InvalidParamsError> for InvalidParamsError {
    fn from(value: &InvalidParamsError) -> Self {
        value.clone()
    }
}
impl InvalidParamsError {
    pub fn builder() -> builder::InvalidParamsError {
        Default::default()
    }
}
#[doc = "`InvalidRequestError`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"InvalidRequestError\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"code\","]
#[doc = "    \"message\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"code\": {"]
#[doc = "      \"title\": \"Code\","]
#[doc = "      \"description\": \"Error code\","]
#[doc = "      \"default\": -32600,"]
#[doc = "      \"examples\": ["]
#[doc = "        -32600"]
#[doc = "      ],"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"const\": -32600"]
#[doc = "    },"]
#[doc = "    \"data\": {"]
#[doc = "      \"title\": \"Data\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"message\": {"]
#[doc = "      \"title\": \"Message\","]
#[doc = "      \"description\": \"A short description of the error\","]
#[doc = "      \"default\": \"Request payload validation error\","]
#[doc = "      \"examples\": ["]
#[doc = "        \"Request payload validation error\""]
#[doc = "      ],"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"Request payload validation error\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct InvalidRequestError {
    #[doc = "Error code"]
    pub code: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub data: ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    #[doc = "A short description of the error"]
    pub message: ::std::string::String,
}
impl ::std::convert::From<&InvalidRequestError> for InvalidRequestError {
    fn from(value: &InvalidRequestError) -> Self {
        value.clone()
    }
}
impl InvalidRequestError {
    pub fn builder() -> builder::InvalidRequestError {
        Default::default()
    }
}
#[doc = "`JsonParseError`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"JSONParseError\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"code\","]
#[doc = "    \"message\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"code\": {"]
#[doc = "      \"title\": \"Code\","]
#[doc = "      \"description\": \"Error code\","]
#[doc = "      \"default\": -32700,"]
#[doc = "      \"examples\": ["]
#[doc = "        -32700"]
#[doc = "      ],"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"const\": -32700"]
#[doc = "    },"]
#[doc = "    \"data\": {"]
#[doc = "      \"title\": \"Data\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"message\": {"]
#[doc = "      \"title\": \"Message\","]
#[doc = "      \"description\": \"A short description of the error\","]
#[doc = "      \"default\": \"Invalid JSON payload\","]
#[doc = "      \"examples\": ["]
#[doc = "        \"Invalid JSON payload\""]
#[doc = "      ],"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"Invalid JSON payload\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct JsonParseError {
    #[doc = "Error code"]
    pub code: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub data: ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    #[doc = "A short description of the error"]
    pub message: ::std::string::String,
}
impl ::std::convert::From<&JsonParseError> for JsonParseError {
    fn from(value: &JsonParseError) -> Self {
        value.clone()
    }
}
impl JsonParseError {
    pub fn builder() -> builder::JsonParseError {
        Default::default()
    }
}
#[doc = "`JsonrpcError`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"JSONRPCError\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"code\","]
#[doc = "    \"message\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"code\": {"]
#[doc = "      \"title\": \"Code\","]
#[doc = "      \"type\": \"integer\""]
#[doc = "    },"]
#[doc = "    \"data\": {"]
#[doc = "      \"title\": \"Data\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"message\": {"]
#[doc = "      \"title\": \"Message\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct JsonrpcError {
    pub code: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub data: ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    pub message: ::std::string::String,
}
impl ::std::convert::From<&JsonrpcError> for JsonrpcError {
    fn from(value: &JsonrpcError) -> Self {
        value.clone()
    }
}
impl JsonrpcError {
    pub fn builder() -> builder::JsonrpcError {
        Default::default()
    }
}
#[doc = "`JsonrpcMessage`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"JSONRPCMessage\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"properties\": {"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct JsonrpcMessage {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::jsonrpc_message_jsonrpc")]
    pub jsonrpc: ::std::string::String,
}
impl ::std::convert::From<&JsonrpcMessage> for JsonrpcMessage {
    fn from(value: &JsonrpcMessage) -> Self {
        value.clone()
    }
}
impl ::std::default::Default for JsonrpcMessage {
    fn default() -> Self {
        Self {
            id: Default::default(),
            jsonrpc: defaults::jsonrpc_message_jsonrpc(),
        }
    }
}
impl JsonrpcMessage {
    pub fn builder() -> builder::JsonrpcMessage {
        Default::default()
    }
}
#[doc = "`JsonrpcRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"JSONRPCRequest\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"method\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"method\": {"]
#[doc = "      \"title\": \"Method\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"params\": {"]
#[doc = "      \"title\": \"Params\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
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
pub struct JsonrpcRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::jsonrpc_request_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    pub method: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub params:
        ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
}
impl ::std::convert::From<&JsonrpcRequest> for JsonrpcRequest {
    fn from(value: &JsonrpcRequest) -> Self {
        value.clone()
    }
}
impl JsonrpcRequest {
    pub fn builder() -> builder::JsonrpcRequest {
        Default::default()
    }
}
#[doc = "`JsonrpcResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"JSONRPCResponse\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"properties\": {"]
#[doc = "    \"error\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/JSONRPCError\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"result\": {"]
#[doc = "      \"title\": \"Result\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
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
pub struct JsonrpcResponse {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub error: ::std::option::Option<JsonrpcError>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::jsonrpc_response_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub result:
        ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
}
impl ::std::convert::From<&JsonrpcResponse> for JsonrpcResponse {
    fn from(value: &JsonrpcResponse) -> Self {
        value.clone()
    }
}
impl ::std::default::Default for JsonrpcResponse {
    fn default() -> Self {
        Self {
            error: Default::default(),
            id: Default::default(),
            jsonrpc: defaults::jsonrpc_response_jsonrpc(),
            result: Default::default(),
        }
    }
}
impl JsonrpcResponse {
    pub fn builder() -> builder::JsonrpcResponse {
        Default::default()
    }
}
#[doc = "`Message`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"Message\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"parts\","]
#[doc = "    \"role\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"metadata\": {"]
#[doc = "      \"title\": \"Metadata\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"parts\": {"]
#[doc = "      \"title\": \"Parts\","]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"$ref\": \"#/$defs/Part\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"role\": {"]
#[doc = "      \"title\": \"Role\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"enum\": ["]
#[doc = "        \"user\","]
#[doc = "        \"agent\""]
#[doc = "      ]"]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Message {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub metadata:
        ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    pub parts: ::std::vec::Vec<Part>,
    pub role: Role,
}
impl ::std::convert::From<&Message> for Message {
    fn from(value: &Message) -> Self {
        value.clone()
    }
}
impl Message {
    pub fn builder() -> builder::Message {
        Default::default()
    }
}
#[doc = "`MethodNotFoundError`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"MethodNotFoundError\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"code\","]
#[doc = "    \"data\","]
#[doc = "    \"message\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"code\": {"]
#[doc = "      \"title\": \"Code\","]
#[doc = "      \"description\": \"Error code\","]
#[doc = "      \"default\": -32601,"]
#[doc = "      \"examples\": ["]
#[doc = "        -32601"]
#[doc = "      ],"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"const\": -32601"]
#[doc = "    },"]
#[doc = "    \"data\": {"]
#[doc = "      \"title\": \"Data\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"const\": null"]
#[doc = "    },"]
#[doc = "    \"message\": {"]
#[doc = "      \"title\": \"Message\","]
#[doc = "      \"description\": \"A short description of the error\","]
#[doc = "      \"default\": \"Method not found\","]
#[doc = "      \"examples\": ["]
#[doc = "        \"Method not found\""]
#[doc = "      ],"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"Method not found\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct MethodNotFoundError {
    #[doc = "Error code"]
    pub code: i64,
    pub data: ::serde_json::Value,
    #[doc = "A short description of the error"]
    pub message: ::std::string::String,
}
impl ::std::convert::From<&MethodNotFoundError> for MethodNotFoundError {
    fn from(value: &MethodNotFoundError) -> Self {
        value.clone()
    }
}
impl MethodNotFoundError {
    pub fn builder() -> builder::MethodNotFoundError {
        Default::default()
    }
}
#[doc = "`Part`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"Part\","]
#[doc = "  \"anyOf\": ["]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/TextPart\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/FilePart\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/DataPart\""]
#[doc = "    }"]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum Part {
    TextPart(TextPart),
    FilePart(FilePart),
    DataPart(DataPart),
}
impl ::std::convert::From<&Self> for Part {
    fn from(value: &Part) -> Self {
        value.clone()
    }
}
impl ::std::convert::From<TextPart> for Part {
    fn from(value: TextPart) -> Self {
        Self::TextPart(value)
    }
}
impl ::std::convert::From<FilePart> for Part {
    fn from(value: FilePart) -> Self {
        Self::FilePart(value)
    }
}
impl ::std::convert::From<DataPart> for Part {
    fn from(value: DataPart) -> Self {
        Self::DataPart(value)
    }
}
#[doc = "`PushNotificationConfig`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"PushNotificationConfig\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"url\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"authentication\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/AuthenticationInfo\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"token\": {"]
#[doc = "      \"title\": \"Token\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"url\": {"]
#[doc = "      \"title\": \"Url\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct PushNotificationConfig {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub authentication: ::std::option::Option<AuthenticationInfo>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub token: ::std::option::Option<::std::string::String>,
    pub url: ::std::string::String,
}
impl ::std::convert::From<&PushNotificationConfig> for PushNotificationConfig {
    fn from(value: &PushNotificationConfig) -> Self {
        value.clone()
    }
}
impl PushNotificationConfig {
    pub fn builder() -> builder::PushNotificationConfig {
        Default::default()
    }
}
#[doc = "`PushNotificationNotSupportedError`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"PushNotificationNotSupportedError\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"code\","]
#[doc = "    \"data\","]
#[doc = "    \"message\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"code\": {"]
#[doc = "      \"title\": \"Code\","]
#[doc = "      \"description\": \"Error code\","]
#[doc = "      \"default\": -32003,"]
#[doc = "      \"examples\": ["]
#[doc = "        -32003"]
#[doc = "      ],"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"const\": -32003"]
#[doc = "    },"]
#[doc = "    \"data\": {"]
#[doc = "      \"title\": \"Data\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"const\": null"]
#[doc = "    },"]
#[doc = "    \"message\": {"]
#[doc = "      \"title\": \"Message\","]
#[doc = "      \"description\": \"A short description of the error\","]
#[doc = "      \"default\": \"Push Notification is not supported\","]
#[doc = "      \"examples\": ["]
#[doc = "        \"Push Notification is not supported\""]
#[doc = "      ],"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"Push Notification is not supported\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct PushNotificationNotSupportedError {
    #[doc = "Error code"]
    pub code: i64,
    pub data: ::serde_json::Value,
    #[doc = "A short description of the error"]
    pub message: ::std::string::String,
}
impl ::std::convert::From<&PushNotificationNotSupportedError>
    for PushNotificationNotSupportedError
{
    fn from(value: &PushNotificationNotSupportedError) -> Self {
        value.clone()
    }
}
impl PushNotificationNotSupportedError {
    pub fn builder() -> builder::PushNotificationNotSupportedError {
        Default::default()
    }
}
#[doc = "`Role`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"Role\","]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"user\","]
#[doc = "    \"agent\""]
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
pub enum Role {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "agent")]
    Agent,
}
impl ::std::convert::From<&Self> for Role {
    fn from(value: &Role) -> Self {
        value.clone()
    }
}
impl ::std::fmt::Display for Role {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::User => write!(f, "user"),
            Self::Agent => write!(f, "agent"),
        }
    }
}
impl ::std::str::FromStr for Role {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "user" => Ok(Self::User),
            "agent" => Ok(Self::Agent),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for Role {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for Role {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for Role {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SendTaskRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"SendTaskRequest\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"method\","]
#[doc = "    \"params\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"method\": {"]
#[doc = "      \"title\": \"Method\","]
#[doc = "      \"default\": \"tasks/send\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"tasks/send\""]
#[doc = "    },"]
#[doc = "    \"params\": {"]
#[doc = "      \"$ref\": \"#/$defs/TaskSendParams\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SendTaskRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::send_task_request_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    pub method: ::std::string::String,
    pub params: TaskSendParams,
}
impl ::std::convert::From<&SendTaskRequest> for SendTaskRequest {
    fn from(value: &SendTaskRequest) -> Self {
        value.clone()
    }
}
impl SendTaskRequest {
    pub fn builder() -> builder::SendTaskRequest {
        Default::default()
    }
}
#[doc = "`SendTaskResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"SendTaskResponse\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"properties\": {"]
#[doc = "    \"error\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/JSONRPCError\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"result\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/Task\""]
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
pub struct SendTaskResponse {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub error: ::std::option::Option<JsonrpcError>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::send_task_response_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub result: ::std::option::Option<Task>,
}
impl ::std::convert::From<&SendTaskResponse> for SendTaskResponse {
    fn from(value: &SendTaskResponse) -> Self {
        value.clone()
    }
}
impl ::std::default::Default for SendTaskResponse {
    fn default() -> Self {
        Self {
            error: Default::default(),
            id: Default::default(),
            jsonrpc: defaults::send_task_response_jsonrpc(),
            result: Default::default(),
        }
    }
}
impl SendTaskResponse {
    pub fn builder() -> builder::SendTaskResponse {
        Default::default()
    }
}
#[doc = "`SendTaskStreamingRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"SendTaskStreamingRequest\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"method\","]
#[doc = "    \"params\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"method\": {"]
#[doc = "      \"title\": \"Method\","]
#[doc = "      \"default\": \"tasks/sendSubscribe\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"tasks/sendSubscribe\""]
#[doc = "    },"]
#[doc = "    \"params\": {"]
#[doc = "      \"$ref\": \"#/$defs/TaskSendParams\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SendTaskStreamingRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::send_task_streaming_request_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    pub method: ::std::string::String,
    pub params: TaskSendParams,
}
impl ::std::convert::From<&SendTaskStreamingRequest> for SendTaskStreamingRequest {
    fn from(value: &SendTaskStreamingRequest) -> Self {
        value.clone()
    }
}
impl SendTaskStreamingRequest {
    pub fn builder() -> builder::SendTaskStreamingRequest {
        Default::default()
    }
}
#[doc = "`SendTaskStreamingResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"SendTaskStreamingResponse\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"properties\": {"]
#[doc = "    \"error\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/JSONRPCError\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"result\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/TaskStatusUpdateEvent\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/TaskArtifactUpdateEvent\""]
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
pub struct SendTaskStreamingResponse {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub error: ::std::option::Option<JsonrpcError>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::send_task_streaming_response_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    #[serde(default = "defaults::send_task_streaming_response_result")]
    pub result: SendTaskStreamingResponseResult,
}
impl ::std::convert::From<&SendTaskStreamingResponse> for SendTaskStreamingResponse {
    fn from(value: &SendTaskStreamingResponse) -> Self {
        value.clone()
    }
}
impl ::std::default::Default for SendTaskStreamingResponse {
    fn default() -> Self {
        Self {
            error: Default::default(),
            id: Default::default(),
            jsonrpc: defaults::send_task_streaming_response_jsonrpc(),
            result: defaults::send_task_streaming_response_result(),
        }
    }
}
impl SendTaskStreamingResponse {
    pub fn builder() -> builder::SendTaskStreamingResponse {
        Default::default()
    }
}
#[doc = "`SendTaskStreamingResponseResult`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"default\": null,"]
#[doc = "  \"anyOf\": ["]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/TaskStatusUpdateEvent\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/TaskArtifactUpdateEvent\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"type\": \"null\""]
#[doc = "    }"]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum SendTaskStreamingResponseResult {
    Variant0(TaskStatusUpdateEvent),
    Variant1(TaskArtifactUpdateEvent),
    Variant2,
}
impl ::std::convert::From<&Self> for SendTaskStreamingResponseResult {
    fn from(value: &SendTaskStreamingResponseResult) -> Self {
        value.clone()
    }
}
impl ::std::default::Default for SendTaskStreamingResponseResult {
    fn default() -> Self {
        SendTaskStreamingResponseResult::Variant2
    }
}
impl ::std::convert::From<TaskStatusUpdateEvent> for SendTaskStreamingResponseResult {
    fn from(value: TaskStatusUpdateEvent) -> Self {
        Self::Variant0(value)
    }
}
impl ::std::convert::From<TaskArtifactUpdateEvent> for SendTaskStreamingResponseResult {
    fn from(value: TaskArtifactUpdateEvent) -> Self {
        Self::Variant1(value)
    }
}
#[doc = "`SetTaskPushNotificationRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"SetTaskPushNotificationRequest\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"method\","]
#[doc = "    \"params\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"method\": {"]
#[doc = "      \"title\": \"Method\","]
#[doc = "      \"default\": \"tasks/pushNotification/set\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"tasks/pushNotification/set\""]
#[doc = "    },"]
#[doc = "    \"params\": {"]
#[doc = "      \"$ref\": \"#/$defs/TaskPushNotificationConfig\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct SetTaskPushNotificationRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::set_task_push_notification_request_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    pub method: ::std::string::String,
    pub params: TaskPushNotificationConfig,
}
impl ::std::convert::From<&SetTaskPushNotificationRequest> for SetTaskPushNotificationRequest {
    fn from(value: &SetTaskPushNotificationRequest) -> Self {
        value.clone()
    }
}
impl SetTaskPushNotificationRequest {
    pub fn builder() -> builder::SetTaskPushNotificationRequest {
        Default::default()
    }
}
#[doc = "`SetTaskPushNotificationResponse`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"SetTaskPushNotificationResponse\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"properties\": {"]
#[doc = "    \"error\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/JSONRPCError\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"result\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/TaskPushNotificationConfig\""]
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
pub struct SetTaskPushNotificationResponse {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub error: ::std::option::Option<JsonrpcError>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::set_task_push_notification_response_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub result: ::std::option::Option<TaskPushNotificationConfig>,
}
impl ::std::convert::From<&SetTaskPushNotificationResponse> for SetTaskPushNotificationResponse {
    fn from(value: &SetTaskPushNotificationResponse) -> Self {
        value.clone()
    }
}
impl ::std::default::Default for SetTaskPushNotificationResponse {
    fn default() -> Self {
        Self {
            error: Default::default(),
            id: Default::default(),
            jsonrpc: defaults::set_task_push_notification_response_jsonrpc(),
            result: Default::default(),
        }
    }
}
impl SetTaskPushNotificationResponse {
    pub fn builder() -> builder::SetTaskPushNotificationResponse {
        Default::default()
    }
}
#[doc = "`Task`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"Task\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"id\","]
#[doc = "    \"status\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"artifacts\": {"]
#[doc = "      \"title\": \"Artifacts\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"array\","]
#[doc = "          \"items\": {"]
#[doc = "            \"$ref\": \"#/$defs/Artifact\""]
#[doc = "          }"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"history\": {"]
#[doc = "      \"title\": \"History\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"array\","]
#[doc = "          \"items\": {"]
#[doc = "            \"$ref\": \"#/$defs/Message\""]
#[doc = "          }"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"metadata\": {"]
#[doc = "      \"title\": \"Metadata\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"sessionId\": {"]
#[doc = "      \"title\": \"Sessionid\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"status\": {"]
#[doc = "      \"$ref\": \"#/$defs/TaskStatus\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct Task {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub artifacts: ::std::option::Option<::std::vec::Vec<Artifact>>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub history: ::std::option::Option<::std::vec::Vec<Message>>,
    pub id: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub metadata:
        ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    #[serde(
        rename = "sessionId",
        default,
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub session_id: ::std::option::Option<::std::string::String>,
    pub status: TaskStatus,
}
impl ::std::convert::From<&Task> for Task {
    fn from(value: &Task) -> Self {
        value.clone()
    }
}
impl Task {
    pub fn builder() -> builder::Task {
        Default::default()
    }
}
#[doc = "`TaskArtifactUpdateEvent`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"TaskArtifactUpdateEvent\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"artifact\","]
#[doc = "    \"id\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"artifact\": {"]
#[doc = "      \"$ref\": \"#/$defs/Artifact\""]
#[doc = "    },"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"metadata\": {"]
#[doc = "      \"title\": \"Metadata\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
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
pub struct TaskArtifactUpdateEvent {
    pub artifact: Artifact,
    pub id: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub metadata:
        ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
}
impl ::std::convert::From<&TaskArtifactUpdateEvent> for TaskArtifactUpdateEvent {
    fn from(value: &TaskArtifactUpdateEvent) -> Self {
        value.clone()
    }
}
impl TaskArtifactUpdateEvent {
    pub fn builder() -> builder::TaskArtifactUpdateEvent {
        Default::default()
    }
}
#[doc = "`TaskIdParams`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"TaskIdParams\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"id\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"metadata\": {"]
#[doc = "      \"title\": \"Metadata\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
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
pub struct TaskIdParams {
    pub id: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub metadata:
        ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
}
impl ::std::convert::From<&TaskIdParams> for TaskIdParams {
    fn from(value: &TaskIdParams) -> Self {
        value.clone()
    }
}
impl TaskIdParams {
    pub fn builder() -> builder::TaskIdParams {
        Default::default()
    }
}
#[doc = "`TaskNotCancelableError`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"TaskNotCancelableError\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"code\","]
#[doc = "    \"data\","]
#[doc = "    \"message\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"code\": {"]
#[doc = "      \"title\": \"Code\","]
#[doc = "      \"description\": \"Error code\","]
#[doc = "      \"default\": -32002,"]
#[doc = "      \"examples\": ["]
#[doc = "        -32002"]
#[doc = "      ],"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"const\": -32002"]
#[doc = "    },"]
#[doc = "    \"data\": {"]
#[doc = "      \"title\": \"Data\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"const\": null"]
#[doc = "    },"]
#[doc = "    \"message\": {"]
#[doc = "      \"title\": \"Message\","]
#[doc = "      \"description\": \"A short description of the error\","]
#[doc = "      \"default\": \"Task cannot be canceled\","]
#[doc = "      \"examples\": ["]
#[doc = "        \"Task cannot be canceled\""]
#[doc = "      ],"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"Task cannot be canceled\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct TaskNotCancelableError {
    #[doc = "Error code"]
    pub code: i64,
    pub data: ::serde_json::Value,
    #[doc = "A short description of the error"]
    pub message: ::std::string::String,
}
impl ::std::convert::From<&TaskNotCancelableError> for TaskNotCancelableError {
    fn from(value: &TaskNotCancelableError) -> Self {
        value.clone()
    }
}
impl TaskNotCancelableError {
    pub fn builder() -> builder::TaskNotCancelableError {
        Default::default()
    }
}
#[doc = "`TaskNotFoundError`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"TaskNotFoundError\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"code\","]
#[doc = "    \"data\","]
#[doc = "    \"message\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"code\": {"]
#[doc = "      \"title\": \"Code\","]
#[doc = "      \"description\": \"Error code\","]
#[doc = "      \"default\": -32001,"]
#[doc = "      \"examples\": ["]
#[doc = "        -32001"]
#[doc = "      ],"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"const\": -32001"]
#[doc = "    },"]
#[doc = "    \"data\": {"]
#[doc = "      \"title\": \"Data\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"const\": null"]
#[doc = "    },"]
#[doc = "    \"message\": {"]
#[doc = "      \"title\": \"Message\","]
#[doc = "      \"description\": \"A short description of the error\","]
#[doc = "      \"default\": \"Task not found\","]
#[doc = "      \"examples\": ["]
#[doc = "        \"Task not found\""]
#[doc = "      ],"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"Task not found\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct TaskNotFoundError {
    #[doc = "Error code"]
    pub code: i64,
    pub data: ::serde_json::Value,
    #[doc = "A short description of the error"]
    pub message: ::std::string::String,
}
impl ::std::convert::From<&TaskNotFoundError> for TaskNotFoundError {
    fn from(value: &TaskNotFoundError) -> Self {
        value.clone()
    }
}
impl TaskNotFoundError {
    pub fn builder() -> builder::TaskNotFoundError {
        Default::default()
    }
}
#[doc = "`TaskPushNotificationConfig`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"TaskPushNotificationConfig\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"id\","]
#[doc = "    \"pushNotificationConfig\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"pushNotificationConfig\": {"]
#[doc = "      \"$ref\": \"#/$defs/PushNotificationConfig\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct TaskPushNotificationConfig {
    pub id: ::std::string::String,
    #[serde(rename = "pushNotificationConfig")]
    pub push_notification_config: PushNotificationConfig,
}
impl ::std::convert::From<&TaskPushNotificationConfig> for TaskPushNotificationConfig {
    fn from(value: &TaskPushNotificationConfig) -> Self {
        value.clone()
    }
}
impl TaskPushNotificationConfig {
    pub fn builder() -> builder::TaskPushNotificationConfig {
        Default::default()
    }
}
#[doc = "`TaskQueryParams`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"TaskQueryParams\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"id\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"historyLength\": {"]
#[doc = "      \"title\": \"HistoryLength\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"metadata\": {"]
#[doc = "      \"title\": \"Metadata\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
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
pub struct TaskQueryParams {
    #[serde(
        rename = "historyLength",
        default,
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub history_length: ::std::option::Option<i64>,
    pub id: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub metadata:
        ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
}
impl ::std::convert::From<&TaskQueryParams> for TaskQueryParams {
    fn from(value: &TaskQueryParams) -> Self {
        value.clone()
    }
}
impl TaskQueryParams {
    pub fn builder() -> builder::TaskQueryParams {
        Default::default()
    }
}
#[doc = "`TaskResubscriptionRequest`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"TaskResubscriptionRequest\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"method\","]
#[doc = "    \"params\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"jsonrpc\": {"]
#[doc = "      \"title\": \"Jsonrpc\","]
#[doc = "      \"default\": \"2.0\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"2.0\""]
#[doc = "    },"]
#[doc = "    \"method\": {"]
#[doc = "      \"title\": \"Method\","]
#[doc = "      \"default\": \"tasks/resubscribe\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"tasks/resubscribe\""]
#[doc = "    },"]
#[doc = "    \"params\": {"]
#[doc = "      \"$ref\": \"#/$defs/TaskQueryParams\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct TaskResubscriptionRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<Id>,
    #[serde(default = "defaults::task_resubscription_request_jsonrpc")]
    pub jsonrpc: ::std::string::String,
    pub method: ::std::string::String,
    pub params: TaskQueryParams,
}
impl ::std::convert::From<&TaskResubscriptionRequest> for TaskResubscriptionRequest {
    fn from(value: &TaskResubscriptionRequest) -> Self {
        value.clone()
    }
}
impl TaskResubscriptionRequest {
    pub fn builder() -> builder::TaskResubscriptionRequest {
        Default::default()
    }
}
#[doc = "`TaskSendParams`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"TaskSendParams\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"id\","]
#[doc = "    \"message\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"historyLength\": {"]
#[doc = "      \"title\": \"HistoryLength\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"integer\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"message\": {"]
#[doc = "      \"$ref\": \"#/$defs/Message\""]
#[doc = "    },"]
#[doc = "    \"metadata\": {"]
#[doc = "      \"title\": \"Metadata\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"pushNotification\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/PushNotificationConfig\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"sessionId\": {"]
#[doc = "      \"title\": \"Sessionid\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct TaskSendParams {
    #[serde(
        rename = "historyLength",
        default,
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub history_length: ::std::option::Option<i64>,
    pub id: ::std::string::String,
    pub message: Message,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub metadata:
        ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    #[serde(
        rename = "pushNotification",
        default,
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub push_notification: ::std::option::Option<PushNotificationConfig>,
    #[serde(
        rename = "sessionId",
        default,
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub session_id: ::std::option::Option<::std::string::String>,
}
impl ::std::convert::From<&TaskSendParams> for TaskSendParams {
    fn from(value: &TaskSendParams) -> Self {
        value.clone()
    }
}
impl TaskSendParams {
    pub fn builder() -> builder::TaskSendParams {
        Default::default()
    }
}
#[doc = "An enumeration."]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"TaskState\","]
#[doc = "  \"description\": \"An enumeration.\","]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"submitted\","]
#[doc = "    \"working\","]
#[doc = "    \"input-required\","]
#[doc = "    \"completed\","]
#[doc = "    \"canceled\","]
#[doc = "    \"failed\","]
#[doc = "    \"unknown\""]
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
pub enum TaskState {
    #[serde(rename = "submitted")]
    Submitted,
    #[serde(rename = "working")]
    Working,
    #[serde(rename = "input-required")]
    InputRequired,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "canceled")]
    Canceled,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "unknown")]
    Unknown,
}
impl ::std::convert::From<&Self> for TaskState {
    fn from(value: &TaskState) -> Self {
        value.clone()
    }
}
impl ::std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Submitted => write!(f, "submitted"),
            Self::Working => write!(f, "working"),
            Self::InputRequired => write!(f, "input-required"),
            Self::Completed => write!(f, "completed"),
            Self::Canceled => write!(f, "canceled"),
            Self::Failed => write!(f, "failed"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}
impl ::std::str::FromStr for TaskState {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "submitted" => Ok(Self::Submitted),
            "working" => Ok(Self::Working),
            "input-required" => Ok(Self::InputRequired),
            "completed" => Ok(Self::Completed),
            "canceled" => Ok(Self::Canceled),
            "failed" => Ok(Self::Failed),
            "unknown" => Ok(Self::Unknown),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TaskState {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for TaskState {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TaskState {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`TaskStatus`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"TaskStatus\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"state\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"message\": {"]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/Message\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"state\": {"]
#[doc = "      \"$ref\": \"#/$defs/TaskState\""]
#[doc = "    },"]
#[doc = "    \"timestamp\": {"]
#[doc = "      \"title\": \"Timestamp\","]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"format\": \"date-time\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct TaskStatus {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub message: ::std::option::Option<Message>,
    pub state: TaskState,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub timestamp: ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>,
}
impl ::std::convert::From<&TaskStatus> for TaskStatus {
    fn from(value: &TaskStatus) -> Self {
        value.clone()
    }
}
impl TaskStatus {
    pub fn builder() -> builder::TaskStatus {
        Default::default()
    }
}
#[doc = "`TaskStatusUpdateEvent`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"TaskStatusUpdateEvent\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"id\","]
#[doc = "    \"status\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"final\": {"]
#[doc = "      \"title\": \"Final\","]
#[doc = "      \"default\": false,"]
#[doc = "      \"type\": \"boolean\""]
#[doc = "    },"]
#[doc = "    \"id\": {"]
#[doc = "      \"title\": \"Id\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"metadata\": {"]
#[doc = "      \"title\": \"Metadata\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"status\": {"]
#[doc = "      \"$ref\": \"#/$defs/TaskStatus\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct TaskStatusUpdateEvent {
    #[serde(rename = "final", default)]
    pub final_: bool,
    pub id: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub metadata:
        ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    pub status: TaskStatus,
}
impl ::std::convert::From<&TaskStatusUpdateEvent> for TaskStatusUpdateEvent {
    fn from(value: &TaskStatusUpdateEvent) -> Self {
        value.clone()
    }
}
impl TaskStatusUpdateEvent {
    pub fn builder() -> builder::TaskStatusUpdateEvent {
        Default::default()
    }
}
#[doc = "`TextPart`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"TextPart\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"text\","]
#[doc = "    \"type\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"metadata\": {"]
#[doc = "      \"title\": \"Metadata\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"anyOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"object\","]
#[doc = "          \"additionalProperties\": {}"]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"text\": {"]
#[doc = "      \"title\": \"Text\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"type\": {"]
#[doc = "      \"title\": \"Type\","]
#[doc = "      \"description\": \"Type of the part\","]
#[doc = "      \"default\": \"text\","]
#[doc = "      \"examples\": ["]
#[doc = "        \"text\""]
#[doc = "      ],"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"text\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct TextPart {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub metadata:
        ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    pub text: ::std::string::String,
    #[doc = "Type of the part"]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
impl ::std::convert::From<&TextPart> for TextPart {
    fn from(value: &TextPart) -> Self {
        value.clone()
    }
}
impl TextPart {
    pub fn builder() -> builder::TextPart {
        Default::default()
    }
}
#[doc = "`UnsupportedOperationError`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"UnsupportedOperationError\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"code\","]
#[doc = "    \"data\","]
#[doc = "    \"message\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"code\": {"]
#[doc = "      \"title\": \"Code\","]
#[doc = "      \"description\": \"Error code\","]
#[doc = "      \"default\": -32004,"]
#[doc = "      \"examples\": ["]
#[doc = "        -32004"]
#[doc = "      ],"]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"const\": -32004"]
#[doc = "    },"]
#[doc = "    \"data\": {"]
#[doc = "      \"title\": \"Data\","]
#[doc = "      \"default\": null,"]
#[doc = "      \"const\": null"]
#[doc = "    },"]
#[doc = "    \"message\": {"]
#[doc = "      \"title\": \"Message\","]
#[doc = "      \"description\": \"A short description of the error\","]
#[doc = "      \"default\": \"This operation is not supported\","]
#[doc = "      \"examples\": ["]
#[doc = "        \"This operation is not supported\""]
#[doc = "      ],"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"const\": \"This operation is not supported\""]
#[doc = "    }"]
#[doc = "  }"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
pub struct UnsupportedOperationError {
    #[doc = "Error code"]
    pub code: i64,
    pub data: ::serde_json::Value,
    #[doc = "A short description of the error"]
    pub message: ::std::string::String,
}
impl ::std::convert::From<&UnsupportedOperationError> for UnsupportedOperationError {
    fn from(value: &UnsupportedOperationError) -> Self {
        value.clone()
    }
}
impl UnsupportedOperationError {
    pub fn builder() -> builder::UnsupportedOperationError {
        Default::default()
    }
}
#[doc = r" Types for composing complex structures."]
pub mod builder {
    #[derive(Clone, Debug)]
    pub struct AgentAuthentication {
        credentials: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        schemes:
            ::std::result::Result<::std::vec::Vec<::std::string::String>, ::std::string::String>,
    }
    impl ::std::default::Default for AgentAuthentication {
        fn default() -> Self {
            Self {
                credentials: Ok(Default::default()),
                schemes: Err("no value supplied for schemes".to_string()),
            }
        }
    }
    impl AgentAuthentication {
        pub fn credentials<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.credentials = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for credentials: {}", e));
            self
        }
        pub fn schemes<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.schemes = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for schemes: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<AgentAuthentication> for super::AgentAuthentication {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AgentAuthentication,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                credentials: value.credentials?,
                schemes: value.schemes?,
            })
        }
    }
    impl ::std::convert::From<super::AgentAuthentication> for AgentAuthentication {
        fn from(value: super::AgentAuthentication) -> Self {
            Self {
                credentials: Ok(value.credentials),
                schemes: Ok(value.schemes),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AgentCapabilities {
        push_notifications: ::std::result::Result<bool, ::std::string::String>,
        state_transition_history: ::std::result::Result<bool, ::std::string::String>,
        streaming: ::std::result::Result<bool, ::std::string::String>,
    }
    impl ::std::default::Default for AgentCapabilities {
        fn default() -> Self {
            Self {
                push_notifications: Ok(Default::default()),
                state_transition_history: Ok(Default::default()),
                streaming: Ok(Default::default()),
            }
        }
    }
    impl AgentCapabilities {
        pub fn push_notifications<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.push_notifications = value.try_into().map_err(|e| {
                format!(
                    "error converting supplied value for push_notifications: {}",
                    e
                )
            });
            self
        }
        pub fn state_transition_history<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.state_transition_history = value.try_into().map_err(|e| {
                format!(
                    "error converting supplied value for state_transition_history: {}",
                    e
                )
            });
            self
        }
        pub fn streaming<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.streaming = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for streaming: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<AgentCapabilities> for super::AgentCapabilities {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AgentCapabilities,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                push_notifications: value.push_notifications?,
                state_transition_history: value.state_transition_history?,
                streaming: value.streaming?,
            })
        }
    }
    impl ::std::convert::From<super::AgentCapabilities> for AgentCapabilities {
        fn from(value: super::AgentCapabilities) -> Self {
            Self {
                push_notifications: Ok(value.push_notifications),
                state_transition_history: Ok(value.state_transition_history),
                streaming: Ok(value.streaming),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AgentCard {
        authentication: ::std::result::Result<
            ::std::option::Option<super::AgentAuthentication>,
            ::std::string::String,
        >,
        capabilities: ::std::result::Result<super::AgentCapabilities, ::std::string::String>,
        default_input_modes:
            ::std::result::Result<::std::vec::Vec<::std::string::String>, ::std::string::String>,
        default_output_modes:
            ::std::result::Result<::std::vec::Vec<::std::string::String>, ::std::string::String>,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        documentation_url: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        provider: ::std::result::Result<
            ::std::option::Option<super::AgentProvider>,
            ::std::string::String,
        >,
        skills: ::std::result::Result<::std::vec::Vec<super::AgentSkill>, ::std::string::String>,
        url: ::std::result::Result<::std::string::String, ::std::string::String>,
        version: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for AgentCard {
        fn default() -> Self {
            Self {
                authentication: Ok(Default::default()),
                capabilities: Err("no value supplied for capabilities".to_string()),
                default_input_modes: Ok(super::defaults::agent_card_default_input_modes()),
                default_output_modes: Ok(super::defaults::agent_card_default_output_modes()),
                description: Ok(Default::default()),
                documentation_url: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                provider: Ok(Default::default()),
                skills: Err("no value supplied for skills".to_string()),
                url: Err("no value supplied for url".to_string()),
                version: Err("no value supplied for version".to_string()),
            }
        }
    }
    impl AgentCard {
        pub fn authentication<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::AgentAuthentication>>,
            T::Error: ::std::fmt::Display,
        {
            self.authentication = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for authentication: {}", e));
            self
        }
        pub fn capabilities<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::AgentCapabilities>,
            T::Error: ::std::fmt::Display,
        {
            self.capabilities = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for capabilities: {}", e));
            self
        }
        pub fn default_input_modes<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.default_input_modes = value.try_into().map_err(|e| {
                format!(
                    "error converting supplied value for default_input_modes: {}",
                    e
                )
            });
            self
        }
        pub fn default_output_modes<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.default_output_modes = value.try_into().map_err(|e| {
                format!(
                    "error converting supplied value for default_output_modes: {}",
                    e
                )
            });
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {}", e));
            self
        }
        pub fn documentation_url<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.documentation_url = value.try_into().map_err(|e| {
                format!(
                    "error converting supplied value for documentation_url: {}",
                    e
                )
            });
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {}", e));
            self
        }
        pub fn provider<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::AgentProvider>>,
            T::Error: ::std::fmt::Display,
        {
            self.provider = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for provider: {}", e));
            self
        }
        pub fn skills<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::AgentSkill>>,
            T::Error: ::std::fmt::Display,
        {
            self.skills = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for skills: {}", e));
            self
        }
        pub fn url<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.url = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for url: {}", e));
            self
        }
        pub fn version<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.version = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for version: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<AgentCard> for super::AgentCard {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AgentCard,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                authentication: value.authentication?,
                capabilities: value.capabilities?,
                default_input_modes: value.default_input_modes?,
                default_output_modes: value.default_output_modes?,
                description: value.description?,
                documentation_url: value.documentation_url?,
                name: value.name?,
                provider: value.provider?,
                skills: value.skills?,
                url: value.url?,
                version: value.version?,
            })
        }
    }
    impl ::std::convert::From<super::AgentCard> for AgentCard {
        fn from(value: super::AgentCard) -> Self {
            Self {
                authentication: Ok(value.authentication),
                capabilities: Ok(value.capabilities),
                default_input_modes: Ok(value.default_input_modes),
                default_output_modes: Ok(value.default_output_modes),
                description: Ok(value.description),
                documentation_url: Ok(value.documentation_url),
                name: Ok(value.name),
                provider: Ok(value.provider),
                skills: Ok(value.skills),
                url: Ok(value.url),
                version: Ok(value.version),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AgentProvider {
        organization: ::std::result::Result<::std::string::String, ::std::string::String>,
        url: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for AgentProvider {
        fn default() -> Self {
            Self {
                organization: Err("no value supplied for organization".to_string()),
                url: Ok(Default::default()),
            }
        }
    }
    impl AgentProvider {
        pub fn organization<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.organization = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for organization: {}", e));
            self
        }
        pub fn url<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.url = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for url: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<AgentProvider> for super::AgentProvider {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AgentProvider,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                organization: value.organization?,
                url: value.url?,
            })
        }
    }
    impl ::std::convert::From<super::AgentProvider> for AgentProvider {
        fn from(value: super::AgentProvider) -> Self {
            Self {
                organization: Ok(value.organization),
                url: Ok(value.url),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AgentSkill {
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        examples: ::std::result::Result<
            ::std::option::Option<::std::vec::Vec<::std::string::String>>,
            ::std::string::String,
        >,
        id: ::std::result::Result<::std::string::String, ::std::string::String>,
        input_modes: ::std::result::Result<
            ::std::option::Option<::std::vec::Vec<::std::string::String>>,
            ::std::string::String,
        >,
        name: ::std::result::Result<::std::string::String, ::std::string::String>,
        output_modes: ::std::result::Result<
            ::std::option::Option<::std::vec::Vec<::std::string::String>>,
            ::std::string::String,
        >,
        tags: ::std::result::Result<
            ::std::option::Option<::std::vec::Vec<::std::string::String>>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for AgentSkill {
        fn default() -> Self {
            Self {
                description: Ok(Default::default()),
                examples: Ok(Default::default()),
                id: Err("no value supplied for id".to_string()),
                input_modes: Ok(Default::default()),
                name: Err("no value supplied for name".to_string()),
                output_modes: Ok(Default::default()),
                tags: Ok(Default::default()),
            }
        }
    }
    impl AgentSkill {
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {}", e));
            self
        }
        pub fn examples<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<::std::vec::Vec<::std::string::String>>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.examples = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for examples: {}", e));
            self
        }
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn input_modes<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<::std::vec::Vec<::std::string::String>>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.input_modes = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for input_modes: {}", e));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {}", e));
            self
        }
        pub fn output_modes<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<::std::vec::Vec<::std::string::String>>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.output_modes = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for output_modes: {}", e));
            self
        }
        pub fn tags<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<::std::vec::Vec<::std::string::String>>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.tags = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for tags: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<AgentSkill> for super::AgentSkill {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AgentSkill,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                description: value.description?,
                examples: value.examples?,
                id: value.id?,
                input_modes: value.input_modes?,
                name: value.name?,
                output_modes: value.output_modes?,
                tags: value.tags?,
            })
        }
    }
    impl ::std::convert::From<super::AgentSkill> for AgentSkill {
        fn from(value: super::AgentSkill) -> Self {
            Self {
                description: Ok(value.description),
                examples: Ok(value.examples),
                id: Ok(value.id),
                input_modes: Ok(value.input_modes),
                name: Ok(value.name),
                output_modes: Ok(value.output_modes),
                tags: Ok(value.tags),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Artifact {
        append: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        description: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        index: ::std::result::Result<i64, ::std::string::String>,
        last_chunk: ::std::result::Result<::std::option::Option<bool>, ::std::string::String>,
        metadata: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
        name: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        parts: ::std::result::Result<::std::vec::Vec<super::Part>, ::std::string::String>,
    }
    impl ::std::default::Default for Artifact {
        fn default() -> Self {
            Self {
                append: Ok(Default::default()),
                description: Ok(Default::default()),
                index: Ok(Default::default()),
                last_chunk: Ok(Default::default()),
                metadata: Ok(Default::default()),
                name: Ok(Default::default()),
                parts: Err("no value supplied for parts".to_string()),
            }
        }
    }
    impl Artifact {
        pub fn append<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.append = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for append: {}", e));
            self
        }
        pub fn description<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.description = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for description: {}", e));
            self
        }
        pub fn index<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.index = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for index: {}", e));
            self
        }
        pub fn last_chunk<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<bool>>,
            T::Error: ::std::fmt::Display,
        {
            self.last_chunk = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for last_chunk: {}", e));
            self
        }
        pub fn metadata<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.metadata = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for metadata: {}", e));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {}", e));
            self
        }
        pub fn parts<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Part>>,
            T::Error: ::std::fmt::Display,
        {
            self.parts = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for parts: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<Artifact> for super::Artifact {
        type Error = super::error::ConversionError;
        fn try_from(value: Artifact) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                append: value.append?,
                description: value.description?,
                index: value.index?,
                last_chunk: value.last_chunk?,
                metadata: value.metadata?,
                name: value.name?,
                parts: value.parts?,
            })
        }
    }
    impl ::std::convert::From<super::Artifact> for Artifact {
        fn from(value: super::Artifact) -> Self {
            Self {
                append: Ok(value.append),
                description: Ok(value.description),
                index: Ok(value.index),
                last_chunk: Ok(value.last_chunk),
                metadata: Ok(value.metadata),
                name: Ok(value.name),
                parts: Ok(value.parts),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct AuthenticationInfo {
        credentials: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        schemes:
            ::std::result::Result<::std::vec::Vec<::std::string::String>, ::std::string::String>,
        extra: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for AuthenticationInfo {
        fn default() -> Self {
            Self {
                credentials: Ok(Default::default()),
                schemes: Err("no value supplied for schemes".to_string()),
                extra: Err("no value supplied for extra".to_string()),
            }
        }
    }
    impl AuthenticationInfo {
        pub fn credentials<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.credentials = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for credentials: {}", e));
            self
        }
        pub fn schemes<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.schemes = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for schemes: {}", e));
            self
        }
        pub fn extra<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.extra = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for extra: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<AuthenticationInfo> for super::AuthenticationInfo {
        type Error = super::error::ConversionError;
        fn try_from(
            value: AuthenticationInfo,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                credentials: value.credentials?,
                schemes: value.schemes?,
                extra: value.extra?,
            })
        }
    }
    impl ::std::convert::From<super::AuthenticationInfo> for AuthenticationInfo {
        fn from(value: super::AuthenticationInfo) -> Self {
            Self {
                credentials: Ok(value.credentials),
                schemes: Ok(value.schemes),
                extra: Ok(value.extra),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct CancelTaskRequest {
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        method: ::std::result::Result<::std::string::String, ::std::string::String>,
        params: ::std::result::Result<super::TaskIdParams, ::std::string::String>,
    }
    impl ::std::default::Default for CancelTaskRequest {
        fn default() -> Self {
            Self {
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::cancel_task_request_jsonrpc()),
                method: Err("no value supplied for method".to_string()),
                params: Err("no value supplied for params".to_string()),
            }
        }
    }
    impl CancelTaskRequest {
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn method<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.method = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for method: {}", e));
            self
        }
        pub fn params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::TaskIdParams>,
            T::Error: ::std::fmt::Display,
        {
            self.params = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for params: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<CancelTaskRequest> for super::CancelTaskRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: CancelTaskRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                method: value.method?,
                params: value.params?,
            })
        }
    }
    impl ::std::convert::From<super::CancelTaskRequest> for CancelTaskRequest {
        fn from(value: super::CancelTaskRequest) -> Self {
            Self {
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                method: Ok(value.method),
                params: Ok(value.params),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct CancelTaskResponse {
        error: ::std::result::Result<
            ::std::option::Option<super::JsonrpcError>,
            ::std::string::String,
        >,
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        result: ::std::result::Result<::std::option::Option<super::Task>, ::std::string::String>,
    }
    impl ::std::default::Default for CancelTaskResponse {
        fn default() -> Self {
            Self {
                error: Ok(Default::default()),
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::cancel_task_response_jsonrpc()),
                result: Ok(Default::default()),
            }
        }
    }
    impl CancelTaskResponse {
        pub fn error<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::JsonrpcError>>,
            T::Error: ::std::fmt::Display,
        {
            self.error = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for error: {}", e));
            self
        }
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn result<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Task>>,
            T::Error: ::std::fmt::Display,
        {
            self.result = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for result: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<CancelTaskResponse> for super::CancelTaskResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: CancelTaskResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                error: value.error?,
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                result: value.result?,
            })
        }
    }
    impl ::std::convert::From<super::CancelTaskResponse> for CancelTaskResponse {
        fn from(value: super::CancelTaskResponse) -> Self {
            Self {
                error: Ok(value.error),
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                result: Ok(value.result),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct DataPart {
        data: ::std::result::Result<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            ::std::string::String,
        >,
        metadata: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
        type_: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for DataPart {
        fn default() -> Self {
            Self {
                data: Err("no value supplied for data".to_string()),
                metadata: Ok(Default::default()),
                type_: Err("no value supplied for type_".to_string()),
            }
        }
    }
    impl DataPart {
        pub fn data<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::serde_json::Map<::std::string::String, ::serde_json::Value>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.data = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data: {}", e));
            self
        }
        pub fn metadata<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.metadata = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for metadata: {}", e));
            self
        }
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<DataPart> for super::DataPart {
        type Error = super::error::ConversionError;
        fn try_from(value: DataPart) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                data: value.data?,
                metadata: value.metadata?,
                type_: value.type_?,
            })
        }
    }
    impl ::std::convert::From<super::DataPart> for DataPart {
        fn from(value: super::DataPart) -> Self {
            Self {
                data: Ok(value.data),
                metadata: Ok(value.metadata),
                type_: Ok(value.type_),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct FileContent {
        bytes: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        mime_type: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        name: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        uri: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for FileContent {
        fn default() -> Self {
            Self {
                bytes: Ok(Default::default()),
                mime_type: Ok(Default::default()),
                name: Ok(Default::default()),
                uri: Ok(Default::default()),
            }
        }
    }
    impl FileContent {
        pub fn bytes<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.bytes = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for bytes: {}", e));
            self
        }
        pub fn mime_type<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.mime_type = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for mime_type: {}", e));
            self
        }
        pub fn name<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.name = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for name: {}", e));
            self
        }
        pub fn uri<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.uri = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for uri: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<FileContent> for super::FileContent {
        type Error = super::error::ConversionError;
        fn try_from(
            value: FileContent,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                bytes: value.bytes?,
                mime_type: value.mime_type?,
                name: value.name?,
                uri: value.uri?,
            })
        }
    }
    impl ::std::convert::From<super::FileContent> for FileContent {
        fn from(value: super::FileContent) -> Self {
            Self {
                bytes: Ok(value.bytes),
                mime_type: Ok(value.mime_type),
                name: Ok(value.name),
                uri: Ok(value.uri),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct FilePart {
        file: ::std::result::Result<super::FileContent, ::std::string::String>,
        metadata: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
        type_: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for FilePart {
        fn default() -> Self {
            Self {
                file: Err("no value supplied for file".to_string()),
                metadata: Ok(Default::default()),
                type_: Err("no value supplied for type_".to_string()),
            }
        }
    }
    impl FilePart {
        pub fn file<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::FileContent>,
            T::Error: ::std::fmt::Display,
        {
            self.file = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for file: {}", e));
            self
        }
        pub fn metadata<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.metadata = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for metadata: {}", e));
            self
        }
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<FilePart> for super::FilePart {
        type Error = super::error::ConversionError;
        fn try_from(value: FilePart) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                file: value.file?,
                metadata: value.metadata?,
                type_: value.type_?,
            })
        }
    }
    impl ::std::convert::From<super::FilePart> for FilePart {
        fn from(value: super::FilePart) -> Self {
            Self {
                file: Ok(value.file),
                metadata: Ok(value.metadata),
                type_: Ok(value.type_),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct GetTaskPushNotificationRequest {
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        method: ::std::result::Result<::std::string::String, ::std::string::String>,
        params: ::std::result::Result<super::TaskIdParams, ::std::string::String>,
    }
    impl ::std::default::Default for GetTaskPushNotificationRequest {
        fn default() -> Self {
            Self {
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::get_task_push_notification_request_jsonrpc()),
                method: Err("no value supplied for method".to_string()),
                params: Err("no value supplied for params".to_string()),
            }
        }
    }
    impl GetTaskPushNotificationRequest {
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn method<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.method = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for method: {}", e));
            self
        }
        pub fn params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::TaskIdParams>,
            T::Error: ::std::fmt::Display,
        {
            self.params = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for params: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<GetTaskPushNotificationRequest>
        for super::GetTaskPushNotificationRequest
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GetTaskPushNotificationRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                method: value.method?,
                params: value.params?,
            })
        }
    }
    impl ::std::convert::From<super::GetTaskPushNotificationRequest>
        for GetTaskPushNotificationRequest
    {
        fn from(value: super::GetTaskPushNotificationRequest) -> Self {
            Self {
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                method: Ok(value.method),
                params: Ok(value.params),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct GetTaskPushNotificationResponse {
        error: ::std::result::Result<
            ::std::option::Option<super::JsonrpcError>,
            ::std::string::String,
        >,
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        result: ::std::result::Result<
            ::std::option::Option<super::TaskPushNotificationConfig>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for GetTaskPushNotificationResponse {
        fn default() -> Self {
            Self {
                error: Ok(Default::default()),
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::get_task_push_notification_response_jsonrpc()),
                result: Ok(Default::default()),
            }
        }
    }
    impl GetTaskPushNotificationResponse {
        pub fn error<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::JsonrpcError>>,
            T::Error: ::std::fmt::Display,
        {
            self.error = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for error: {}", e));
            self
        }
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn result<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::TaskPushNotificationConfig>>,
            T::Error: ::std::fmt::Display,
        {
            self.result = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for result: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<GetTaskPushNotificationResponse>
        for super::GetTaskPushNotificationResponse
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GetTaskPushNotificationResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                error: value.error?,
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                result: value.result?,
            })
        }
    }
    impl ::std::convert::From<super::GetTaskPushNotificationResponse>
        for GetTaskPushNotificationResponse
    {
        fn from(value: super::GetTaskPushNotificationResponse) -> Self {
            Self {
                error: Ok(value.error),
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                result: Ok(value.result),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct GetTaskRequest {
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        method: ::std::result::Result<::std::string::String, ::std::string::String>,
        params: ::std::result::Result<super::TaskQueryParams, ::std::string::String>,
    }
    impl ::std::default::Default for GetTaskRequest {
        fn default() -> Self {
            Self {
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::get_task_request_jsonrpc()),
                method: Err("no value supplied for method".to_string()),
                params: Err("no value supplied for params".to_string()),
            }
        }
    }
    impl GetTaskRequest {
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn method<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.method = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for method: {}", e));
            self
        }
        pub fn params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::TaskQueryParams>,
            T::Error: ::std::fmt::Display,
        {
            self.params = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for params: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<GetTaskRequest> for super::GetTaskRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GetTaskRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                method: value.method?,
                params: value.params?,
            })
        }
    }
    impl ::std::convert::From<super::GetTaskRequest> for GetTaskRequest {
        fn from(value: super::GetTaskRequest) -> Self {
            Self {
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                method: Ok(value.method),
                params: Ok(value.params),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct GetTaskResponse {
        error: ::std::result::Result<
            ::std::option::Option<super::JsonrpcError>,
            ::std::string::String,
        >,
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        result: ::std::result::Result<::std::option::Option<super::Task>, ::std::string::String>,
    }
    impl ::std::default::Default for GetTaskResponse {
        fn default() -> Self {
            Self {
                error: Ok(Default::default()),
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::get_task_response_jsonrpc()),
                result: Ok(Default::default()),
            }
        }
    }
    impl GetTaskResponse {
        pub fn error<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::JsonrpcError>>,
            T::Error: ::std::fmt::Display,
        {
            self.error = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for error: {}", e));
            self
        }
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn result<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Task>>,
            T::Error: ::std::fmt::Display,
        {
            self.result = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for result: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<GetTaskResponse> for super::GetTaskResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GetTaskResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                error: value.error?,
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                result: value.result?,
            })
        }
    }
    impl ::std::convert::From<super::GetTaskResponse> for GetTaskResponse {
        fn from(value: super::GetTaskResponse) -> Self {
            Self {
                error: Ok(value.error),
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                result: Ok(value.result),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct InternalError {
        code: ::std::result::Result<i64, ::std::string::String>,
        data: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
        message: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for InternalError {
        fn default() -> Self {
            Self {
                code: Err("no value supplied for code".to_string()),
                data: Ok(Default::default()),
                message: Err("no value supplied for message".to_string()),
            }
        }
    }
    impl InternalError {
        pub fn code<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.code = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for code: {}", e));
            self
        }
        pub fn data<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.data = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data: {}", e));
            self
        }
        pub fn message<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.message = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for message: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<InternalError> for super::InternalError {
        type Error = super::error::ConversionError;
        fn try_from(
            value: InternalError,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                code: value.code?,
                data: value.data?,
                message: value.message?,
            })
        }
    }
    impl ::std::convert::From<super::InternalError> for InternalError {
        fn from(value: super::InternalError) -> Self {
            Self {
                code: Ok(value.code),
                data: Ok(value.data),
                message: Ok(value.message),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct InvalidParamsError {
        code: ::std::result::Result<i64, ::std::string::String>,
        data: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
        message: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for InvalidParamsError {
        fn default() -> Self {
            Self {
                code: Err("no value supplied for code".to_string()),
                data: Ok(Default::default()),
                message: Err("no value supplied for message".to_string()),
            }
        }
    }
    impl InvalidParamsError {
        pub fn code<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.code = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for code: {}", e));
            self
        }
        pub fn data<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.data = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data: {}", e));
            self
        }
        pub fn message<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.message = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for message: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<InvalidParamsError> for super::InvalidParamsError {
        type Error = super::error::ConversionError;
        fn try_from(
            value: InvalidParamsError,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                code: value.code?,
                data: value.data?,
                message: value.message?,
            })
        }
    }
    impl ::std::convert::From<super::InvalidParamsError> for InvalidParamsError {
        fn from(value: super::InvalidParamsError) -> Self {
            Self {
                code: Ok(value.code),
                data: Ok(value.data),
                message: Ok(value.message),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct InvalidRequestError {
        code: ::std::result::Result<i64, ::std::string::String>,
        data: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
        message: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for InvalidRequestError {
        fn default() -> Self {
            Self {
                code: Err("no value supplied for code".to_string()),
                data: Ok(Default::default()),
                message: Err("no value supplied for message".to_string()),
            }
        }
    }
    impl InvalidRequestError {
        pub fn code<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.code = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for code: {}", e));
            self
        }
        pub fn data<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.data = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data: {}", e));
            self
        }
        pub fn message<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.message = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for message: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<InvalidRequestError> for super::InvalidRequestError {
        type Error = super::error::ConversionError;
        fn try_from(
            value: InvalidRequestError,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                code: value.code?,
                data: value.data?,
                message: value.message?,
            })
        }
    }
    impl ::std::convert::From<super::InvalidRequestError> for InvalidRequestError {
        fn from(value: super::InvalidRequestError) -> Self {
            Self {
                code: Ok(value.code),
                data: Ok(value.data),
                message: Ok(value.message),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct JsonParseError {
        code: ::std::result::Result<i64, ::std::string::String>,
        data: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
        message: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for JsonParseError {
        fn default() -> Self {
            Self {
                code: Err("no value supplied for code".to_string()),
                data: Ok(Default::default()),
                message: Err("no value supplied for message".to_string()),
            }
        }
    }
    impl JsonParseError {
        pub fn code<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.code = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for code: {}", e));
            self
        }
        pub fn data<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.data = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data: {}", e));
            self
        }
        pub fn message<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.message = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for message: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<JsonParseError> for super::JsonParseError {
        type Error = super::error::ConversionError;
        fn try_from(
            value: JsonParseError,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                code: value.code?,
                data: value.data?,
                message: value.message?,
            })
        }
    }
    impl ::std::convert::From<super::JsonParseError> for JsonParseError {
        fn from(value: super::JsonParseError) -> Self {
            Self {
                code: Ok(value.code),
                data: Ok(value.data),
                message: Ok(value.message),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct JsonrpcError {
        code: ::std::result::Result<i64, ::std::string::String>,
        data: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
        message: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for JsonrpcError {
        fn default() -> Self {
            Self {
                code: Err("no value supplied for code".to_string()),
                data: Ok(Default::default()),
                message: Err("no value supplied for message".to_string()),
            }
        }
    }
    impl JsonrpcError {
        pub fn code<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.code = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for code: {}", e));
            self
        }
        pub fn data<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.data = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data: {}", e));
            self
        }
        pub fn message<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.message = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for message: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<JsonrpcError> for super::JsonrpcError {
        type Error = super::error::ConversionError;
        fn try_from(
            value: JsonrpcError,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                code: value.code?,
                data: value.data?,
                message: value.message?,
            })
        }
    }
    impl ::std::convert::From<super::JsonrpcError> for JsonrpcError {
        fn from(value: super::JsonrpcError) -> Self {
            Self {
                code: Ok(value.code),
                data: Ok(value.data),
                message: Ok(value.message),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct JsonrpcMessage {
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for JsonrpcMessage {
        fn default() -> Self {
            Self {
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::jsonrpc_message_jsonrpc()),
            }
        }
    }
    impl JsonrpcMessage {
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<JsonrpcMessage> for super::JsonrpcMessage {
        type Error = super::error::ConversionError;
        fn try_from(
            value: JsonrpcMessage,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                id: value.id?,
                jsonrpc: value.jsonrpc?,
            })
        }
    }
    impl ::std::convert::From<super::JsonrpcMessage> for JsonrpcMessage {
        fn from(value: super::JsonrpcMessage) -> Self {
            Self {
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct JsonrpcRequest {
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        method: ::std::result::Result<::std::string::String, ::std::string::String>,
        params: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for JsonrpcRequest {
        fn default() -> Self {
            Self {
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::jsonrpc_request_jsonrpc()),
                method: Err("no value supplied for method".to_string()),
                params: Ok(Default::default()),
            }
        }
    }
    impl JsonrpcRequest {
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn method<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.method = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for method: {}", e));
            self
        }
        pub fn params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.params = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for params: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<JsonrpcRequest> for super::JsonrpcRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: JsonrpcRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                method: value.method?,
                params: value.params?,
            })
        }
    }
    impl ::std::convert::From<super::JsonrpcRequest> for JsonrpcRequest {
        fn from(value: super::JsonrpcRequest) -> Self {
            Self {
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                method: Ok(value.method),
                params: Ok(value.params),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct JsonrpcResponse {
        error: ::std::result::Result<
            ::std::option::Option<super::JsonrpcError>,
            ::std::string::String,
        >,
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        result: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for JsonrpcResponse {
        fn default() -> Self {
            Self {
                error: Ok(Default::default()),
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::jsonrpc_response_jsonrpc()),
                result: Ok(Default::default()),
            }
        }
    }
    impl JsonrpcResponse {
        pub fn error<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::JsonrpcError>>,
            T::Error: ::std::fmt::Display,
        {
            self.error = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for error: {}", e));
            self
        }
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn result<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.result = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for result: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<JsonrpcResponse> for super::JsonrpcResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: JsonrpcResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                error: value.error?,
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                result: value.result?,
            })
        }
    }
    impl ::std::convert::From<super::JsonrpcResponse> for JsonrpcResponse {
        fn from(value: super::JsonrpcResponse) -> Self {
            Self {
                error: Ok(value.error),
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                result: Ok(value.result),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Message {
        metadata: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
        parts: ::std::result::Result<::std::vec::Vec<super::Part>, ::std::string::String>,
        role: ::std::result::Result<super::Role, ::std::string::String>,
    }
    impl ::std::default::Default for Message {
        fn default() -> Self {
            Self {
                metadata: Ok(Default::default()),
                parts: Err("no value supplied for parts".to_string()),
                role: Err("no value supplied for role".to_string()),
            }
        }
    }
    impl Message {
        pub fn metadata<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.metadata = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for metadata: {}", e));
            self
        }
        pub fn parts<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::vec::Vec<super::Part>>,
            T::Error: ::std::fmt::Display,
        {
            self.parts = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for parts: {}", e));
            self
        }
        pub fn role<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::Role>,
            T::Error: ::std::fmt::Display,
        {
            self.role = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for role: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<Message> for super::Message {
        type Error = super::error::ConversionError;
        fn try_from(value: Message) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                metadata: value.metadata?,
                parts: value.parts?,
                role: value.role?,
            })
        }
    }
    impl ::std::convert::From<super::Message> for Message {
        fn from(value: super::Message) -> Self {
            Self {
                metadata: Ok(value.metadata),
                parts: Ok(value.parts),
                role: Ok(value.role),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct MethodNotFoundError {
        code: ::std::result::Result<i64, ::std::string::String>,
        data: ::std::result::Result<::serde_json::Value, ::std::string::String>,
        message: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for MethodNotFoundError {
        fn default() -> Self {
            Self {
                code: Err("no value supplied for code".to_string()),
                data: Err("no value supplied for data".to_string()),
                message: Err("no value supplied for message".to_string()),
            }
        }
    }
    impl MethodNotFoundError {
        pub fn code<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.code = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for code: {}", e));
            self
        }
        pub fn data<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::serde_json::Value>,
            T::Error: ::std::fmt::Display,
        {
            self.data = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data: {}", e));
            self
        }
        pub fn message<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.message = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for message: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<MethodNotFoundError> for super::MethodNotFoundError {
        type Error = super::error::ConversionError;
        fn try_from(
            value: MethodNotFoundError,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                code: value.code?,
                data: value.data?,
                message: value.message?,
            })
        }
    }
    impl ::std::convert::From<super::MethodNotFoundError> for MethodNotFoundError {
        fn from(value: super::MethodNotFoundError) -> Self {
            Self {
                code: Ok(value.code),
                data: Ok(value.data),
                message: Ok(value.message),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct PushNotificationConfig {
        authentication: ::std::result::Result<
            ::std::option::Option<super::AuthenticationInfo>,
            ::std::string::String,
        >,
        token: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        url: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for PushNotificationConfig {
        fn default() -> Self {
            Self {
                authentication: Ok(Default::default()),
                token: Ok(Default::default()),
                url: Err("no value supplied for url".to_string()),
            }
        }
    }
    impl PushNotificationConfig {
        pub fn authentication<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::AuthenticationInfo>>,
            T::Error: ::std::fmt::Display,
        {
            self.authentication = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for authentication: {}", e));
            self
        }
        pub fn token<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.token = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for token: {}", e));
            self
        }
        pub fn url<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.url = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for url: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<PushNotificationConfig> for super::PushNotificationConfig {
        type Error = super::error::ConversionError;
        fn try_from(
            value: PushNotificationConfig,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                authentication: value.authentication?,
                token: value.token?,
                url: value.url?,
            })
        }
    }
    impl ::std::convert::From<super::PushNotificationConfig> for PushNotificationConfig {
        fn from(value: super::PushNotificationConfig) -> Self {
            Self {
                authentication: Ok(value.authentication),
                token: Ok(value.token),
                url: Ok(value.url),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct PushNotificationNotSupportedError {
        code: ::std::result::Result<i64, ::std::string::String>,
        data: ::std::result::Result<::serde_json::Value, ::std::string::String>,
        message: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for PushNotificationNotSupportedError {
        fn default() -> Self {
            Self {
                code: Err("no value supplied for code".to_string()),
                data: Err("no value supplied for data".to_string()),
                message: Err("no value supplied for message".to_string()),
            }
        }
    }
    impl PushNotificationNotSupportedError {
        pub fn code<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.code = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for code: {}", e));
            self
        }
        pub fn data<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::serde_json::Value>,
            T::Error: ::std::fmt::Display,
        {
            self.data = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data: {}", e));
            self
        }
        pub fn message<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.message = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for message: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<PushNotificationNotSupportedError>
        for super::PushNotificationNotSupportedError
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: PushNotificationNotSupportedError,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                code: value.code?,
                data: value.data?,
                message: value.message?,
            })
        }
    }
    impl ::std::convert::From<super::PushNotificationNotSupportedError>
        for PushNotificationNotSupportedError
    {
        fn from(value: super::PushNotificationNotSupportedError) -> Self {
            Self {
                code: Ok(value.code),
                data: Ok(value.data),
                message: Ok(value.message),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SendTaskRequest {
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        method: ::std::result::Result<::std::string::String, ::std::string::String>,
        params: ::std::result::Result<super::TaskSendParams, ::std::string::String>,
    }
    impl ::std::default::Default for SendTaskRequest {
        fn default() -> Self {
            Self {
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::send_task_request_jsonrpc()),
                method: Err("no value supplied for method".to_string()),
                params: Err("no value supplied for params".to_string()),
            }
        }
    }
    impl SendTaskRequest {
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn method<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.method = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for method: {}", e));
            self
        }
        pub fn params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::TaskSendParams>,
            T::Error: ::std::fmt::Display,
        {
            self.params = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for params: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<SendTaskRequest> for super::SendTaskRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SendTaskRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                method: value.method?,
                params: value.params?,
            })
        }
    }
    impl ::std::convert::From<super::SendTaskRequest> for SendTaskRequest {
        fn from(value: super::SendTaskRequest) -> Self {
            Self {
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                method: Ok(value.method),
                params: Ok(value.params),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SendTaskResponse {
        error: ::std::result::Result<
            ::std::option::Option<super::JsonrpcError>,
            ::std::string::String,
        >,
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        result: ::std::result::Result<::std::option::Option<super::Task>, ::std::string::String>,
    }
    impl ::std::default::Default for SendTaskResponse {
        fn default() -> Self {
            Self {
                error: Ok(Default::default()),
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::send_task_response_jsonrpc()),
                result: Ok(Default::default()),
            }
        }
    }
    impl SendTaskResponse {
        pub fn error<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::JsonrpcError>>,
            T::Error: ::std::fmt::Display,
        {
            self.error = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for error: {}", e));
            self
        }
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn result<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Task>>,
            T::Error: ::std::fmt::Display,
        {
            self.result = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for result: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<SendTaskResponse> for super::SendTaskResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SendTaskResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                error: value.error?,
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                result: value.result?,
            })
        }
    }
    impl ::std::convert::From<super::SendTaskResponse> for SendTaskResponse {
        fn from(value: super::SendTaskResponse) -> Self {
            Self {
                error: Ok(value.error),
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                result: Ok(value.result),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SendTaskStreamingRequest {
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        method: ::std::result::Result<::std::string::String, ::std::string::String>,
        params: ::std::result::Result<super::TaskSendParams, ::std::string::String>,
    }
    impl ::std::default::Default for SendTaskStreamingRequest {
        fn default() -> Self {
            Self {
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::send_task_streaming_request_jsonrpc()),
                method: Err("no value supplied for method".to_string()),
                params: Err("no value supplied for params".to_string()),
            }
        }
    }
    impl SendTaskStreamingRequest {
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn method<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.method = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for method: {}", e));
            self
        }
        pub fn params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::TaskSendParams>,
            T::Error: ::std::fmt::Display,
        {
            self.params = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for params: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<SendTaskStreamingRequest> for super::SendTaskStreamingRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SendTaskStreamingRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                method: value.method?,
                params: value.params?,
            })
        }
    }
    impl ::std::convert::From<super::SendTaskStreamingRequest> for SendTaskStreamingRequest {
        fn from(value: super::SendTaskStreamingRequest) -> Self {
            Self {
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                method: Ok(value.method),
                params: Ok(value.params),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SendTaskStreamingResponse {
        error: ::std::result::Result<
            ::std::option::Option<super::JsonrpcError>,
            ::std::string::String,
        >,
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        result:
            ::std::result::Result<super::SendTaskStreamingResponseResult, ::std::string::String>,
    }
    impl ::std::default::Default for SendTaskStreamingResponse {
        fn default() -> Self {
            Self {
                error: Ok(Default::default()),
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::send_task_streaming_response_jsonrpc()),
                result: Ok(super::defaults::send_task_streaming_response_result()),
            }
        }
    }
    impl SendTaskStreamingResponse {
        pub fn error<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::JsonrpcError>>,
            T::Error: ::std::fmt::Display,
        {
            self.error = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for error: {}", e));
            self
        }
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn result<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::SendTaskStreamingResponseResult>,
            T::Error: ::std::fmt::Display,
        {
            self.result = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for result: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<SendTaskStreamingResponse> for super::SendTaskStreamingResponse {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SendTaskStreamingResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                error: value.error?,
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                result: value.result?,
            })
        }
    }
    impl ::std::convert::From<super::SendTaskStreamingResponse> for SendTaskStreamingResponse {
        fn from(value: super::SendTaskStreamingResponse) -> Self {
            Self {
                error: Ok(value.error),
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                result: Ok(value.result),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SetTaskPushNotificationRequest {
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        method: ::std::result::Result<::std::string::String, ::std::string::String>,
        params: ::std::result::Result<super::TaskPushNotificationConfig, ::std::string::String>,
    }
    impl ::std::default::Default for SetTaskPushNotificationRequest {
        fn default() -> Self {
            Self {
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::set_task_push_notification_request_jsonrpc()),
                method: Err("no value supplied for method".to_string()),
                params: Err("no value supplied for params".to_string()),
            }
        }
    }
    impl SetTaskPushNotificationRequest {
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn method<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.method = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for method: {}", e));
            self
        }
        pub fn params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::TaskPushNotificationConfig>,
            T::Error: ::std::fmt::Display,
        {
            self.params = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for params: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<SetTaskPushNotificationRequest>
        for super::SetTaskPushNotificationRequest
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SetTaskPushNotificationRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                method: value.method?,
                params: value.params?,
            })
        }
    }
    impl ::std::convert::From<super::SetTaskPushNotificationRequest>
        for SetTaskPushNotificationRequest
    {
        fn from(value: super::SetTaskPushNotificationRequest) -> Self {
            Self {
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                method: Ok(value.method),
                params: Ok(value.params),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct SetTaskPushNotificationResponse {
        error: ::std::result::Result<
            ::std::option::Option<super::JsonrpcError>,
            ::std::string::String,
        >,
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        result: ::std::result::Result<
            ::std::option::Option<super::TaskPushNotificationConfig>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for SetTaskPushNotificationResponse {
        fn default() -> Self {
            Self {
                error: Ok(Default::default()),
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::set_task_push_notification_response_jsonrpc()),
                result: Ok(Default::default()),
            }
        }
    }
    impl SetTaskPushNotificationResponse {
        pub fn error<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::JsonrpcError>>,
            T::Error: ::std::fmt::Display,
        {
            self.error = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for error: {}", e));
            self
        }
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn result<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::TaskPushNotificationConfig>>,
            T::Error: ::std::fmt::Display,
        {
            self.result = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for result: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<SetTaskPushNotificationResponse>
        for super::SetTaskPushNotificationResponse
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: SetTaskPushNotificationResponse,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                error: value.error?,
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                result: value.result?,
            })
        }
    }
    impl ::std::convert::From<super::SetTaskPushNotificationResponse>
        for SetTaskPushNotificationResponse
    {
        fn from(value: super::SetTaskPushNotificationResponse) -> Self {
            Self {
                error: Ok(value.error),
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                result: Ok(value.result),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct Task {
        artifacts: ::std::result::Result<
            ::std::option::Option<::std::vec::Vec<super::Artifact>>,
            ::std::string::String,
        >,
        history: ::std::result::Result<
            ::std::option::Option<::std::vec::Vec<super::Message>>,
            ::std::string::String,
        >,
        id: ::std::result::Result<::std::string::String, ::std::string::String>,
        metadata: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
        session_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
        status: ::std::result::Result<super::TaskStatus, ::std::string::String>,
    }
    impl ::std::default::Default for Task {
        fn default() -> Self {
            Self {
                artifacts: Ok(Default::default()),
                history: Ok(Default::default()),
                id: Err("no value supplied for id".to_string()),
                metadata: Ok(Default::default()),
                session_id: Ok(Default::default()),
                status: Err("no value supplied for status".to_string()),
            }
        }
    }
    impl Task {
        pub fn artifacts<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::vec::Vec<super::Artifact>>>,
            T::Error: ::std::fmt::Display,
        {
            self.artifacts = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for artifacts: {}", e));
            self
        }
        pub fn history<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::vec::Vec<super::Message>>>,
            T::Error: ::std::fmt::Display,
        {
            self.history = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for history: {}", e));
            self
        }
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn metadata<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.metadata = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for metadata: {}", e));
            self
        }
        pub fn session_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.session_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for session_id: {}", e));
            self
        }
        pub fn status<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::TaskStatus>,
            T::Error: ::std::fmt::Display,
        {
            self.status = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for status: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<Task> for super::Task {
        type Error = super::error::ConversionError;
        fn try_from(value: Task) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                artifacts: value.artifacts?,
                history: value.history?,
                id: value.id?,
                metadata: value.metadata?,
                session_id: value.session_id?,
                status: value.status?,
            })
        }
    }
    impl ::std::convert::From<super::Task> for Task {
        fn from(value: super::Task) -> Self {
            Self {
                artifacts: Ok(value.artifacts),
                history: Ok(value.history),
                id: Ok(value.id),
                metadata: Ok(value.metadata),
                session_id: Ok(value.session_id),
                status: Ok(value.status),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TaskArtifactUpdateEvent {
        artifact: ::std::result::Result<super::Artifact, ::std::string::String>,
        id: ::std::result::Result<::std::string::String, ::std::string::String>,
        metadata: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for TaskArtifactUpdateEvent {
        fn default() -> Self {
            Self {
                artifact: Err("no value supplied for artifact".to_string()),
                id: Err("no value supplied for id".to_string()),
                metadata: Ok(Default::default()),
            }
        }
    }
    impl TaskArtifactUpdateEvent {
        pub fn artifact<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::Artifact>,
            T::Error: ::std::fmt::Display,
        {
            self.artifact = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for artifact: {}", e));
            self
        }
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn metadata<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.metadata = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for metadata: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<TaskArtifactUpdateEvent> for super::TaskArtifactUpdateEvent {
        type Error = super::error::ConversionError;
        fn try_from(
            value: TaskArtifactUpdateEvent,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                artifact: value.artifact?,
                id: value.id?,
                metadata: value.metadata?,
            })
        }
    }
    impl ::std::convert::From<super::TaskArtifactUpdateEvent> for TaskArtifactUpdateEvent {
        fn from(value: super::TaskArtifactUpdateEvent) -> Self {
            Self {
                artifact: Ok(value.artifact),
                id: Ok(value.id),
                metadata: Ok(value.metadata),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TaskIdParams {
        id: ::std::result::Result<::std::string::String, ::std::string::String>,
        metadata: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for TaskIdParams {
        fn default() -> Self {
            Self {
                id: Err("no value supplied for id".to_string()),
                metadata: Ok(Default::default()),
            }
        }
    }
    impl TaskIdParams {
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn metadata<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.metadata = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for metadata: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<TaskIdParams> for super::TaskIdParams {
        type Error = super::error::ConversionError;
        fn try_from(
            value: TaskIdParams,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                id: value.id?,
                metadata: value.metadata?,
            })
        }
    }
    impl ::std::convert::From<super::TaskIdParams> for TaskIdParams {
        fn from(value: super::TaskIdParams) -> Self {
            Self {
                id: Ok(value.id),
                metadata: Ok(value.metadata),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TaskNotCancelableError {
        code: ::std::result::Result<i64, ::std::string::String>,
        data: ::std::result::Result<::serde_json::Value, ::std::string::String>,
        message: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for TaskNotCancelableError {
        fn default() -> Self {
            Self {
                code: Err("no value supplied for code".to_string()),
                data: Err("no value supplied for data".to_string()),
                message: Err("no value supplied for message".to_string()),
            }
        }
    }
    impl TaskNotCancelableError {
        pub fn code<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.code = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for code: {}", e));
            self
        }
        pub fn data<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::serde_json::Value>,
            T::Error: ::std::fmt::Display,
        {
            self.data = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data: {}", e));
            self
        }
        pub fn message<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.message = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for message: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<TaskNotCancelableError> for super::TaskNotCancelableError {
        type Error = super::error::ConversionError;
        fn try_from(
            value: TaskNotCancelableError,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                code: value.code?,
                data: value.data?,
                message: value.message?,
            })
        }
    }
    impl ::std::convert::From<super::TaskNotCancelableError> for TaskNotCancelableError {
        fn from(value: super::TaskNotCancelableError) -> Self {
            Self {
                code: Ok(value.code),
                data: Ok(value.data),
                message: Ok(value.message),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TaskNotFoundError {
        code: ::std::result::Result<i64, ::std::string::String>,
        data: ::std::result::Result<::serde_json::Value, ::std::string::String>,
        message: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for TaskNotFoundError {
        fn default() -> Self {
            Self {
                code: Err("no value supplied for code".to_string()),
                data: Err("no value supplied for data".to_string()),
                message: Err("no value supplied for message".to_string()),
            }
        }
    }
    impl TaskNotFoundError {
        pub fn code<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.code = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for code: {}", e));
            self
        }
        pub fn data<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::serde_json::Value>,
            T::Error: ::std::fmt::Display,
        {
            self.data = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data: {}", e));
            self
        }
        pub fn message<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.message = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for message: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<TaskNotFoundError> for super::TaskNotFoundError {
        type Error = super::error::ConversionError;
        fn try_from(
            value: TaskNotFoundError,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                code: value.code?,
                data: value.data?,
                message: value.message?,
            })
        }
    }
    impl ::std::convert::From<super::TaskNotFoundError> for TaskNotFoundError {
        fn from(value: super::TaskNotFoundError) -> Self {
            Self {
                code: Ok(value.code),
                data: Ok(value.data),
                message: Ok(value.message),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TaskPushNotificationConfig {
        id: ::std::result::Result<::std::string::String, ::std::string::String>,
        push_notification_config:
            ::std::result::Result<super::PushNotificationConfig, ::std::string::String>,
    }
    impl ::std::default::Default for TaskPushNotificationConfig {
        fn default() -> Self {
            Self {
                id: Err("no value supplied for id".to_string()),
                push_notification_config: Err(
                    "no value supplied for push_notification_config".to_string()
                ),
            }
        }
    }
    impl TaskPushNotificationConfig {
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn push_notification_config<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::PushNotificationConfig>,
            T::Error: ::std::fmt::Display,
        {
            self.push_notification_config = value.try_into().map_err(|e| {
                format!(
                    "error converting supplied value for push_notification_config: {}",
                    e
                )
            });
            self
        }
    }
    impl ::std::convert::TryFrom<TaskPushNotificationConfig> for super::TaskPushNotificationConfig {
        type Error = super::error::ConversionError;
        fn try_from(
            value: TaskPushNotificationConfig,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                id: value.id?,
                push_notification_config: value.push_notification_config?,
            })
        }
    }
    impl ::std::convert::From<super::TaskPushNotificationConfig> for TaskPushNotificationConfig {
        fn from(value: super::TaskPushNotificationConfig) -> Self {
            Self {
                id: Ok(value.id),
                push_notification_config: Ok(value.push_notification_config),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TaskQueryParams {
        history_length: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        id: ::std::result::Result<::std::string::String, ::std::string::String>,
        metadata: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for TaskQueryParams {
        fn default() -> Self {
            Self {
                history_length: Ok(Default::default()),
                id: Err("no value supplied for id".to_string()),
                metadata: Ok(Default::default()),
            }
        }
    }
    impl TaskQueryParams {
        pub fn history_length<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.history_length = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for history_length: {}", e));
            self
        }
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn metadata<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.metadata = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for metadata: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<TaskQueryParams> for super::TaskQueryParams {
        type Error = super::error::ConversionError;
        fn try_from(
            value: TaskQueryParams,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                history_length: value.history_length?,
                id: value.id?,
                metadata: value.metadata?,
            })
        }
    }
    impl ::std::convert::From<super::TaskQueryParams> for TaskQueryParams {
        fn from(value: super::TaskQueryParams) -> Self {
            Self {
                history_length: Ok(value.history_length),
                id: Ok(value.id),
                metadata: Ok(value.metadata),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TaskResubscriptionRequest {
        id: ::std::result::Result<::std::option::Option<super::Id>, ::std::string::String>,
        jsonrpc: ::std::result::Result<::std::string::String, ::std::string::String>,
        method: ::std::result::Result<::std::string::String, ::std::string::String>,
        params: ::std::result::Result<super::TaskQueryParams, ::std::string::String>,
    }
    impl ::std::default::Default for TaskResubscriptionRequest {
        fn default() -> Self {
            Self {
                id: Ok(Default::default()),
                jsonrpc: Ok(super::defaults::task_resubscription_request_jsonrpc()),
                method: Err("no value supplied for method".to_string()),
                params: Err("no value supplied for params".to_string()),
            }
        }
    }
    impl TaskResubscriptionRequest {
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Id>>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn jsonrpc<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.jsonrpc = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for jsonrpc: {}", e));
            self
        }
        pub fn method<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.method = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for method: {}", e));
            self
        }
        pub fn params<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::TaskQueryParams>,
            T::Error: ::std::fmt::Display,
        {
            self.params = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for params: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<TaskResubscriptionRequest> for super::TaskResubscriptionRequest {
        type Error = super::error::ConversionError;
        fn try_from(
            value: TaskResubscriptionRequest,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                id: value.id?,
                jsonrpc: value.jsonrpc?,
                method: value.method?,
                params: value.params?,
            })
        }
    }
    impl ::std::convert::From<super::TaskResubscriptionRequest> for TaskResubscriptionRequest {
        fn from(value: super::TaskResubscriptionRequest) -> Self {
            Self {
                id: Ok(value.id),
                jsonrpc: Ok(value.jsonrpc),
                method: Ok(value.method),
                params: Ok(value.params),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TaskSendParams {
        history_length: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        id: ::std::result::Result<::std::string::String, ::std::string::String>,
        message: ::std::result::Result<super::Message, ::std::string::String>,
        metadata: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
        push_notification: ::std::result::Result<
            ::std::option::Option<super::PushNotificationConfig>,
            ::std::string::String,
        >,
        session_id: ::std::result::Result<
            ::std::option::Option<::std::string::String>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for TaskSendParams {
        fn default() -> Self {
            Self {
                history_length: Ok(Default::default()),
                id: Err("no value supplied for id".to_string()),
                message: Err("no value supplied for message".to_string()),
                metadata: Ok(Default::default()),
                push_notification: Ok(Default::default()),
                session_id: Ok(Default::default()),
            }
        }
    }
    impl TaskSendParams {
        pub fn history_length<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<i64>>,
            T::Error: ::std::fmt::Display,
        {
            self.history_length = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for history_length: {}", e));
            self
        }
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn message<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::Message>,
            T::Error: ::std::fmt::Display,
        {
            self.message = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for message: {}", e));
            self
        }
        pub fn metadata<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.metadata = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for metadata: {}", e));
            self
        }
        pub fn push_notification<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::PushNotificationConfig>>,
            T::Error: ::std::fmt::Display,
        {
            self.push_notification = value.try_into().map_err(|e| {
                format!(
                    "error converting supplied value for push_notification: {}",
                    e
                )
            });
            self
        }
        pub fn session_id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
            T::Error: ::std::fmt::Display,
        {
            self.session_id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for session_id: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<TaskSendParams> for super::TaskSendParams {
        type Error = super::error::ConversionError;
        fn try_from(
            value: TaskSendParams,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                history_length: value.history_length?,
                id: value.id?,
                message: value.message?,
                metadata: value.metadata?,
                push_notification: value.push_notification?,
                session_id: value.session_id?,
            })
        }
    }
    impl ::std::convert::From<super::TaskSendParams> for TaskSendParams {
        fn from(value: super::TaskSendParams) -> Self {
            Self {
                history_length: Ok(value.history_length),
                id: Ok(value.id),
                message: Ok(value.message),
                metadata: Ok(value.metadata),
                push_notification: Ok(value.push_notification),
                session_id: Ok(value.session_id),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TaskStatus {
        message:
            ::std::result::Result<::std::option::Option<super::Message>, ::std::string::String>,
        state: ::std::result::Result<super::TaskState, ::std::string::String>,
        timestamp: ::std::result::Result<
            ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for TaskStatus {
        fn default() -> Self {
            Self {
                message: Ok(Default::default()),
                state: Err("no value supplied for state".to_string()),
                timestamp: Ok(Default::default()),
            }
        }
    }
    impl TaskStatus {
        pub fn message<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::option::Option<super::Message>>,
            T::Error: ::std::fmt::Display,
        {
            self.message = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for message: {}", e));
            self
        }
        pub fn state<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::TaskState>,
            T::Error: ::std::fmt::Display,
        {
            self.state = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for state: {}", e));
            self
        }
        pub fn timestamp<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.timestamp = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for timestamp: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<TaskStatus> for super::TaskStatus {
        type Error = super::error::ConversionError;
        fn try_from(
            value: TaskStatus,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                message: value.message?,
                state: value.state?,
                timestamp: value.timestamp?,
            })
        }
    }
    impl ::std::convert::From<super::TaskStatus> for TaskStatus {
        fn from(value: super::TaskStatus) -> Self {
            Self {
                message: Ok(value.message),
                state: Ok(value.state),
                timestamp: Ok(value.timestamp),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TaskStatusUpdateEvent {
        final_: ::std::result::Result<bool, ::std::string::String>,
        id: ::std::result::Result<::std::string::String, ::std::string::String>,
        metadata: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
        status: ::std::result::Result<super::TaskStatus, ::std::string::String>,
    }
    impl ::std::default::Default for TaskStatusUpdateEvent {
        fn default() -> Self {
            Self {
                final_: Ok(Default::default()),
                id: Err("no value supplied for id".to_string()),
                metadata: Ok(Default::default()),
                status: Err("no value supplied for status".to_string()),
            }
        }
    }
    impl TaskStatusUpdateEvent {
        pub fn final_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<bool>,
            T::Error: ::std::fmt::Display,
        {
            self.final_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for final_: {}", e));
            self
        }
        pub fn id<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.id = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for id: {}", e));
            self
        }
        pub fn metadata<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.metadata = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for metadata: {}", e));
            self
        }
        pub fn status<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::TaskStatus>,
            T::Error: ::std::fmt::Display,
        {
            self.status = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for status: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<TaskStatusUpdateEvent> for super::TaskStatusUpdateEvent {
        type Error = super::error::ConversionError;
        fn try_from(
            value: TaskStatusUpdateEvent,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                final_: value.final_?,
                id: value.id?,
                metadata: value.metadata?,
                status: value.status?,
            })
        }
    }
    impl ::std::convert::From<super::TaskStatusUpdateEvent> for TaskStatusUpdateEvent {
        fn from(value: super::TaskStatusUpdateEvent) -> Self {
            Self {
                final_: Ok(value.final_),
                id: Ok(value.id),
                metadata: Ok(value.metadata),
                status: Ok(value.status),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct TextPart {
        metadata: ::std::result::Result<
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
            ::std::string::String,
        >,
        text: ::std::result::Result<::std::string::String, ::std::string::String>,
        type_: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for TextPart {
        fn default() -> Self {
            Self {
                metadata: Ok(Default::default()),
                text: Err("no value supplied for text".to_string()),
                type_: Err("no value supplied for type_".to_string()),
            }
        }
    }
    impl TextPart {
        pub fn metadata<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<
                    ::serde_json::Map<::std::string::String, ::serde_json::Value>,
                >,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.metadata = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for metadata: {}", e));
            self
        }
        pub fn text<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.text = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for text: {}", e));
            self
        }
        pub fn type_<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.type_ = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for type_: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<TextPart> for super::TextPart {
        type Error = super::error::ConversionError;
        fn try_from(value: TextPart) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                metadata: value.metadata?,
                text: value.text?,
                type_: value.type_?,
            })
        }
    }
    impl ::std::convert::From<super::TextPart> for TextPart {
        fn from(value: super::TextPart) -> Self {
            Self {
                metadata: Ok(value.metadata),
                text: Ok(value.text),
                type_: Ok(value.type_),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct UnsupportedOperationError {
        code: ::std::result::Result<i64, ::std::string::String>,
        data: ::std::result::Result<::serde_json::Value, ::std::string::String>,
        message: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for UnsupportedOperationError {
        fn default() -> Self {
            Self {
                code: Err("no value supplied for code".to_string()),
                data: Err("no value supplied for data".to_string()),
                message: Err("no value supplied for message".to_string()),
            }
        }
    }
    impl UnsupportedOperationError {
        pub fn code<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.code = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for code: {}", e));
            self
        }
        pub fn data<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::serde_json::Value>,
            T::Error: ::std::fmt::Display,
        {
            self.data = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for data: {}", e));
            self
        }
        pub fn message<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.message = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for message: {}", e));
            self
        }
    }
    impl ::std::convert::TryFrom<UnsupportedOperationError> for super::UnsupportedOperationError {
        type Error = super::error::ConversionError;
        fn try_from(
            value: UnsupportedOperationError,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                code: value.code?,
                data: value.data?,
                message: value.message?,
            })
        }
    }
    impl ::std::convert::From<super::UnsupportedOperationError> for UnsupportedOperationError {
        fn from(value: super::UnsupportedOperationError) -> Self {
            Self {
                code: Ok(value.code),
                data: Ok(value.data),
                message: Ok(value.message),
            }
        }
    }
}
#[doc = r" Generation of default values for serde."]
pub mod defaults {
    pub(super) fn agent_card_default_input_modes() -> ::std::vec::Vec<::std::string::String> {
        vec!["text".to_string()]
    }
    pub(super) fn agent_card_default_output_modes() -> ::std::vec::Vec<::std::string::String> {
        vec!["text".to_string()]
    }
    pub(super) fn cancel_task_request_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn cancel_task_response_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn get_task_push_notification_request_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn get_task_push_notification_response_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn get_task_request_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn get_task_response_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn jsonrpc_message_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn jsonrpc_request_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn jsonrpc_response_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn send_task_request_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn send_task_response_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn send_task_streaming_request_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn send_task_streaming_response_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn send_task_streaming_response_result() -> super::SendTaskStreamingResponseResult {
        super::SendTaskStreamingResponseResult::Variant2
    }
    pub(super) fn set_task_push_notification_request_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn set_task_push_notification_response_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
    pub(super) fn task_resubscription_request_jsonrpc() -> ::std::string::String {
        "2.0".to_string()
    }
}
