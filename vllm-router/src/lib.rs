use axum::{
    extract::State,
    http::StatusCode,
    response::{sse::Event, IntoResponse, Sse},
    routing::{get, post},
    Json, Router,
};
use dashmap::DashMap;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use zeromq::{PullSocket, RouterSocket, Socket, SocketRecv, SocketSend};

// --- OpenAI API Models ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: Option<serde_json::Value>, // Can be string or array of parts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub n: Option<u32>,
    pub max_tokens: Option<u32>,
    pub max_completion_tokens: Option<u32>,
    pub stop: Option<serde_json::Value>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub logit_bias: Option<HashMap<String, f32>>,
    pub user: Option<String>,
    pub tools: Option<serde_json::Value>,
    pub tool_choice: Option<serde_json::Value>,
    pub response_format: Option<serde_json::Value>,
    pub seed: Option<i64>,
    pub top_k: Option<i32>,
    pub min_p: Option<f32>,
    pub repetition_penalty: Option<f32>,
    pub logprobs: Option<bool>,
    pub top_logprobs: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompletionRequest {
    pub model: String,
    pub prompt: serde_json::Value,
    #[serde(default)]
    pub stream: bool,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub n: Option<u32>,
    pub stop: Option<serde_json::Value>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub logit_bias: Option<HashMap<String, f32>>,
    pub logprobs: Option<u32>,
    pub echo: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: serde_json::Value,
    pub user: Option<String>,
    pub encoding_format: Option<String>,
    pub dimensions: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: UsageInfo,
}

#[derive(Debug, Serialize)]
pub struct ChatChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatChunkChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageInfo>,
}

#[derive(Debug, Serialize)]
pub struct ChatChunkChoice {
    pub index: u32,
    pub delta: ChatMessageDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatMessageDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Default)]
pub struct UsageInfo {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Serialize)]
pub struct ModelList {
    pub object: String,
    pub data: Vec<ModelCard>,
}

#[derive(Debug, Serialize)]
pub struct ModelCard {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

#[derive(Debug, Serialize)]
pub struct EmbeddingResponse {
    pub object: String,
    pub data: Vec<EmbeddingData>,
    pub model: String,
    pub usage: UsageInfo,
}

#[derive(Debug, Serialize)]
pub struct EmbeddingData {
    pub object: String,
    pub embedding: Vec<f32>,
    pub index: u32,
}

// --- vLLM Engine Protocol (msgspec array_like=True) ---

#[derive(Debug, Serialize)]
struct EngineCoreRequest(
    String,                          // request_id
    Option<Vec<u32>>,                // prompt_token_ids
    Option<serde_json::Value>,       // mm_features
    Option<serde_json::Value>,       // sampling_params
    Option<serde_json::Value>,       // pooling_params
    f64,                             // arrival_time
    Option<serde_json::Value>,       // lora_request
    Option<String>,                  // cache_salt
    Option<u32>,                     // data_parallel_rank
    Option<serde_json::Value>,       // prompt_embeds
    u32,                             // client_index
    u32,                             // current_wave
    i32,                             // priority
    Option<HashMap<String, String>>, // trace_headers
    bool,                            // resumable
    Option<String>,                  // external_req_id
    Option<bool>,                    // reasoning_ended
);

// --- Server State & Handlers ---

#[derive(Clone)]
pub struct AppState {
    input_socket: Arc<Mutex<RouterSocket>>,
    engine_identities: Arc<Mutex<Vec<bytes::Bytes>>>,
    request_streams: Arc<DashMap<String, mpsc::UnboundedSender<serde_json::Value>>>,
    renderer: Arc<PyObject>,
}

async fn health_check() -> &'static str {
    "OK"
}

