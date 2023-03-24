variable "release_name" {
  type = string
}

variable "chart_name" {
  type = string
}

variable "chart_version" {
  type = string
  default = ""
}

variable "namespace" {
  type = string
}

variable "values" {
  type = list(string)
  default = [""]
}

variable "repository" {
  type = string
  default = null
}

variable "timeout" {
  type =  number
  default = 300
}

variable "cleanup_on_fail" {
  type = bool
  default = true
}

variable "wait" {
  type = bool
  default = true
}

variable "wait_for_jobs" {
  type = bool
  default = true
}

variable "inputs" {
  type = list(object({name=string, value=string}))
}