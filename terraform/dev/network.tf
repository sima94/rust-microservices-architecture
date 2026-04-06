resource "oci_core_vcn" "dev_vcn" {
  compartment_id = var.compartment_id
  display_name   = "dev-oke-vcn"
  cidr_block     = "10.0.0.0/16"
}

resource "oci_core_internet_gateway" "dev_ig" {
  compartment_id = var.compartment_id
  display_name   = "dev-oke-ig"
  vcn_id         = oci_core_vcn.dev_vcn.id
}

resource "oci_core_nat_gateway" "dev_nat" {
  compartment_id = var.compartment_id
  display_name   = "dev-oke-nat"
  vcn_id         = oci_core_vcn.dev_vcn.id
}

resource "oci_core_route_table" "public_rt" {
  compartment_id = var.compartment_id
  vcn_id         = oci_core_vcn.dev_vcn.id
  display_name   = "dev-public-rt"
  route_rules {
    destination       = "0.0.0.0/0"
    destination_type  = "CIDR_BLOCK"
    network_entity_id = oci_core_internet_gateway.dev_ig.id
  }
}

resource "oci_core_route_table" "private_rt" {
  compartment_id = var.compartment_id
  vcn_id         = oci_core_vcn.dev_vcn.id
  display_name   = "dev-private-rt"
  route_rules {
    destination       = "0.0.0.0/0"
    destination_type  = "CIDR_BLOCK"
    network_entity_id = oci_core_nat_gateway.dev_nat.id
  }
}

# For development purposes, we open all traffic. 
# For production, strict rules are used (only 443, Kubelet ports etc.)
resource "oci_core_security_list" "dev_sl" {
  compartment_id = var.compartment_id
  vcn_id         = oci_core_vcn.dev_vcn.id
  display_name   = "dev-oke-sl"

  egress_security_rules {
    destination = "0.0.0.0/0"
    protocol    = "all"
  }

  ingress_security_rules {
    source   = "0.0.0.0/0"
    protocol = "all"
  }
}

# Public subnet for Ingress Controller (Load Balancer) and Kubernetes API
resource "oci_core_subnet" "public_subnet" {
  compartment_id    = var.compartment_id
  vcn_id            = oci_core_vcn.dev_vcn.id
  cidr_block        = "10.0.1.0/24"
  display_name      = "dev-public-subnet"
  route_table_id    = oci_core_route_table.public_rt.id
  security_list_ids = [oci_core_security_list.dev_sl.id]
}

# Private subnet for Kubernetes Worker Nodes (Node Pool)
resource "oci_core_subnet" "private_subnet" {
  compartment_id             = var.compartment_id
  vcn_id                     = oci_core_vcn.dev_vcn.id
  cidr_block                 = "10.0.2.0/24"
  display_name               = "dev-private-subnet"
  prohibit_public_ip_on_vnic = true
  route_table_id             = oci_core_route_table.private_rt.id
  security_list_ids          = [oci_core_security_list.dev_sl.id]
}
