variable "release_name" {
  type = string
}

variable "chart_name" {
  type = string
}

variable "repository" {
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

variable "inputs" {
  type = list(object({name=string, value=string}))
  default = []
}