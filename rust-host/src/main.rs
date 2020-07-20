mod abi;
mod utils;

use kube::Config;
use std::cell::RefCell;
use wasmer_runtime::{compile_with, error};
use wasmer_singlepass_backend::SinglePassCompiler;

use crate::abi::Abi;

// Make sure that the compiled wasm-sample-app is accessible at this path.
static WASM: &'static [u8] = include_bytes!("../http.wasm");

fn main() -> error::Result<()> {
    std::env::set_var("RUST_LOG", "info,kube=debug");
    env_logger::init();

    let mut runtime = tokio::runtime::Runtime::new().expect("Cannot create a tokio runtime");

    let kubeconfig = runtime
        .block_on(Config::infer())
        .expect("Cannot infer the kubeconfig");
    let cluster_url = kubeconfig.cluster_url.clone();

    let ref_cell_runtime = RefCell::new(runtime);

    let client = reqwest::ClientBuilder::from(kubeconfig)
        .build()
        .expect("Cannot build the http client from the kubeconfig");

    let (module, duration) = execution_time!({
        compile_with(WASM, &SinglePassCompiler::new()).expect("wasm compilation")
    });
    println!("Compilation time duration: {} ms", duration.as_millis());

    // get the version of the WASI module in a non-strict way, meaning we're
    // allowed to have extra imports
    let wasi_version = wasmer_wasi::get_wasi_version(&module, false)
        .expect("WASI version detected from Wasm module");

    // Resolve abi
    let abi = abi::AbiVersion::RustV1Alpha1.get_abi();

    // WASI imports
    let mut base_imports = wasmer_wasi::generate_import_object_for_version(
        wasi_version,
        vec![],
        vec![],
        vec![],
        vec![],
    );

    base_imports.extend(abi.generate_imports(cluster_url, ref_cell_runtime, client));

    // Compile our webassembly into an `Instance`.
    let instance = module
        .instantiate(&base_imports)
        .expect("Failed to instantiate wasm module");

    abi.start_controller(&instance);

    Ok(())
}
