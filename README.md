# Statusplattform NAIS (K8s) Operator

This application is a webserver which leverages the Rust ecosystem's
[kube-rs](https://kube.rs/) to act as a K8s operator inside the NAIS
clusters. At time of writing, it:
1. It reactively receives events on `EndpointSlices` by the K8s API
1. Ignores all events on EndpointSlice(s) we can't map to a Service
   owned by a NAIS Application
1. It checks if both (a) it contains pod IPs and (b) a minimum of 1x
   of them show 'Ready' readiness status
1. Send HTTP request w/NAIS app's readiness status to the
   statusplattform backend

## Development enviroment
Mandatory:
1. Rust

Optional (w/benefits):
1. Nix (flaked)


## How to deploy
All branches get deployed to dev.

Only commits that have a tag that starts with the letter v (as in
v1.0.0, very-cool-tag[!] or, genereally vx.y.z) get deployed to prod.

Images are tagged as $CARGOVERSION-$REVCOUNT-$SHORTHASH where
- CARGOVERSION is the package version from Cargo.toml
- REVCOUNT is the current git depth and
- SHORTHASH is the eight first characters of the commit sha
