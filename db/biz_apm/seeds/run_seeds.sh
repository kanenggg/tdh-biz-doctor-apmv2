#!/bin/bash

# PostgreSQL seeds runner script

set -e

DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_NAME="${DB_NAME:-biz_apm}"
DB_USER="${DB_USER:-postgres}"
DB_PASSWORD="${DB_PASSWORD:-postgres}"

export PGPASSWORD="$DB_PASSWORD"

echo "Running PostgreSQL seeds..."
echo "Host: $DB_HOST:$DB_PORT"
echo "Database: $DB_NAME"
echo "User: $DB_USER"

# Check if connection is available
until psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c '\q'; do
	echo "PostgreSQL is unavailable - sleeping"
	sleep 1
done

echo "PostgreSQL is up - executing seeds"

# Find and run all seed files in order
SEED_DIR="$(dirname "$0")"
for seed_file in "$SEED_DIR"/*.sql; do
	if [ -f "$seed_file" ]; then
		echo "Executing seed file: $(basename "$seed_file")"
		psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -f "$seed_file"
	fi
done

echo "Seeds completed successfully!"
