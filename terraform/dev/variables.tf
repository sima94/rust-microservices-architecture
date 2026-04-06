variable "region" {
  description = "OCI region (e.g. eu-frankfurt-1)"
  type        = string
  default     = "eu-frankfurt-1"
}

variable "compartment_id" {
  description = "OCID of your Compartment where resources are created"
  type        = string
  # Set via TF_VAR_compartment_id or terraform.tfvars (not committed)
  # default = "ocid1.tenancy.oc1..your-tenancy-ocid"
}

variable "cluster_name" {
  description = "Name of the Kubernetes cluster"
  type        = string
  default     = "dev-oke-cluster"
}

variable "kubernetes_version" {
  description = "Kubernetes version - check Oracle Cloud for the latest supported version"
  type        = string
  default     = "v1.32.10"
}

variable "node_shape" {
  description = "OKE worker node shape"
  type        = string
  default     = "VM.Standard.A1.Flex"
}

variable "node_pool_size" {
  description = "Number of worker nodes in the dev node pool"
  type        = number
  default     = 1
}

variable "node_pool_ocpus" {
  description = "OCPU allocation per worker node"
  type        = number
  default     = 1
}

variable "node_pool_memory_gbs" {
  description = "Memory (GB) allocation per worker node"
  type        = number
  default     = 8
}

variable "node_pool_ad_index" {
  description = "Availability Domain index for worker placement (0-based)"
  type        = number
  default     = 0
}