async fn list_models(State(state): State<AppState>) -> impl IntoResponse {
    let model_id: String = Python::with_gil(|py| {
        let renderer = state.renderer.bind(py);
        renderer
            .getattr("model_config")
            .and_then(|m| m.getattr("model"))
            .and_then(|m| m.extract())
            .unwrap_or_else(|_| "vllm-model".into())
    });

    Json(ModelList {
        object: "list".into(),
        data: vec![ModelCard {
            id: model_id,
            object: "model".into(),
            created: 1700000000,
            owned_by: "vllm".into(),
        }],
    })
}

async fn chat_completions(
    State(state): State<AppState>,
    Json(payload): Json<ChatCompletionRequest>,
) -> impl IntoResponse {
    let request_id = format!("chatcmpl-{}", Uuid::new_v4());
    debug!("Received chat completion request: {}", request_id);

    let res: PyResult<(Vec<u32>, String)> = Python::with_gil(|py| {
        let _renderer = state.renderer.bind(py);
        let req_json = serde_json::to_string(&payload)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        let json_mod = py.import_bound("json")?;
        let _req_dict = json_mod.call_method1("loads", (req_json,))?;

    let res: PyResult<(Vec<u32>, String)> = Python::with_gil(|py| {
        let renderer = state.renderer.bind(py);
        let tokenizer = renderer.getattr("renderer").and_then(|r| r.getattr("tokenizer"))?;
        
        let req_json = serde_json::to_string(&payload)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        let json_mod = py.import_bound("json")?;
        let req_dict = json_mod.call_method1("loads", (req_json,))?;

        let prompt = renderer.call_method1("render_messages", (req_dict,))?;
        let prompt_str = prompt.extract::<String>()?;
        
        let ids: Vec<u32> = tokenizer.call_method1("encode", (prompt_str,))
            ?.extract()?;
            
        Ok((ids, prompt_str))
    });
        Ok((ids, "rendered prompt".into()))
    });

    let (prompt_token_ids, _) = match res {
        Ok(v) => v,
        Err(e) => {
            error!("Error rendering chat: {:?}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {:?}", e)).into_response();
        }
    };

    handle_engine_request(
        state,
        payload.model,
        request_id,
        prompt_token_ids,
        payload.stream,
        payload.temperature,
        payload.max_tokens.or(payload.max_completion_tokens),
        payload.top_p,
        payload.n,
        false, // is_embedding
    )
    .await
}

async fn completions(
    State(state): State<AppState>,
    Json(payload): Json<CompletionRequest>,
) -> impl IntoResponse {
    let request_id = format!("cmpl-{}", Uuid::new_v4());

    let prompt_token_ids: Vec<u32> = Python::with_gil(|py| {
        let renderer = state.renderer.bind(py);
    let prompt_token_ids: Vec<u32> = Python::with_gil(|py| {
        let renderer = state.renderer.bind(py);
        let tokenizer = renderer.getattr("renderer").and_then(|r| r.getattr("tokenizer")).map_err(|e| {
            error!("Failed to get tokenizer: {:?}", e);
            PyErr::new::<pyo3::exceptions::PyAttributeError, _>("Failed to get tokenizer")
        })?;
        let prompt_str = match &payload.prompt {
            serde_json::Value::String(s) => s.clone(),
            _ => "".into(),
        };
        tokenizer
            .call_method1("encode", (prompt_str,))
            .map_err(|e| {
                error!("Failed to encode prompt: {:?}", e);
                e
            })?
            .extract()
            .unwrap_or_default()
    });
        let prompt_str = match &payload.prompt {
            serde_json::Value::String(s) => s.clone(),
            _ => "".into(),
        };
        tokenizer
            .call_method1("encode", (prompt_str,))
            .unwrap()
            .extract()
            .unwrap_or_default()
    });

    handle_engine_request(
        state,
        payload.model,
        request_id,
        prompt_token_ids,
        payload.stream,
        payload.temperature,
        payload.max_tokens,
        payload.top_p,
        payload.n,
        false, // is_embedding
    )
    .await
}

