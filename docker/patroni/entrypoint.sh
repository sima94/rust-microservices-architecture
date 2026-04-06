#!/bin/bash
set -e

PATRONI_REST_CONNECT_ADDRESS="${PATRONI_REST_CONNECT_ADDRESS:-${PATRONI_NAME}:8008}"
PATRONI_PG_CONNECT_ADDRESS="${PATRONI_PG_CONNECT_ADDRESS:-${PATRONI_NAME}:5432}"

cat > /home/postgres/patroni.yml <<EOF
scope: ${PATRONI_SCOPE}
name: ${PATRONI_NAME}

restapi:
  listen: 0.0.0.0:8008
  connect_address: ${PATRONI_REST_CONNECT_ADDRESS}

etcd3:
  hosts: ${ETCD_HOSTS:-etcd:2379}

bootstrap:
  dcs:
    ttl: 30
    loop_wait: 10
    retry_timeout: 10
    maximum_lag_on_failover: 1048576
    postgresql:
      use_pg_rewind: true
      parameters:
        wal_level: replica
        max_wal_senders: 5
        max_replication_slots: 5
        hot_standby: "on"
  initdb:
    - encoding: UTF8
    - data-checksums
  pg_hba:
    - local all all trust
    - host replication replicator all trust
    - host all all all trust
  post_init: ${POST_INIT_SCRIPT:-/post-init-user.sh}

postgresql:
  listen: 0.0.0.0:5432
  connect_address: ${PATRONI_PG_CONNECT_ADDRESS}
  data_dir: /home/postgres/data/patroni
  pgpass: /tmp/pgpass
  authentication:
    superuser:
      username: \${PG_SUPERUSER:-postgres}
      password: \${PG_SUPERUSER_PASSWORD:-postgres}
    replication:
      username: \${PG_REPLICATION_USER:-replicator}
      password: \${PG_REPLICATION_PASSWORD:-replicator_pass}
  parameters:
    unix_socket_directories: /var/run/postgresql
EOF

exec patroni /home/postgres/patroni.yml
