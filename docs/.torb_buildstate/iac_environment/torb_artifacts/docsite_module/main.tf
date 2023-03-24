module "createreactapp_helm_release" {
    source = "../common/helm"
    release_name = var.release_name
    chart_name = var.chart_name
    repository = var.repository
    namespace = var.namespace
    values = var.values
    inputs = var.inputs
}