{
  lib,
  teamName,
  pname,
  imageName,
  ...
}:
let
  statusplattformNaisOperator = {
    apiVersion = "nais.io/v1alpha1";
    kind = "Application";
    metadata = {
      name = pname;
      namespace = teamName;
      labels = {
        team = teamName;
      };
    };
    spec = {
      envFrom = [ { secret = "swagger-api-konfig"; } ];
      env = lib.attrsToList {
        RUST_BACKTRACE = "full";
        RUST_LOG = "kube=debug";
        COLORBT_SHOW_HIDDEN = "1";
      };
      image = "europe-north1-docker.pkg.dev/nais-management-233d/${teamName}/${imageName}";

      observability = {
        logging.destinations = {
          id = loki;
        };
        tracing.enabled = true;
      };
      port = 8080;
      replicas = {
        min = 1;
        max = 1;
      };
      accessPolicy = {
        outbound = {
          rules = [ { application = "portalserver"; } ];
        };
      };
    };
  };

  allowApiserverAndDns = {
    apiVersion = "networking.k8s.io/v1";
    kind = "NetworkPolicy";
    metadata = {
      name = "allow-apiserver-and-dns";
      namespace = teamName;
    };
    spec = {
      egress = [
        {
          ports = [
            {
              port = 443;
              protocol = "TCP";
            }
          ];
          to = [ { ipBlock.cidr = "172.16.0.13/32"; } ];
        }
        {
          ports = [
            {
              port = 988;
              protocol = "TCP";
            }
          ];
          to = [ { ipBlock.cidr = "169.254.169.252/32"; } ];
        }
        {
          ports = [
            {
              port = 53;
              protocol = "UDP";
            }
            {
              port = 53;
              protocol = "TCP";
            }
          ];
          to = [
            {
              namespaceSelector.matchLabels = {
                "kubernetes.io/metadata.name" = "nais-system";
              };
              podSelector.matchLabels = {
                "k8s-app" = "kube-dns";
              };
            }
            {
              namespaceSelector.matchLabels = {
                "kubernetes.io/metadata.name" = "nais-system";
              };
              podSelector.matchLabels = {
                "k8s-app" = "node-local-dns";
              };
            }
            { ipBlock.cidr = "192.168.64.10/32"; }
          ];
        }
      ];
      podSelector.matchLabels = {
        app = pname;
      };
      policyTypes = [ "Egress" ];
    };
  };
in
[
  statusplattformNaisOperator
  allowApiserverAndDns
]
