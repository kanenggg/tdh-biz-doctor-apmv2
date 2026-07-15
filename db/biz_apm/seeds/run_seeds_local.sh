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
for seed_file in "$SEED_DIR"/001_*.sql; do
	if [ -f "$seed_file" ]; then
		echo ""
		echo "=========================================="
		echo "Running: $(basename "$seed_file")"
		echo "=========================================="
		psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -f "$seed_file"
	fi
done

echo ""
echo "=========================================="
echo "✓ Seeds completed successfully!"
echo "=========================================="
echo ""
echo "Test booking IDs available:"
echo "  - 10001 (INSTANT, CONFIRMED)"
echo "  - 10002 (SCHEDULE, CONSULTATION_DONE)"
echo "  - 10003 (FOLLOW_UP, PENDING)"
echo ""
ecro "Test with:"
echo "  curl -H 'tdh-sec-iam-user-identity: {\"accountId\":1001,\"accountType\":1,\"userProfileId\":2001,\"tenantId\":1,\"oidcUserId\":\"test-user-1\"}' \\"
echo "       http://localhost:8080/v2/appointment/10001"
