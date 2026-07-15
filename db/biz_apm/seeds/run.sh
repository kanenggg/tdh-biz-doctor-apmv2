#!/bin/bash

# PostgreSQL seeds runner script with concern-based execution
# Usage: ./run.sh [concern]
#   concern: Optional. Run seeds for specific concern only (consultation, appointment, doctor_schedule)
#            If not specified, runs all concerns in order

set -e

DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_NAME="${DB_NAME:-biz_apm}"
DB_USER="${DB_USER:-biz_apm_admin}"
DB_PASSWORD="${DB_PASSWORD:-password}"

export PGPASSWORD="$DB_PASSWORD"

SEED_DIR="$(dirname "$0")"
CONCERN="$1"

echo "Running PostgreSQL seeds..."
echo "Host: $DB_HOST:$DB_PORT"
echo "Database: $DB_NAME"
echo "User: $DB_USER"
echo ""

check_connection() {
	if ! psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c '\q' 2>/dev/null; then
		echo "Error: Cannot connect to PostgreSQL database"
		echo "Make sure PostgreSQL is running and accessible"
		exit 1
	fi
}

run_seeds_for_concern() {
	local concern_dir="$1"
	local concern_name=$(basename "$concern_dir")

	if [ ! -d "$concern_dir" ]; then
		echo "Warning: Concern directory '$concern_dir' does not exist"
		return 1
	fi

	echo "=========================================="
	echo "Running seeds for: $concern_name"
	echo "=========================================="

	local seed_count=0
	for seed_file in "$concern_dir"/*.sql; do
		if [ -f "$seed_file" ]; then
			seed_count=$((seed_count + 1))
			echo "Executing: $(basename "$seed_file")"
			psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -f "$seed_file" -q
			if [ $? -eq 0 ]; then
				echo "  ✓ Success"
			else
				echo "  ✗ Failed"
				return 1
			fi
		fi
	done

	if [ $seed_count -eq 0 ]; then
		echo "No SQL files found in $concern_dir"
	fi

	echo ""
	return 0
}

check_connection

if [ -n "$CONCERN" ]; then
	concern_dir="$SEED_DIR/$CONCERN"
	if [ ! -d "$concern_dir" ]; then
		echo "Error: Concern '$CONCERN' not found"
		echo "Available concerns:"
		for dir in "$SEED_DIR"/*/; do
			if [ -d "$dir" ] && [ "$(basename "$dir")" != ".*" ]; then
				echo "  - $(basename "$dir")"
			fi
		done
		exit 1
	fi

	run_seeds_for_concern "$concern_dir" || exit 1
else
	echo "Running all concerns in order..."
	echo ""

	concerns=("consultation" "appointment" "doctor_schedule")

	for concern in "${concerns[@]}"; do
		concern_dir="$SEED_DIR/$concern"
		if [ -d "$concern_dir" ]; then
			run_seeds_for_concern "$concern_dir" || exit 1
		fi
	done
fi

echo "=========================================="
echo "✓ Seeds completed successfully!"
echo "=========================================="
