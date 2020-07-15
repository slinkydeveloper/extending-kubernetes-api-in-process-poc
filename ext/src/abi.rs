// This program will be compiled to WebAssembly using a Custom ABI
// We will then use Wasmer embedded to run this module.
use std::{str, mem};
use std::ffi::c_void;
use safe_transmute::{transmute_to_bytes, transmute_one};
use serde::{Serialize, Deserialize};

// Define the functions that this module will use from the outside world.
// In general, the set of this functions is what we define as an ABI.
// Here we define the "customabi" namespace for the imports,
// Otherwise it will be "env" by default
#[link(wasm_import_module = "http-proxy-abi")]
extern "C" {
    fn request(ptr: *const u8, len: usize, allocator_fn: u32) -> u64;
}

#[no_mangle]
pub extern "C" fn allocate(size: usize) -> *mut c_void {
    let mut buffer = Vec::with_capacity(size);
    let pointer = buffer.as_mut_ptr();
    // Say to compiler to forget about this memory cell
    // Deallocation will be done by who's going to consume this allocation
    mem::forget(buffer);

    pointer as *mut c_void
}

/// Struct to pass a pointer and its size to/from the host
#[repr(C)]
#[derive(Clone, Copy)]
struct Ptr {
    ptr: u32,
    size: u32,
}
unsafe impl safe_transmute::TriviallyTransmutable for Ptr {}

impl From<u64> for Ptr {
    fn from(value: u64) -> Self {
        transmute_one(transmute_to_bytes(&[value]))
            .unwrap()
    }
}

// Hack to serialize/deserialize http request
#[derive(Serialize, Deserialize)]
struct HttpRequest {
    #[serde(with = "http_serde::method")]
    method: http::Method,

    #[serde(with = "http_serde::uri")]
    uri: http::Uri,

    #[serde(with = "http_serde::header_map")]
    headers: http::HeaderMap,

    body: Vec<u8>
}

impl From<http::Request<Vec<u8>>> for HttpRequest {
    fn from(req: http::Request<Vec<u8>>) -> Self {
        let (parts, body) = req.into_parts();

        HttpRequest {
            method: parts.method,
            uri: parts.uri,
            headers: parts.headers,
            body
        }
    }
}

#[derive(Serialize, Deserialize)]
struct HttpResponse {
    #[serde(with = "http_serde::status_code")]
    status_code: http::StatusCode,

    #[serde(with = "http_serde::header_map")]
    headers: http::HeaderMap,

    body: Vec<u8>
}

impl Into<http::Response<Vec<u8>>> for HttpResponse {
    fn into(self) -> http::Response<Vec<u8>> {
        let mut builder = http::response::Builder::new()
            .status(self.status_code);

        for (h, v) in self.headers.iter() {
            builder = builder.header(h, v);
        }

        builder.body(self.body).unwrap()
    }
}

pub fn execute_request(req: http::Request<Vec<u8>>) -> http::Response<Vec<u8>> {
    let inner_request: HttpRequest = req.into();
    let bytes = bincode::serialize(&inner_request).unwrap();

    let response_ptr: Ptr = unsafe { request(bytes.as_ptr(), bytes.len(), allocate as usize as u32) }
        .into();

    let response_raw = unsafe { Vec::from_raw_parts(response_ptr.ptr as *mut u8, response_ptr.size as usize, response_ptr.size as usize) };
    let response_inner: HttpResponse = bincode::deserialize(&response_raw).unwrap();

    response_inner.into()
}