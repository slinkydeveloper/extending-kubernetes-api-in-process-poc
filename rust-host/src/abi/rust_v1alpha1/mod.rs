use std::cell::RefCell;
use wasmer_runtime::{func, imports, Func, ImportObject, Instance};

mod data;
mod func;

pub(crate) struct Abi {}

impl super::Abi for Abi {
    fn generate_imports(
        &self,
        cluster_url: url::Url,
        rt: RefCell<tokio::runtime::Runtime>,
        http_client: reqwest::Client,
    ) -> ImportObject {
        imports! {
            "http-proxy-abi" => {
                // the func! macro autodetects the signature
                "request" => func!(func::request_fn(cluster_url, rt, http_client)),
            },
        }
    }

    fn start_controller(&self, instance: &Instance) {
        let run_fn: Func<(), ()> = instance.exports.get("run").unwrap();
        run_fn
            .call()
            .expect("Something went wrong while invoking run");
    }
}
