#!/bin/bash
set -e
psql -U postgres -c "CREATE USER rust_user WITH LOGIN;"
psql -U postgres -c "CREATE DATABASE auth_db OWNER rust_user;"
echo "Auth DB initialized: rust_user + auth_db"
