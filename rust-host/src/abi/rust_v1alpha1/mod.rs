use crate::kube_watch::WatchCommand;

use super::AbiConfig;
use futures::AsyncWriteExt;
use std::cell::RefCell;
use std::io::Write;
use tokio::sync::mpsc::UnboundedSender;
use wasmtime::*;

mod http_data;
mod watch_data;

use bytes::Bytes;
use http::HeaderMap;
use http_data::{HttpRequest, HttpResponse, Ptr};
use std::convert::{TryFrom, TryInto};

use crate::execution_time;
use tokio::runtime::Handle;

use crate::abi::rust_v1alpha1::watch_data::WatchRequest;

pub(crate) struct Abi {}

impl super::Abi for Abi {
    fn link(&self, linker: &mut Linker, controller_name: &str, abi_config: AbiConfig) {
        let ctx = RequestFnCtx {
            cluster_url: abi_config.cluster_url,
            http_client: abi_config.http_client,
        };
        linker.func(
            "http-proxy-abi",
            "request",
            move |caller: Caller, ptr: i32, size: u32, allocator: Option<Func>| {
                ctx.request_impl(caller, ptr, size, allocator)
            },
        );
        let ctx = WatchFnCtx {
            controller_name: controller_name.to_string(),
            watch_command_sender: abi_config.watch_command_sender,
            watch_counter: RefCell::new(0),
        };
        linker.func(
            "kube-watch-abi",
            "watch",
            move |caller: Caller, ptr: i32, size: u32, allocator: Option<Func>| {
                ctx.watch_impl(caller, ptr, size, allocator)
            },
        );
    }

    fn start_controller(&self, instance: &Instance) -> anyhow::Result<()> {
        let run_fn = instance
            .get_func("run")
            .ok_or(anyhow::anyhow!("Cannot find 'run' export"))?
            .get0::<()>()?;
        anyhow::Result::Ok(run_fn()?)
    }

    fn on_event(&self, instance: &Instance, event_id: u64, event: Vec<u8>) -> anyhow::Result<()> {
        let memory_location_size = event.len();
        let memory_location_ptr = self.allocate(instance, memory_location_size as u32)?;

        let mem = instance
            .get_memory("memory")
            .ok_or(anyhow::anyhow!("Cannot find memory in the module instance"))?;

        unsafe {
            let full_memory = mem.data_unchecked_mut();
            let mut our_slice =
                &mut full_memory[memory_location_ptr as usize..memory_location_size];
            our_slice.write(&event);
        }

        let on_event_fn = instance
            .get_func("on_event")
            .ok_or(anyhow::anyhow!("Cannot find 'on_event' export"))?
            .get3::<u64, u32, u32, ()>()?;
        Ok(on_event_fn(
            event_id,
            memory_location_ptr,
            memory_location_size as u32,
        )?)
    }

    fn allocate(&self, instance: &Instance, allocation_size: u32) -> anyhow::Result<u32> {
        let allocate_fn = instance
            .get_func("allocate")
            .ok_or(anyhow::anyhow!("Cannot find 'allocate' export"))?
            .get1::<u32, u32>()?;
        Ok(allocate_fn(allocation_size)?)
    }
}

pub(crate) struct RequestFnCtx {
    cluster_url: url::Url,
    http_client: reqwest::Client,
}

