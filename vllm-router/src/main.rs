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
        // Load the model config and create the renderer
        let pickle = py.import_bound("pickle")?;
        let bytes = std::fs::read(&args.model_config_pickle)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;
        let model_config = pickle.call_method1("loads", (PyBytes::new_bound(py, &bytes),))?;

        let vllm_entrypoints = py.import_bound("vllm.entrypoints.openai.api_server")?;
        let renderer = vllm_entrypoints.call_method1("Renderer", (model_config,))?;

        let renderer_obj = renderer.to_object(py);

        // Start the server
        start_rust_server_internal(
            args.host,
            args.port,
            args.input_address,
            args.output_address,
            renderer_obj,
        )
    })
}
