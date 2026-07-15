--sqlfluff:dialect:postgres

SET foo.app_pwd = :'DB__USERS__APP_USER__PASSWORD';

DO $$
DECLARE
    raw_password text := current_setting('foo.app_pwd');
BEGIN
   IF EXISTS (
      SELECT FROM pg_catalog.pg_roles
      WHERE  rolname = 'u1') THEN
      RAISE NOTICE 'Role "u1" already exists. Skipping.';
   ELSE
    EXECUTE format('CREATE ROLE u1 LOGIN PASSWORD %L', current_setting('foo.app_pwd'));
   END IF;
END
$$;
