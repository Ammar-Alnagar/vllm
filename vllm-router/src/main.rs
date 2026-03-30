use clap::Parser;
use vllm_router::start_rust_server_internal;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 8000)]
    port: u16,

    #[arg(long)]
    input_address: String,

    #[arg(long)]
    output_address: String,

    #[arg(long)]
    model_config_pickle: String,
}

fn main() -> PyResult<()> {
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Initialize Python interpreter for PyO3
    pyo3::prepare_freethreaded_python();

    Python::with_gil(|py| {
        // Load the cli args
        let pickle = py.import_bound("pickle")?;
        let bytes = std::fs::read(&args.model_config_pickle)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;
        let cli_args = pickle.call_method1("loads", (PyBytes::new_bound(py, &bytes),))?;

        // Get the model config from args
        let engine_args_cls = py.import_bound("vllm.engine.arg_utils")?.getattr("AsyncEngineArgs")?;
        let engine_args = engine_args_cls.call_method1("from_cli_args", (cli_args.clone(),))?;
        let usage_context = py.import_bound("vllm.usage.usage_lib")?.getattr("UsageContext")?.getattr("OPENAI_API_SERVER")?;
        let vllm_config = engine_args.call_method1("create_engine_config", (usage_context,))?;

        // Get model_config
        let model_config = vllm_config.getattr("model_config")?;
        let resolved_chat_template = py.import_bound("vllm.entrypoints.chat_utils")?.call_method1("load_chat_template", (cli_args.getattr("chat_template")?,))?;

        // Initialize components for OpenAIServingRender
        let renderer_from_config = py.import_bound("vllm.renderers")?.getattr("renderer_from_config")?;
        let renderer = renderer_from_config.call1((vllm_config.clone(),))?;

        let get_io_processor = py.import_bound("vllm.plugins.io_processors")?.getattr("get_io_processor")?;
        let io_processor = get_io_processor.call1((vllm_config.clone(), renderer.clone(), model_config.getattr("io_processor_plugin")?))?;

        let model_registry_cls = py.import_bound("vllm.entrypoints.openai.models.serving")?.getattr("OpenAIModelRegistry")?;
        let served_model_names = cli_args.getattr("served_model_name")?;
        let model_path = cli_args.getattr("model")?;

        let base_model_paths = py.import_bound("vllm.entrypoints.openai.models.protocol")?.getattr("BaseModelPath")?;
        let mut paths = Vec::new();
        if served_model_names.is_none() {
            paths.push(base_model_paths.call1((model_path.clone(), model_path.clone()))?);
        } else {
             for name in served_model_names.iter()? {
                 paths.push(base_model_paths.call1((name?, model_path.clone()))?);
             }
        }

        let model_registry = model_registry_cls.call1((model_config.clone(), paths))?;

        let serving_render_cls = py.import_bound("vllm.entrypoints.serve.render.serving")?.getattr("OpenAIServingRender")?;

        let kwargs = pyo3::types::PyDict::new_bound(py);
        kwargs.set_item("request_logger", py.None())?;
        kwargs.set_item("chat_template", resolved_chat_template)?;
        kwargs.set_item("chat_template_content_format", cli_args.getattr("chat_template_content_format")?)?;
        kwargs.set_item("trust_request_chat_template", cli_args.getattr("trust_request_chat_template")?)?;
        kwargs.set_item("enable_auto_tools", cli_args.getattr("enable_auto_tool_choice")?)?;
        kwargs.set_item("exclude_tools_when_tool_choice_none", cli_args.getattr("exclude_tools_when_tool_choice_none")?)?;
        kwargs.set_item("tool_parser", cli_args.getattr("tool_call_parser")?)?;
        kwargs.set_item("default_chat_template_kwargs", cli_args.getattr("default_chat_template_kwargs")?)?;
        kwargs.set_item("log_error_stack", cli_args.getattr("log_error_stack")?)?;

        let renderer_obj = serving_render_cls.call((
            model_config,
            renderer,
            io_processor,
            model_registry,
        ), Some(&kwargs))?;

        // Start the server
        start_rust_server_internal(
            args.host,
            args.port,
            args.input_address,
            args.output_address,
            renderer_obj.to_object(py),
        )
    })
}