async fn embeddings(
    State(state): State<AppState>,
    Json(payload): Json<EmbeddingRequest>,
) -> impl IntoResponse {
    let request_id = format!("emb-{}", Uuid::new_v4());

    let prompt_token_ids: Vec<u32> = Python::with_gil(|py| {
        let renderer = state.renderer.bind(py);
    let prompt_token_ids: Vec<u32> = Python::with_gil(|py| {
        let renderer = state.renderer.bind(py);
        let tokenizer = renderer.getattr("renderer")?.getattr("tokenizer")?;
        let input_str = match &payload.input {
            serde_json::Value::String(s) => s.clone(),
            _ => "".into(), // Handle array of strings if needed
        };
        tokenizer
            .call_method1("encode", (input_str,))
            .and_then(|v| v.extract())
            .unwrap_or_default()
    });
        let input_str = match &payload.input {
            serde_json::Value::String(s) => s.clone(),
            _ => "".into(), // Handle array of strings if needed
        };
        tokenizer
            .call_method1("encode", (input_str,))
            .unwrap()
            .extract()
            .unwrap_or_default()
    });

    handle_engine_request(
        state,
        payload.model,
        request_id,
        prompt_token_ids,
        false, // stream
        None,  // temperature
        None,  // max_tokens
        None,  // top_p
        None,  // n
        true,  // is_embedding
    )
    .await
}

