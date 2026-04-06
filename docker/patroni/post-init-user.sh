#!/bin/bash
set -e
psql -U postgres -c "CREATE USER rust_user WITH LOGIN;"
psql -U postgres -c "CREATE DATABASE rust_db OWNER rust_user;"
echo "User DB initialized: rust_user + rust_db"
