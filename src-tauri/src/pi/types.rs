use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const PI_RUNTIME_KIND: &str = "pi";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PiRequestId(pub String);

impl PiRequestId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for PiRequestId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for PiRequestId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum PiCommand {
    Prompt { prompt: String },
    Steer { instruction: String },
    FollowUp { prompt: String },
    Abort,
    SetModel { model: String },
    Compact,
    GetState,
}

impl PiCommand {
    pub fn method(&self) -> &'static str {
        match self {
            Self::Prompt { .. } => "prompt",
            Self::Steer { .. } => "steer",
            Self::FollowUp { .. } => "follow_up",
            Self::Abort => "abort",
            Self::SetModel { .. } => "set_model",
            Self::Compact => "compact",
            Self::GetState => "get_state",
        }
    }

    pub fn params(&self) -> Value {
        match self {
            Self::Prompt { prompt } => json!({ "prompt": prompt }),
            Self::Steer { instruction } => json!({ "instruction": instruction }),
            Self::FollowUp { prompt } => json!({ "prompt": prompt }),
            Self::Abort | Self::Compact | Self::GetState => json!({}),
            Self::SetModel { model } => json!({ "model": model }),
        }
    }

    pub fn to_rpc_request(&self, id: Option<&PiRequestId>) -> Value {
        let mut obj = Map::new();
        if let Some(id) = id {
            obj.insert("id".into(), Value::String(id.0.clone()));
        }
        obj.insert("method".into(), Value::String(self.method().into()));
        obj.insert("params".into(), self.params());
        Value::Object(obj)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PiEvent {
    AgentStart {
        raw: Value,
    },
    TurnStart {
        raw: Value,
    },
    MessageUpdate {
        raw: Value,
    },
    ToolExecutionStart {
        raw: Value,
    },
    ToolExecutionUpdate {
        raw: Value,
    },
    ToolExecutionEnd {
        raw: Value,
    },
    ExtensionUiBlocked {
        method: Option<String>,
        reason: String,
        raw: Value,
    },
    ExtensionUiTimeline {
        method: Option<String>,
        raw: Value,
    },
    AgentEnd {
        raw: Value,
    },
    Response {
        id: String,
        result: Option<Value>,
        error: Option<Value>,
        raw: Value,
    },
    RuntimeKind {
        #[serde(rename = "runtimeKind")]
        runtime_kind: &'static str,
    },
    ProtocolError {
        reason: String,
        raw: Option<Value>,
    },
    ResponseTimeout {
        id: String,
        timeout_ms: u64,
    },
    Stderr {
        line: String,
    },
    MalformedJson {
        line: String,
        error: String,
    },
    Exit {
        exit: crate::process::ProcessExit,
    },
}
