resource "helm_release" "release" {
  name = var.release_name
  chart = var.chart_name
  repository = var.repository
  version = var.chart_version
  namespace = var.namespace
  values = var.values
  timeout = var.timeout
  cleanup_on_fail = var.cleanup_on_fail
  wait = var.wait
  wait_for_jobs = var.wait_for_jobs
  create_namespace = true

  dynamic "set" {
    for_each = var.inputs
    iterator = input
    content {
      name = input.value["name"]
      value = input.value["value"]
    }
  }
}
  