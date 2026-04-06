FROM postgres:15

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        python3-pip \
        python3-psycopg2 \
        python3-yaml \
    && pip3 install --break-system-packages patroni[etcd3] \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /home/postgres/data \
    && chown -R postgres:postgres /home/postgres

COPY entrypoint.sh /entrypoint.sh
COPY post-init-user.sh /post-init-user.sh
COPY post-init-auth.sh /post-init-auth.sh
RUN chmod +x /entrypoint.sh /post-init-user.sh /post-init-auth.sh

USER postgres

EXPOSE 5432 8008

ENTRYPOINT ["/entrypoint.sh"]
