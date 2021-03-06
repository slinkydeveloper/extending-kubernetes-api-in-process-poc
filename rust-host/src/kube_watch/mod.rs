use http::Request;

use kube::api::ListParams;
use std::convert::TryInto;

mod watchers;
pub use watchers::Watchers;

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct WatchKey {
    pub resource_version: String,
    pub resource: kube::Resource,
    pub list_params: ListParams,
}

impl TryInto<http::Request<Vec<u8>>> for WatchKey {
    type Error = kube::Error;

    fn try_into(self) -> Result<Request<Vec<u8>>, Self::Error> {
        let res = kube::Resource {
            api_version: self.resource.api_version,
            group: self.resource.group,
            kind: self.resource.kind,
            version: self.resource.version,
            namespace: self.resource.namespace,
        };
        let lp = kube::api::ListParams {
            field_selector: self.list_params.field_selector,
            label_selector: self.list_params.label_selector,
            timeout: self.list_params.timeout,
            allow_bookmarks: self.list_params.allow_bookmarks,
            limit: self.list_params.limit,
            continue_token: self.list_params.continue_token,
        };
        res.watch(&lp, &self.resource_version)
    }
}
