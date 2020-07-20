use std::cell::RefCell;
use wasmer_runtime_core::import::ImportObject;
use wasmer_runtime_core::Instance;

#[cfg(feature = "abi-rust-v1alpha1")]
mod rust_v1alpha1;

pub trait Abi {
    fn generate_imports(
        &self,
        cluster_url: url::Url,
        rt: RefCell<tokio::runtime::Runtime>,
        http_client: reqwest::Client,
    ) -> ImportObject;
    fn start_controller(&self, instance: &Instance);
}

pub enum AbiVersion {
    #[cfg(feature = "abi-rust-v1alpha1")]
    RustV1Alpha1,
}

impl AbiVersion {
    pub fn get_abi(&self) -> impl Abi {
        match self {
            #[cfg(feature = "abi-rust-v1alpha1")]
            AbiVersion::RustV1Alpha1 => rust_v1alpha1::Abi {},
        }
    }
}
