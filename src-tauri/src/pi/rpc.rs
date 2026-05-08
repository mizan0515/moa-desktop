use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::pi::types::{PiEvent, PiRequestId, PI_RUNTIME_KIND};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiRpcFrameError {
    FrameTooLarge,
    InvalidUtf8(String),
}

#[derive(Debug, Clone)]
pub struct JsonlFramer {
    buf: Vec<u8>,
    max_frame_bytes: usize,
}

impl JsonlFramer {
    pub fn new(max_frame_bytes: usize) -> Self {
        Self {
            buf: Vec::new(),
            max_frame_bytes,
        }
    }

    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<String>, PiRpcFrameError> {
        let mut out = Vec::new();
        for &byte in chunk {
            if byte == b'\n' {
                if self.buf.last() == Some(&b'\r') {
                    self.buf.pop();
                }
                let frame = String::from_utf8(std::mem::take(&mut self.buf))
                    .map_err(|e| PiRpcFrameError::InvalidUtf8(e.to_string()))?;
                out.push(frame);
                continue;
            }

            self.buf.push(byte);
            if self.buf.len() > self.max_frame_bytes {
                return Err(PiRpcFrameError::FrameTooLarge);
            }
        }
        Ok(out)
    }

    pub fn buffered_len(&self) -> usize {
        self.buf.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingError {
    DuplicateId(String),
    UnknownResponseId(String),
    QueueOverflow { max: usize },
}

#[derive(Debug, Clone)]
pub struct PendingRequests {
    pending: HashMap<String, Instant>,
    completed: HashSet<String>,
    max_pending: usize,
}

impl PendingRequests {
    pub fn new(max_pending: usize) -> Self {
        Self {
            pending: HashMap::new(),
            completed: HashSet::new(),
            max_pending: max_pending.max(1),
        }
    }

    pub fn begin(&mut self, id: &PiRequestId) -> Result<(), PendingError> {
        let id = id.as_str().to_string();
        if self.pending.contains_key(&id) || self.completed.contains(&id) {
            return Err(PendingError::DuplicateId(id));
        }
        if self.pending.len() >= self.max_pending {
            return Err(PendingError::QueueOverflow {
                max: self.max_pending,
            });
        }
        self.pending.insert(id, Instant::now());
        Ok(())
    }

    pub fn complete(&mut self, id: &str) -> Result<(), PendingError> {
        if self.pending.remove(id).is_some() {
            self.completed.insert(id.to_string());
            Ok(())
        } else if self.completed.contains(id) {
            Err(PendingError::DuplicateId(id.to_string()))
        } else {
            Err(PendingError::UnknownResponseId(id.to_string()))
        }
    }

    pub fn timeout(&mut self, id: &str) -> bool {
        self.pending.remove(id).is_some()
    }

    pub fn cancel(&mut self, id: &str) {
        self.pending.remove(id);
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

pub fn parse_record(line: &str, pending: &mut PendingRequests) -> PiEvent {
    let raw: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => {
            return PiEvent::MalformedJson {
                line: line.to_string(),
                error: error.to_string(),
            };
        }
    };

    let Some(obj) = raw.as_object() else {
        return PiEvent::ProtocolError {
            reason: "Pi RPC record must be a JSON object".into(),
            raw: Some(raw),
        };
    };

    if let Some(id) = obj.get("id").and_then(Value::as_str) {
        if obj.contains_key("result") || obj.contains_key("error") || obj.contains_key("response") {
            if let Err(error) = pending.complete(id) {
                return pending_error_event(error, raw);
            }
            return PiEvent::Response {
                id: id.to_string(),
                result: obj.get("result").or_else(|| obj.get("response")).cloned(),
                error: obj.get("error").cloned(),
                raw,
            };
        }
    }

    let type_ = obj
        .get("type")
        .or_else(|| obj.get("event"))
        .or_else(|| obj.get("method"))
        .and_then(Value::as_str)
        .unwrap_or("");

    match type_ {
        "agent_start" => PiEvent::AgentStart { raw },
        "turn_start" => PiEvent::TurnStart { raw },
        "message_update" => PiEvent::MessageUpdate { raw },
        "tool_execution_start" => PiEvent::ToolExecutionStart { raw },
        "tool_execution_update" => PiEvent::ToolExecutionUpdate { raw },
        "tool_execution_end" => PiEvent::ToolExecutionEnd { raw },
        "agent_end" => PiEvent::AgentEnd { raw },
        "extension_ui_request" => extension_ui_event(raw),
        _ => PiEvent::ProtocolError {
            reason: format!("unknown Pi RPC event type {type_:?}"),
            raw: Some(raw),
        },
    }
}

pub fn runtime_kind_event() -> PiEvent {
    PiEvent::RuntimeKind {
        runtime_kind: PI_RUNTIME_KIND,
    }
}

pub fn timeout_duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn extension_ui_event(raw: Value) -> PiEvent {
    let method = raw
        .get("method")
        .or_else(|| raw.get("name"))
        .or_else(|| raw.get("params").and_then(|p| p.get("method")))
        .and_then(Value::as_str)
        .map(str::to_string);
    let is_fire_and_forget = matches!(
        method.as_deref(),
        Some("notify" | "setStatus" | "setWidget")
    );

    if is_fire_and_forget {
        PiEvent::ExtensionUiTimeline { method, raw }
    } else {
        PiEvent::ExtensionUiBlocked {
            method,
            reason: "extension_ui_request blocked until T15e capability gate is implemented".into(),
            raw,
        }
    }
}

fn pending_error_event(error: PendingError, raw: Value) -> PiEvent {
    let reason = match error {
        PendingError::DuplicateId(id) => format!("duplicate Pi RPC response id {id}"),
        PendingError::UnknownResponseId(id) => format!("unknown Pi RPC response id {id}"),
        PendingError::QueueOverflow { max } => {
            format!("Pi RPC pending request queue overflow at max {max}")
        }
    };
    PiEvent::ProtocolError {
        reason,
        raw: Some(raw),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pi::types::PiCommand;

    #[test]
    fn pi_rpc_jsonl_framer_accepts_only_lf_delimited_records_and_strips_cr() {
        let mut framer = JsonlFramer::new(1024);
        let frames = framer.push(b"{\"a\":1}\r\n{\"b\":2}").unwrap();
        assert_eq!(frames, vec![r#"{"a":1}"#]);
        assert_eq!(framer.buffered_len(), 7);

        let frames = framer.push("\u{2028}{\"c\":3}\n".as_bytes()).unwrap();
        assert_eq!(frames, vec!["{\"b\":2}\u{2028}{\"c\":3}"]);
    }

    #[test]
    fn pi_rpc_command_mapping_uses_methods_and_optional_ids() {
        let request = PiCommand::SetModel {
            model: "pi-fast".into(),
        }
        .to_rpc_request(Some(&"m-1".into()));
        assert_eq!(request["id"], "m-1");
        assert_eq!(request["method"], "set_model");
        assert_eq!(request["params"]["model"], "pi-fast");

        let request = PiCommand::Compact.to_rpc_request(None);
        assert!(request.get("id").is_none());
        assert_eq!(request["method"], "compact");
    }

    #[test]
    fn pi_rpc_duplicate_and_unknown_response_ids_are_protocol_errors() {
        let mut pending = PendingRequests::new(8);
        pending.begin(&"r-1".into()).unwrap();

        assert!(matches!(
            parse_record(r#"{"id":"r-1","result":{"ok":true}}"#, &mut pending),
            PiEvent::Response { .. }
        ));

        match parse_record(r#"{"id":"r-1","result":{"ok":true}}"#, &mut pending) {
            PiEvent::ProtocolError { reason, .. } => assert!(reason.contains("duplicate")),
            other => panic!("expected duplicate protocol error, got {other:?}"),
        }

        match parse_record(r#"{"id":"missing","result":{}}"#, &mut pending) {
            PiEvent::ProtocolError { reason, .. } => assert!(reason.contains("unknown")),
            other => panic!("expected unknown protocol error, got {other:?}"),
        }
    }

    #[test]
    fn pi_rpc_pending_queue_overflow_is_rejected() {
        let mut pending = PendingRequests::new(1);
        pending.begin(&"r-1".into()).unwrap();
        assert_eq!(
            pending.begin(&"r-2".into()),
            Err(PendingError::QueueOverflow { max: 1 })
        );
    }

    #[test]
    fn pi_rpc_extension_ui_request_dialogs_are_blocked_until_t15e() {
        let mut pending = PendingRequests::new(8);
        match parse_record(
            r#"{"type":"extension_ui_request","method":"confirm","message":"Continue?"}"#,
            &mut pending,
        ) {
            PiEvent::ExtensionUiBlocked { method, reason, .. } => {
                assert_eq!(method.as_deref(), Some("confirm"));
                assert!(reason.contains("T15e"));
            }
            other => panic!("expected blocked extension UI request, got {other:?}"),
        }

        match parse_record(
            r#"{"type":"extension_ui_request","method":"notify","message":"Done"}"#,
            &mut pending,
        ) {
            PiEvent::ExtensionUiTimeline { method, .. } => {
                assert_eq!(method.as_deref(), Some("notify"));
            }
            other => panic!("expected timeline extension UI request, got {other:?}"),
        }
    }
}