impl RequestFnCtx {
    fn request_impl(
        &self,
        caller: Caller,
        ptr: i32,
        size: u32,
        allocator: Option<Func>,
    ) -> Result<u64, Trap> {
        // Get the memory and the allocator function
        let mem = caller
            .get_export("memory")
            .and_then(|e| e.into_memory())
            .ok_or(Trap::new("failed to find host memory"))?;
        let allocator_fn = allocator
            .ok_or(Trap::new("Cannot find 'allocate' function pointer"))?
            .get1::<u32, u32>()
            .map_err(|e| Trap::from(e))?;

        // Get the request
        let inner_req_bytes = unsafe {
            mem.data_unchecked()
                .get(ptr as u32 as usize..)
                .and_then(|arr| arr.get(..size as u32 as usize))
        }
        .ok_or(Trap::new("The provided pointer and size are incorrect"))?;
        let inner_request: HttpRequest = bincode::deserialize(inner_req_bytes).unwrap();
        let req_uri = inner_request.uri.clone();

        let (inner_response, duration) = execution_time!({ self.execute_request(inner_request) });
        debug!(
            "Request '{}' duration: {} ms",
            req_uri,
            duration.as_millis()
        );

        let inner_res_bytes = bincode::serialize(&inner_response).unwrap();

        // Allocate memory to write the response
        let allocation_size = inner_res_bytes.len() as u32;
        let allocation_ptr = allocator_fn(allocation_size)?;

        // Write response in module memory
        unsafe {
            let full_memory = mem.data_unchecked_mut();
            let mut our_slice = &mut full_memory[allocation_ptr as usize..allocation_size as usize];
            our_slice.write(&inner_res_bytes);
        }

        // Return the packed bytes
        Ok(Ptr {
            ptr: allocation_ptr,
            size: allocation_size,
        }
        .into())
    }

    fn execute_request(&self, mut inner_request: HttpRequest) -> HttpResponse {
        //TODO implement error propagation when requests goes wrong

        // Path request url
        inner_request.uri =
            http::Uri::try_from(self.generate_url(inner_request.uri.path_and_query().unwrap()))
                .expect("Cannot build the final uri");

        let request: http::Request<Vec<u8>> = inner_request.into();
        let response: reqwest::Response = Handle::current()
            .block_on(async {
                Handle::current();
                self.http_client.execute(request.try_into().unwrap()).await
            })
            .unwrap();

        let status_code = response.status();
        let mut headers = HeaderMap::with_capacity(response.headers().len());
        for (k, v) in response.headers().iter() {
            headers.append(k, v.clone());
        }
        let response_body: Bytes = Handle::current()
            .block_on(async { response.bytes().await })
            .unwrap();

        HttpResponse {
            status_code,
            headers,
            body: response_body.to_vec(),
        }
    }

    /// An internal url joiner to deal with the two different interfaces
    ///
    /// - api module produces a http::Uri which we can turn into a PathAndQuery (has a leading slash by construction)
    /// - config module produces a url::Url from user input (sometimes contains path segments)
    ///
    /// This deals with that in a pretty easy way (tested below)
    fn generate_url(&self, request_p_and_q: &http::uri::PathAndQuery) -> String {
        let base = self.cluster_url.as_str().trim_end_matches('/');
        format!("{}{}", base, request_p_and_q)
    }
}

pub(crate) struct WatchFnCtx {
    controller_name: String,

    watch_command_sender: UnboundedSender<WatchCommand>,
    watch_counter: RefCell<u64>,
}

impl WatchFnCtx {
    fn watch_impl(
        &self,
        caller: Caller,
        ptr: i32,
        size: u32,
        _allocator: Option<Func>,
    ) -> Result<u64, Trap> {
        let mem = caller
            .get_export("memory")
            .and_then(|e| e.into_memory())
            .ok_or(Trap::new("failed to find host memory"))?;

        let watch_req_bytes = unsafe {
            mem.data_unchecked()
                .get(ptr as u32 as usize..)
                .and_then(|arr| arr.get(..size as u32 as usize))
        }
        .ok_or(Trap::new("The provided pointer and size are incorrect"))?;

        let watch_request: WatchRequest = bincode::deserialize(watch_req_bytes).unwrap();

        let watch_counter: &mut u64 = &mut self.watch_counter.borrow_mut();
        let this_watch_counter: u64 = *watch_counter;
        *watch_counter += 1;

        self.watch_command_sender
            .send(
                watch_request.into_watch_command(self.controller_name.clone(), this_watch_counter),
            )
            .map_err(|_e| Trap::new("Cannot dispatch watch requests"))?;

        Ok(this_watch_counter)
    }
}
