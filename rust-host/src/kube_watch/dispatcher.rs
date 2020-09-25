use crate::kube_watch::WatchEvent;
use crate::modules::ControllerModule;
use std::collections::HashMap;
use tokio::sync::mpsc::Receiver;

pub struct Dispatcher {
    map: HashMap<String, ControllerModule>,
}

impl Dispatcher {
    pub fn new(map: HashMap<String, ControllerModule>) -> Dispatcher {
        Dispatcher { map }
    }

    pub async fn start(self, mut rx: Receiver<WatchEvent>) -> anyhow::Result<()> {
        info!("Starting the watch events listener loop");

        while let Some(event) = rx.recv().await {
            if let Some(controller) = self.map.get(&event.controller_name) {
                controller.on_event(event.watch_id, event.event)?
            } else {
                return Err(anyhow::anyhow!(
                    "Cannot find controller for event {:?}",
                    event
                ));
            }
        }
        Ok(())
    }
}
