use futures::TryStreamExt;
use k8s_openapi::api::core::v1::Service;
use kube::{
    api::{Api, ResourceExt},
    runtime::{watcher, WatchStreamExt},
    Client,
};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> color_eyre::eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let client = Client::try_default().await?;
    // TODO: Exclude nais namespaces
    let api = Api::<Service>::all(client);
    // requires WatchList feature gate on 1.27 or later
    let wc = watcher::Config::default().streaming_lists();

    watcher(api, wc)
        .applied_objects()
        .default_backoff()
        .try_for_each(|s| async move {
            info!("saw {}", s.name_any());
            if let Some(unready_reason) = service_unready(&s) {
                warn!("{}", unready_reason);
            }
            Ok(())
        })
        .await?;
    Ok(())
}

fn service_unready(s: &Service) -> Option<String> {
    let status = s.status.as_ref().unwrap();
    if let Some(conds) = &status.conditions {
        let failed = conds
            .iter()
            .filter(|c| c.type_ == "Ready" && c.status == "False")
            .map(|c| c.message.clone())
            .collect::<Vec<_>>()
            .join(",");
        if !failed.is_empty() {
            return Some(format!("Unready service {}: {}", s.name_any(), failed));
        }
    }
    None
}