async fn handle_engine_request(
    state: AppState,
    model: String,
    request_id: String,
    prompt_token_ids: Vec<u32>,
    stream: bool,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    top_p: Option<f32>,
    n: Option<u32>,
    is_embedding: bool,
) -> axum::response::Response {
    let prompt_len = prompt_token_ids.len() as u32;

    let sampling_params = if !is_embedding {
        Some(serde_json::json!({
            "temperature": temperature.unwrap_or(1.0),
            "max_tokens": max_tokens.unwrap_or(16),
            "n": n.unwrap_or(1),
            "top_p": top_p.unwrap_or(1.0),
            "output_kind": if stream { 1 } else { 0 },
            "skip_clone": true,
        }))
    } else {
        None
    };

    let pooling_params = if is_embedding {
        Some(serde_json::json!({
            "additional_metadata": {},
        }))
    } else {
        None
    };

    let engine_req = EngineCoreRequest(
        request_id.clone(),
        Some(prompt_token_ids),
        None,
        sampling_params,
        pooling_params,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
        None,
        None,
        None,
        None,
        0, // client_index
        0, // current_wave
        0, // priority
        None,
        false, // resumable
        Some(request_id.clone()),
        None,
    );

    let (tx, mut rx) = mpsc::unbounded_channel();
    state.request_streams.insert(request_id.clone(), tx);

    let msg = rmp_serde::to_vec(&engine_req).unwrap();

        let idents = state.engine_identities.lock().await;
        if idents.is_empty() {
            warn!("No engine registered yet!");
            return (StatusCode::SERVICE_UNAVAILABLE, "No engine available").into_response();
        }
        let idx = ROUND_ROBIN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % idents.len();
        let identity = idents[idx].clone();
        let identity = idents[idx].clone();
        let mut zmq_msg = zeromq::ZmqMessage::from(identity);
        zmq_msg.push_back(bytes::Bytes::from_static(b"")); // empty delimiter
        zmq_msg.push_back(bytes::Bytes::from_static(b"\x00")); // ADD
        zmq_msg.push_back(msg.into());

        let mut socket = state.input_socket.lock().await;
        if let Err(e) = socket.send(zmq_msg).await {
            error!("ZMQ Send error: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("ZMQ error: {:?}", e),
            )
                .into_response();
        }
    }

    if is_embedding {
        while let Some(output) = rx.recv().await {
            if let Some(utility) = output.get(4) {
                if !utility.is_null() {
                    let embedding: Vec<f32> = utility
                        .as_array()
                        .map(|a| a.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect())
                        .unwrap_or_default();
                    return Json(EmbeddingResponse {
                        object: "list".into(),
                        data: vec![EmbeddingData {
                            object: "embedding".into(),
                            embedding,
                            index: 0,
                        }],
                        model: model.clone(),
                        usage: UsageInfo {
                            prompt_tokens: prompt_len,
                            completion_tokens: 0,
                            total_tokens: prompt_len,
                        },
                    })
                    .into_response();
                }
            }
        }
        return (StatusCode::INTERNAL_SERVER_ERROR, "No embedding returned").into_response();
    }

    if stream {
        let renderer_clone = state.renderer.clone();
        let stream_res = async_stream::stream! {
            let mut total_completion_tokens = 0;
            while let Some(output) = rx.recv().await {
                let req_id = output.get(0).and_then(|v| v.as_str()).unwrap_or("");
                let new_tokens: Vec<u32> = output.get(1).and_then(|v| v.as_array())
                    .map(|a| a.iter().map(|v| v.as_u64().unwrap_or(0) as u32).collect())
                    .unwrap_or_default();
                total_completion_tokens += new_tokens.len() as u32;
                let finish_reason = output.get(5);

                let content: String = Python::with_gil(|py| {
                    let renderer = renderer_clone.bind(py);
                    let tokenizer = renderer.getattr("renderer").and_then(|r| r.getattr("tokenizer")).unwrap();
                    tokenizer.call_method1("decode", (new_tokens,))
                        .and_then(|res| res.extract())
                        .unwrap_or_else(|_| "".into())
                });

                let is_final = finish_reason.is_some() && !finish_reason.unwrap().is_null();

                let chunk = ChatCompletionChunk {
                    id: req_id.to_string(),
                    object: "chat.completion.chunk".into(),
                    created: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    model: model.clone(),
                    choices: vec![ChatChunkChoice {
                        index: 0,
                        delta: ChatMessageDelta { role: None, content: Some(content), tool_calls: None },
                        finish_reason: finish_reason.and_then(|v| if v.is_null() { None } else {
                            match v.as_u64() {
                                Some(0) => Some("stop".into()),
                                Some(1) => Some("length".into()),
                                _ => Some(format!("{}", v)),
                            }
                         }),
                    }],
                    usage: if is_final {
                        Some(UsageInfo {
                            prompt_tokens: prompt_len,
                            completion_tokens: total_completion_tokens,
                            total_tokens: prompt_len + total_completion_tokens,
                        })
                    } else { None },
                };
                yield Ok::<_, Infallible>(Event::default().data(serde_json::to_string(&chunk).unwrap()));
                if is_final { break; }
            }
        };
        Sse::new(stream_res).into_response()
    } else {
        let mut all_token_ids = Vec::new();
        let mut finish_reason_str = None;
        while let Some(output) = rx.recv().await {
            let new_tokens: Vec<u32> = output
                .get(1)
                .and_then(|v| v.as_array())
                .map(|a| a.iter().map(|v| v.as_u64().unwrap_or(0) as u32).collect())
                .unwrap_or_default();
            all_token_ids.extend(new_tokens);
            let finish_reason = output.get(5);
            if finish_reason.is_some() && !finish_reason.unwrap().is_null() {
                finish_reason_str = match finish_reason.unwrap().as_u64() {
                    Some(0) => Some("stop".into()),
                    Some(1) => Some("length".into()),
                    _ => Some(format!("{}", finish_reason.unwrap())),
                };
                break;
            }
        }
        let full_content: String = Python::with_gil(|py| {
            let renderer = state.renderer.bind(py);
            let tokenizer = renderer.getattr("renderer").and_then(|r| r.getattr("tokenizer")).unwrap();
            tokenizer
                .call_method1("decode", (all_token_ids.clone(),))
                .and_then(|res| res.extract())
                .unwrap_or_else(|_| "".into())
        });
        let completion_tokens = all_token_ids.len() as u32;
        Json(ChatCompletionResponse {
            id: request_id,
            object: "chat.completion".to_string(),
            created: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            model: model,
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: Some(serde_json::Value::String(full_content)),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                finish_reason: finish_reason_str,
                logprobs: None,
            }],
            usage: UsageInfo {
                prompt_tokens: prompt_len,
                completion_tokens,
                total_tokens: prompt_len + completion_tokens,
            },
        })
        .into_response()
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/completions", post(completions))
        .route("/v1/embeddings", post(embeddings))
        .with_state(state)
}

