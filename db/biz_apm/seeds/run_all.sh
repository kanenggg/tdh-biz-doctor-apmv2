#!/bin/bash

# Shortcut script to run all seed concerns
# Usage: ./run_all.sh

SEED_DIR="$(dirname "$0")"
exec "$SEED_DIR/run.sh"
