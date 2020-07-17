use k8s_openapi::api::core::v1::Pod;
use kube::api::{ListParams, Meta, PostParams, WatchEvent};
use kube::runtime::Informer;
use kube::{Api, Client};

use kube_derive::CustomResource;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug)]
#[kube(group = "slinky.dev", version = "v1", namespaced)]
pub struct SimplePodSpec {
    image: String,
}

// TODO: Add status?

#[no_mangle]
pub extern "C" fn run() {
    let client = Client::default();

    let foos: Api<SimplePod> = Api::namespaced(client.clone(), "default");
    let inform = Informer::new(foos).params(ListParams::default().timeout(1));

    let pods: Api<Pod> = Api::namespaced(client.clone(), "default");

    loop {
        let events = inform.poll().expect("poll error");

        for event in events {
            let e = event.expect("event error");
            match e {
                WatchEvent::Added(o) => {
                    reconcile_pod(&pods, &o.name(), &o.spec.image).expect("reconcile error");
                }
                WatchEvent::Modified(o) => {
                    reconcile_pod(&pods, &o.name(), &o.spec.image).expect("reconcile error");
                }
                WatchEvent::Error(e) => {
                    println!("Error event: {:?}", e);
                }
                _ => {}
            }
        }
    }
}

fn reconcile_pod(pods: &Api<Pod>, name: &str, image: &str) -> Result<Pod, kube::Error> {
    match pods.get(&name) {
        Ok(mut existing) => {
            let existing_image = existing
                .spec
                .as_ref()
                .map(|spec| spec.containers[0].image.as_ref())
                .flatten()
                .expect("this should never happen");
            if existing_image == image {
                println!("image is equal, doing nothing");
                Ok(existing)
            } else {
                let mut spec = existing.spec.unwrap();
                spec.containers[0].image = Some(image.to_string());
                existing.spec = Some(spec);
                println!("replacing pod");
                pods.replace(&existing.name(), &PostParams::default(), &existing)
            }
        }
        Err(kube::Error::Api(ae)) if ae.code == 404 => {
            println!("creating pod");
            pods.create(&PostParams::default(), &pod(name, image))
        }
        e => e,
    }
}

fn pod(name: &str, image: &str) -> Pod {
    // TODO: Add ownerRef for deletion handling.
    return serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": name },
        "spec": {
            "containers": [{
              "name": "default-container",
              "image": image
            }],
        }
    }))
    .unwrap();
}
