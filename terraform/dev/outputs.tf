output "cluster_id" {
  description = "OCID of the Kubernetes cluster"
  value       = oci_containerengine_cluster.dev_cluster.id
}

output "kubernetes_version" {
  value = oci_containerengine_cluster.dev_cluster.kubernetes_version
}

output "kubeconfig_command" {
  description = "Command to download the Kubeconfig for this cluster"
  value       = "oci ce cluster create-kubeconfig --cluster-id ${oci_containerengine_cluster.dev_cluster.id} --file ~/.kube/config --region ${var.region} --token-version 2.0.0  --kube-endpoint PUBLIC_ENDPOINT"
}
