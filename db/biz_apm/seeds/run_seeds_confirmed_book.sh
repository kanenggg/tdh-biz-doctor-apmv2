#!/bin/bash

# Quick script to run seeds for local development
# Usage: ./run_seeds_local.sh

set -e

DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_NAME="${DB_NAME:-biz_apm}"
DB_USER="${DB_USER:-biz_apm_admin}"
DB_PASSWORD="${DB_PASSWORD:-password}"

export PGPASSWORD="$DB_PASSWORD"

SEED_DIR="$(dirname "$0")"

echo "Running consultation seeds..."
echo "Connecting to: $DB_USER@$DB_HOST:$DB_PORT/$DB_NAME"

# Check if connection is available
if ! psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c '\q' 2>/dev/null; then
  echo "Error: Cannot connect to PostgreSQL database"
  echo "Make sure PostgreSQL is running and accessible"
  exit 1
fi

# Run all seed files
echo ""
echo "=========================================="
echo "Running: $(basename "$seed_file")"
echo "=========================================="
psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -f "$SEED_DIR/900_seed_confirmed_appointmet.sql"

echo ""
echo "=========================================="
echo "✓ Seeds completed successfully!"
echo "=========================================="
echo ""
