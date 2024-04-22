use std::env;

use futures::TryStreamExt;
use k8s_openapi::api::core::v1::Service;
use kube::{
    api::{Api, ResourceExt},
    runtime::{watcher, WatchStreamExt},
    Client,
};
use tracing::{info, warn};

/// Returns true if and only if the Service's `spec.clusterIPs`
///  (as addressed in the YAML spec) is _not_ empty.
fn service_is_ready(s: &Service) -> bool {
    let Some(ref service_spec) = s.spec else {
        return false;
    };
    let Some(ref pod_ip_list) = service_spec.cluster_ips else {
        return false;
    };
    !pod_ip_list.is_empty()
}

#[tokio::main]
async fn main() -> color_eyre::eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let client = Client::try_default().await?;
    let api = Api::<Service>::all(client);
    // requires WatchList feature gate on 1.27 or later: TODO check if cluster supports
    // TODO: Exclude non-app namespaces (like NAIS ones)
    let platform_namespaces = env::var("PLATFORM_NAMESPACES")
        .expect("Env var 'PLATFORM_NAMESPACES' not present. Comma separated list of strings.");
    let excluded_namespaces = platform_namespaces
        .split(",")
        .fold("".to_string(), |result, namespace| {
            format!("{},namespace!={}", result, namespace)
        });
    let wc = watcher::Config::default()
        .labels("app,team") // I just care if the label(s) exist
        .fields(&excluded_namespaces)
        .streaming_lists();

    watcher(api, wc)
        .applied_objects()
        .default_backoff()
        .try_for_each(|s| async move {
            info!("saw {}", s.name_any());
            match service_is_ready(&s) {
                // TODO: Send http request to the statusplattform backend API w/reqwest
                true => {
                    warn!(
                        "{}/{} is alive!!!",
                        s.metadata.namespace.unwrap(),
                        s.metadata.name.unwrap()
                    );
                    // unimplemented!()
                }
                false => {
                    warn!(
                        "{}/{} is dead!!!",
                        s.metadata.namespace.unwrap(),
                        s.metadata.name.unwrap()
                    );
                    // unimplemented!()
                }
            };
            // TODO: Consider/research if the events can be further filtered to only send http request to backend
            Ok(())
        })
        .await?;
    Ok(())
}
