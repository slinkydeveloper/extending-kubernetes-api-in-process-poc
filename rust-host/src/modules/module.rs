use super::ControllerModuleMetadata;
use crate::abi::{Abi, AbiConfig};
use wasmtime::*;
use wasmtime_wasi::*;

pub struct ControllerModule {
    meta: ControllerModuleMetadata,
    instance: Instance,
}

impl ControllerModule {
    pub fn compile(
        meta: ControllerModuleMetadata,
        wasm_bytes: Vec<u8>,
        abi_config: AbiConfig,
    ) -> anyhow::Result<ControllerModule> {
        let store = Store::default();
        let mut linker = Linker::new(&store);

        // Link wasi to the linker
        let wasi = Wasi::new(&store, WasiCtx::new(std::env::args())?);
        wasi.add_to_linker(&mut linker)?;

        // Resolve abi
        let abi = meta.abi.get_abi();
        abi.link(&mut linker, &meta.name, abi_config);

        let module = Module::new(store.engine(), &wasm_bytes)?;
        let instance = linker.instantiate(&module)?;

        Ok(ControllerModule { meta, instance })
    }

    pub fn name(&self) -> &str {
        &self.meta.name
    }

    pub fn start(&self) -> anyhow::Result<()> {
        let abi = self.meta.abi.get_abi();
        abi.start_controller(&self.instance)?;
        debug!("start_controller completed '{:?}'", &self.meta);
        Ok(())
    }

    pub fn on_event(&self, event_id: u64, event: Vec<u8>) -> anyhow::Result<()> {
        let abi = self.meta.abi.get_abi();
        Ok(abi.on_event(&self.instance, event_id, event)?)
    }
}

// https://github.com/bytecodealliance/wasmtime/issues/793#issuecomment-692740254
unsafe impl Send for ControllerModule {}
