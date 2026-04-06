apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: __APP_NAMESPACE__-user-service
  namespace: __APP_NAMESPACE__
  labels:
    app.kubernetes.io/part-of: rust-microservices
spec:
  selector:
    matchLabels:
      app.kubernetes.io/instance: __USER_SERVICE_NAME__
      app.kubernetes.io/name: __USER_SERVICE_NAME__
  namespaceSelector:
    matchNames:
      - __APP_NAMESPACE__
  endpoints:
    - port: http
      path: /metrics
      interval: __SCRAPE_INTERVAL__
      scrapeTimeout: __SCRAPE_TIMEOUT__
---
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: __APP_NAMESPACE__-auth-service
  namespace: __APP_NAMESPACE__
  labels:
    app.kubernetes.io/part-of: rust-microservices
spec:
  selector:
    matchLabels:
      app.kubernetes.io/instance: __AUTH_SERVICE_NAME__
      app.kubernetes.io/name: __AUTH_SERVICE_NAME__
  namespaceSelector:
    matchNames:
      - __APP_NAMESPACE__
  endpoints:
    - port: http
      path: /metrics
      interval: __SCRAPE_INTERVAL__
      scrapeTimeout: __SCRAPE_TIMEOUT__
---
apiVersion: monitoring.coreos.com/v1
kind: PrometheusRule
metadata:
  name: __APP_NAMESPACE__-microservices-alerts
  namespace: __APP_NAMESPACE__
  labels:
    app.kubernetes.io/part-of: rust-microservices
spec:
  groups:
    - name: rust-microservices-availability
      rules:
        - alert: MicroserviceTargetDown
          expr: up{namespace="__APP_NAMESPACE__",service=~"__USER_SERVICE_NAME__|__AUTH_SERVICE_NAME__"} == 0
          for: 2m
          labels:
            severity: critical
            environment: __APP_NAMESPACE__
          annotations:
            summary: "Service target down"
            description: "One of auth/user scrape targets is down for more than 2 minutes."
        - alert: MicroserviceHighPodRestarts
          expr: increase(kube_pod_container_status_restarts_total{namespace="__APP_NAMESPACE__",container=~"__USER_SERVICE_NAME__|__AUTH_SERVICE_NAME__"}[15m]) > 3
          for: 5m
          labels:
            severity: warning
            environment: __APP_NAMESPACE__
          annotations:
            summary: "High pod restart rate"
            description: "auth/user pods restarted more than 3 times in 15 minutes."
    - name: rust-microservices-http
      rules:
        - alert: MicroserviceHigh5xxRate
          expr: |
            (
              sum(rate({__name__=~".*_http_requests_total",namespace="__APP_NAMESPACE__",service=~"__USER_SERVICE_NAME__|__AUTH_SERVICE_NAME__",status=~"5.."}[5m]))
              /
              clamp_min(sum(rate({__name__=~".*_http_requests_total",namespace="__APP_NAMESPACE__",service=~"__USER_SERVICE_NAME__|__AUTH_SERVICE_NAME__"}[5m])), 1)
            ) > 0.05
          for: 10m
          labels:
            severity: warning
            environment: __APP_NAMESPACE__
          annotations:
            summary: "High 5xx error rate"
            description: "HTTP 5xx rate is above 5% in auth/user services for 10 minutes."
        - alert: MicroserviceHighP95Latency
          expr: |
            histogram_quantile(
              0.95,
              sum(rate({__name__=~".*_http_requests_duration_seconds_bucket",namespace="__APP_NAMESPACE__",service=~"__USER_SERVICE_NAME__|__AUTH_SERVICE_NAME__"}[5m])) by (le, service)
            ) > 0.5
          for: 10m
          labels:
            severity: warning
            environment: __APP_NAMESPACE__
          annotations:
            summary: "High p95 latency"
            description: "P95 latency is above 500ms for auth/user services for 10 minutes."
