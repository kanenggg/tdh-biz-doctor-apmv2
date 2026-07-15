#!/usr/bin/env sh

LOCAL_DIR=$(dirname $0)

# onboard-db.sh --db ${DB_NAME} \
# 	--admin-user "${DB_USER_ADMIN_NAME}" \
# 	--admin-pass "${DB_USER_ADMIN_PASS}" \
# 	--ro-user "${DB_USER_SERVICE_RO_NAME}" \
# 	--ro-pass "${DB_USER_SERVICE_RO_PASS}" \
# 	--rw-user "${DB_USER_SERVICE_RW_NAME}" \
# 	--rw-pass "${DB_USER_SERVICE_RW_PASS}"

# echo "Working $LOCAL_DIR Onboarding complete."
ls $LOCAL_DIR

for file in $LOCAL_DIR/migrations/*.sql; do
  case "$file" in
    *20260713000003__appointment_hold_cutover.sql)
      if [ "${APMV2_HOLD_CUTOVER_READY:-false}" != "true" ]; then
        echo "Skipping gated Hold cutover $file (set APMV2_HOLD_CUTOVER_READY=true after preflight)"
        continue
      fi
      ;;
  esac
  echo "Running $file"
  run-pgsql-script.sh ${DB_NAME} ${DB_ADMIN_USER} "${DB_ADMIN_USER_PASS}" $file
done

# $LOCAL_DIR/onboarding/20260317__grant_schema_in_v2.sql.sh