async fn run_output_loop(
    mut socket: PullSocket,
    request_streams: Arc<DashMap<String, mpsc::UnboundedSender<serde_json::Value>>>,
) {
    loop {
        match socket.recv().await {
            Ok(msg) => {
                if let Some(data) = msg.get(0) {
                    if let Ok(outputs) = rmp_serde::from_slice::<Vec<serde_json::Value>>(data) {
                        if outputs.len() > 5 && !outputs[5].is_null() {
                            if let Some(finished) = outputs[5].as_array() {
                                for req_id_val in finished {
                                    if let Some(req_id) = req_id_val.as_str() {
                                        debug!("Request finished: {}", req_id);
                                        request_streams.remove(req_id);
                                    }
                                }
                            }
                        }

                        if outputs.len() > 1 {
                            if let Some(out_list) = outputs[1].as_array() {
                                for output in out_list {
                                    if let Some(req_id) = output.get(0).and_then(|v| v.as_str()) {
                                        if let Some(tx) = request_streams.get(req_id) {
                                            let _ = tx.send(output.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("ZMQ Recv error in output loop: {:?}", e);
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }
}

async fn run_input_loop(
    input_socket_arc: Arc<Mutex<RouterSocket>>,
    engine_identities: Arc<Mutex<Vec<bytes::Bytes>>>,
) {
    loop {
        let msg_res = {
            let mut socket = input_socket_arc.lock().await;
            socket.recv().await
        };
        if let Ok(msg) = msg_res {
            debug!("Router received ZMQ message: {:?}", msg);
            let identity = msg.get(0).cloned().unwrap();
            if msg.len() <= 2 {
                info!("Registering engine identity: {:?}", identity);
                let mut idents = engine_identities.lock().await;
                if !idents.contains(&identity) {
                    idents.push(identity);
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

pub fn start_rust_server_internal(
    host: String,
    port: u16,
    input_address: String,
    output_address: String,
    renderer: PyObject,
) -> PyResult<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async move {
            let mut input_socket = RouterSocket::new();
            input_socket
                .bind(&input_address)
                .await
                .expect("Failed to bind input socket");

            let mut output_socket = PullSocket::new();
            output_socket
                .bind(&output_address)
                .await
                .expect("Failed to bind output socket");

            let request_streams = Arc::new(DashMap::new());
            let engine_identities = Arc::new(Mutex::new(Vec::new()));
            let input_socket_arc = Arc::new(Mutex::new(input_socket));

            let state = AppState {
                input_socket: input_socket_arc.clone(),
                engine_identities: engine_identities.clone(),
                request_streams: request_streams.clone(),
                renderer: Arc::new(renderer),
            };

            tokio::spawn(run_input_loop(input_socket_arc, engine_identities));
            tokio::spawn(run_output_loop(output_socket, request_streams));

            let addr: SocketAddr = format!("{}:{}", host, port).parse().expect("Invalid address");
            info!("vLLM Rust Router listening on {}", addr);
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

            // Setup Prometheus metrics
            let (prometheus_layer, metric_handle) = axum_prometheus::PrometheusMetricLayer::pair();
            let app = app(state)
                .route("/metrics", get(|| async move { metric_handle.render() }))
                .layer(prometheus_layer);

            axum::serve(listener, app).await.unwrap();
        });
    Ok(())
}

#[pyfunction]
pub fn start_rust_server(
    host: String,
    port: u16,
    input_address: String,
    output_address: String,
    renderer: PyObject,
) -> PyResult<()> {
    start_rust_server_internal(host, port, input_address, output_address, renderer)
}

#[pymodule]
fn _router(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(start_rust_server, m)?)?;
    Ok(())
}
