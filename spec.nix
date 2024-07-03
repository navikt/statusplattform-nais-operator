image:
let
  statusplattformNaisOperator = {
    apiVersion = "nais.io/v1alpha1";
    kind = "Application";
    metadata = {
      name = "statusplattform-nais-operator";
      namespace = "navdig";
      labels = { team = "navdig"; };
    };
    spec = {
      envFrom = [{ secret = "swagger-api-konfig"; }];
      env = [
        {
          name = "RUST_BACKTRACE";
          value = "full";
        }
        {
          name = "RUST_LOG";
          value = "kube=debug";
        }
        { name = "COLORBT_SHOW_HIDDEN"; }
        {
          name = "COLORBT_SHOW_HIDDEN";
          value = "1";
        }
      ];
      inherit image;
      port = 8080;
      replicas = {
        min = 1;
        max = 1;
      };
      accessPolicy = {
        outbound = { rules = [{ application = "portalserver"; }]; };
      };
    };
  };

  allowApiserverAndDns = {
    apiVersion = "networking.k8s.io/v1";
    kind = "NetworkPolicy";
    metadata = {
      name = "allow-apiserver-and-dns";
      namespace = "navdig";
    };
    spec = {
      egress = [
        {
          ports = [{
            port = 443;
            protocol = "TCP";
          }];
          to = [{ ipBlock = { cidr = "172.16.0.13/32"; }; }];
        }
        {
          ports = [{
            port = 988;
            protocol = "TCP";
          }];
          to = [{ ipBlock = { cidr = "169.254.169.252/32"; }; }];
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
              namespaceSelector = {
                matchLabels = {
                  "kubernetes.io/metadata.name" = "nais-system";
                };
              };
              podSelector = { matchLabels = { "k8s-app" = "kube-dns"; }; };
            }
            {
              namespaceSelector = {
                matchLabels = {
                  "kubernetes.io/metadata.name" = "nais-system";
                };
              };
              podSelector = {
                matchLabels = { "k8s-app" = "node-local-dns"; };
              };
            }
            { ipBlock = { cidr = "192.168.64.10/32"; }; }
          ];
        }
      ];
      podSelector = {
        matchLabels = { app = "statusplattform-nais-operator"; };
      };
      policyTypes = [ "Egress" ];
    };
  };
in [ statusplattformNaisOperator allowApiserverAndDns ]
