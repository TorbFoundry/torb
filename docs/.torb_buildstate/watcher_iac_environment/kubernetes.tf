data "external" "kubeconfig_env" {
  program = ["echo", "{\"kubeconfig\": \"$KUBECONFIG\"}"]
}

locals {
  kubeconfig = data.external.kubeconfig_env
}

provider "kubernetes" {
  config_path = coalesce(local.kubeconfig, "~/.kube/config")
}
