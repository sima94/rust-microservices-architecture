resource "oci_containerengine_cluster" "dev_cluster" {
  compartment_id     = var.compartment_id
  kubernetes_version = var.kubernetes_version
  name               = var.cluster_name
  vcn_id             = oci_core_vcn.dev_vcn.id

  endpoint_config {
    is_public_ip_enabled = true
    subnet_id            = oci_core_subnet.public_subnet.id
  }

  options {
    add_ons {
      is_kubernetes_dashboard_enabled = false
      is_tiller_enabled               = false
    }
    kubernetes_network_config {
      pods_cidr     = "10.244.0.0/16"
      services_cidr = "10.96.0.0/16"
    }
  }
}

# Fetch the list of Availability Domains in the given Compartment
data "oci_identity_availability_domains" "ads" {
  compartment_id = var.compartment_id
}

# Find the latest Oracle Linux 8 image based on ARM (aarch64) for nodes
data "oci_core_images" "oracle_linux" {
  compartment_id           = var.compartment_id
  operating_system         = "Oracle Linux"
  operating_system_version = "8"
  shape                    = var.node_shape
  sort_by                  = "TIMECREATED"
  sort_order               = "DESC"
}

resource "oci_containerengine_node_pool" "dev_node_pool" {
  cluster_id         = oci_containerengine_cluster.dev_cluster.id
  compartment_id     = var.compartment_id
  kubernetes_version = var.kubernetes_version
  name               = "dev-oke-pool"

  node_shape = var.node_shape

  # Keep defaults lightweight for dev to reduce host-capacity failures.
  node_shape_config {
    ocpus         = var.node_pool_ocpus
    memory_in_gbs = var.node_pool_memory_gbs
  }

  node_source_details {
    source_type = "IMAGE"
    image_id    = data.oci_core_images.oracle_linux.images[0].id
  }

  node_config_details {
    placement_configs {
      availability_domain = data.oci_identity_availability_domains.ads.availability_domains[var.node_pool_ad_index].name
      subnet_id           = oci_core_subnet.private_subnet.id
    }
    size = var.node_pool_size
  }
}
