use bytes::Bytes;
use std::collections::HashMap;
use tokio::sync::oneshot;

/// HTTP request from the host
#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<Bytes>,
}

/// Response body variants
pub enum ResponseBody {
    /// No body
    None,
    /// Buffered bytes
    Bytes(Bytes),
    /// Streaming body (for future use)
    Stream(tokio::sync::mpsc::Receiver<Result<Bytes, String>>),
}

impl std::fmt::Debug for ResponseBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResponseBody::None => write!(f, "None"),
            ResponseBody::Bytes(b) => write!(f, "Bytes({} bytes)", b.len()),
            ResponseBody::Stream(_) => write!(f, "Stream(...)"),
        }
    }
}

impl ResponseBody {
    /// Get bytes if this is a buffered response
    pub fn as_bytes(&self) -> Option<&Bytes> {
        match self {
            ResponseBody::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// Check if body is empty/none
    pub fn is_none(&self) -> bool {
        matches!(self, ResponseBody::None)
    }

    /// Check if body is a stream
    pub fn is_stream(&self) -> bool {
        matches!(self, ResponseBody::Stream(_))
    }
}

/// HTTP response to return to the host
#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: ResponseBody,
}

/// Task types that can be executed by the worker
pub enum Task {
    /// Handle a fetch event
    Fetch {
        request: HttpRequest,
        response_tx: oneshot::Sender<HttpResponse>,
    },
    /// Handle a scheduled event (cron)
    Scheduled {
        cron: String,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
}

impl Task {
    /// Create a fetch task and return the receiver for the response
    pub fn fetch(request: HttpRequest) -> (Self, oneshot::Receiver<HttpResponse>) {
        let (tx, rx) = oneshot::channel();
        (
            Task::Fetch {
                request,
                response_tx: tx,
            },
            rx,
        )
    }

    /// Create a scheduled task and return the receiver for the result
    pub fn scheduled(cron: &str) -> (Self, oneshot::Receiver<Result<(), String>>) {
        let (tx, rx) = oneshot::channel();
        (
            Task::Scheduled {
                cron: cron.to_string(),
                response_tx: tx,
            },
            rx,
        )
    }
}
