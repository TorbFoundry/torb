terraform {
  required_providers {
    torb = {
      "source" = "TorbFoundry/torb"
      "version" = "0.1.2"
    }
  }
}

provider "torb" {}

module "next_app_project_next_app_1" {
  source = "./torb_artifacts/docsite_module"
  release_name = "docsite-docsite"
  namespace = "next-app"
  chart_name = "/Users/crow/.torb/repositories/torb-artifacts/projects/createnextapp/helm/createnextapp"
  inputs = [
    {
      "name" = "app.name"
      "value" = "docsite"
    },
    {
      "name" = "ingress.enabled"
      "value" = "true"
    }
  ]
  values = [
    "---\nimage:\n  tag: newest\n  repository: docker.torblet.tech/docsite\n",
    "---\ningress:\n  hosts:\n    - host: next.torblet.tech\n      paths:\n        - path: /\n          pathType: Prefix\n",
    "---\nimage:\n  pullPolicy: Always\n"
  ]
}

data "torb_helm_release" "docsite_docsite" {
  release_name = "docsite-docsite"
  namespace = "next-app"
  depends_on = [
    module.next_app_project_next_app_1
  ]
}
