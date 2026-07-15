mod common;

/// The compatibility migration must leave old pods able to resolve their exact
/// thirteen-argument SQL entry point while new pods use the canonical Hold name.
#[tokio::test]
async fn fresh_migrations_expose_canonical_hold_and_legacy_reservation_functions() {
    let database = common::setup_test_db().await;

    let functions: Vec<(String, i16)> = sqlx::query_as(
        r#"
        SELECT p.proname, p.pronargs
        FROM pg_proc p
        JOIN pg_namespace n ON n.oid = p.pronamespace
        WHERE n.nspname = 'v2'
          AND p.proname IN ('create_appointment_hold', 'create_reservation')
        ORDER BY p.proname, p.pronargs
        "#,
    )
    .fetch_all(&database.pool)
    .await
    .expect("fresh migration chain should be queryable");

    assert!(
        functions.contains(&("create_appointment_hold".to_string(), 14)),
        "canonical 14-argument Hold function must exist: {functions:?}"
    );
    assert!(
        functions.contains(&("create_reservation".to_string(), 13)),
        "legacy 13-argument Reservation wrapper must remain for rolling old pods: {functions:?}"
    );
}

#[tokio::test]
async fn canonical_and_legacy_functions_enforce_authoritative_hold_invariants() {
    let database = common::setup_test_db().await;
    let pool = &database.pool;
    let doctor_id = uuid::Uuid::new_v4();
    let doctor_account_id = 981_001_i64;
    let doctor_profile_id = 991_001_i64;

    sqlx::query(
        "INSERT INTO v2.doctor_identity (doctor_id, doctor_account_id, doctor_profile_id, is_active) VALUES ($1, $2, $3, true)",
    )
    .bind(doctor_id)
    .bind(doctor_account_id)
    .bind(doctor_profile_id)
    .execute(pool)
    .await
    .expect("test must seed authoritative doctor identity");
    sqlx::query(
        "INSERT INTO v2.doctor_consultation_config (doctor_id, schedule_available, instant_available, schedule_config) VALUES ($1, true, true, '{\"timezone\":\"Asia/Bangkok\",\"specificDate\":[],\"daysOfWeek\":{\"1\":[{\"startTime\":540,\"endTime\":600}]}}'::jsonb)",
    )
    .bind(doctor_id)
    .execute(pool)
    .await
    .expect("test must seed authoritative operational availability");
    sqlx::query(
        "INSERT INTO v2.doctor_service_config_projection (doctor_id, channels, languages, duration_minutes, doctor_fee_amount, doctor_fee_currency, profile_version, source_event_id) VALUES ($1, ARRAY['video'], ARRAY['en'], 15, 0, 'THB', 1, 'hold-migration-test')",
    )
    .bind(doctor_id)
    .execute(pool)
    .await
    .expect("test must seed authoritative projected service configuration");

    let canonical_booking_id: String = sqlx::query_scalar(
        r#"
        SELECT booking_id FROM v2.create_appointment_hold(
            101, 201, 301, $1, $2, 1, 1, 1,
            'Schedule'::v2.booking_type_enum, 'video'::v2.consultation_type_enum,
            '2030-01-07T02:00:00Z'::timestamptz, 900, 900, true
        )
        "#,
    )
    .bind(doctor_account_id as i32)
    .bind(doctor_profile_id as i32)
    .fetch_one(pool)
    .await
    .expect("canonical Hold function should create a scheduled Hold");

    let legacy_booking_id: String = sqlx::query_scalar(
        r#"
        SELECT booking_id FROM v2.create_reservation(
            102, 202, 301, $1, $2, 1, 1, 1,
            'Schedule'::v2.booking_type_enum, 'video'::v2.consultation_type_enum,
            '2030-01-07T02:15:00Z'::timestamptz, 900, 900
        )
        "#,
    )
    .bind(doctor_account_id as i32)
    .bind(doctor_profile_id as i32)
    .fetch_one(pool)
    .await
    .expect("exact legacy thirteen-argument wrapper should create a Hold");
    assert_ne!(canonical_booking_id, legacy_booking_id);

    let overlap = sqlx::query_scalar::<_, String>(
        r#"
        SELECT booking_id FROM v2.create_appointment_hold(
            103, 203, 301, $1, $2, 1, 1, 1,
            'Schedule'::v2.booking_type_enum, 'video'::v2.consultation_type_enum,
            '2030-01-07T02:00:00Z'::timestamptz, 900, 900, true
        )
        "#,
    )
    .bind(doctor_account_id as i32)
    .bind(doctor_profile_id as i32)
    .fetch_one(pool)
    .await;
    assert!(
        overlap.is_err(),
        "active Doctor Occupancy must prevent overlap"
    );

    sqlx::query("UPDATE v2.doctor_consultation_config SET schedule_config = '{\"timezone\":\"Asia/Bangkok\",\"specificDate\":[],\"daysOfWeek\":{}}'::jsonb WHERE doctor_id = $1")
        .bind(doctor_id)
        .execute(pool)
        .await
        .unwrap();
    let schedule_change = sqlx::query_scalar::<_, String>(
        "SELECT booking_id FROM v2.create_appointment_hold(104, 204, 301, $1, $2, 1, 1, 1, 'Schedule'::v2.booking_type_enum, 'video'::v2.consultation_type_enum, '2030-01-14T02:00:00Z'::timestamptz, 900, 900, true)",
    )
    .bind(doctor_account_id as i32)
    .bind(doctor_profile_id as i32)
    .fetch_one(pool)
    .await;
    assert!(
        schedule_change.is_err(),
        "transaction must recheck the current schedule window"
    );

    let instant_booking_id: String = sqlx::query_scalar(
        "SELECT booking_id FROM v2.create_appointment_hold(105, 205, 301, $1, $2, 1, 1, 1, 'Instant'::v2.booking_type_enum, 'video'::v2.consultation_type_enum, '2030-01-14T04:00:00Z'::timestamptz, 900, 900, true)",
    )
    .bind(doctor_account_id as i32)
    .bind(doctor_profile_id as i32)
    .fetch_one(pool)
    .await
    .expect("Instant Hold must not require a scheduled window");
    assert!(!instant_booking_id.is_empty());

    sqlx::query("UPDATE v2.doctor_service_config_projection SET channels = ARRAY['voice'] WHERE doctor_id = $1")
        .bind(doctor_id)
        .execute(pool)
        .await
        .unwrap();
    let unsupported_channel = sqlx::query_scalar::<_, String>(
        "SELECT booking_id FROM v2.create_appointment_hold(106, 206, 301, $1, $2, 1, 1, 1, 'Instant'::v2.booking_type_enum, 'video'::v2.consultation_type_enum, '2030-01-14T05:00:00Z'::timestamptz, 900, 900, true)",
    )
    .bind(doctor_account_id as i32)
    .bind(doctor_profile_id as i32)
    .fetch_one(pool)
    .await;
    assert!(unsupported_channel.is_err());

    sqlx::query("UPDATE v2.doctor_identity SET is_active = false WHERE doctor_id = $1")
        .bind(doctor_id)
        .execute(pool)
        .await
        .unwrap();
    let inactive_doctor = sqlx::query_scalar::<_, String>(
        "SELECT booking_id FROM v2.create_appointment_hold(107, 207, 301, $1, $2, 1, 1, 1, 'Instant'::v2.booking_type_enum, 'voice'::v2.consultation_type_enum, '2030-01-14T06:00:00Z'::timestamptz, 900, 900, true)",
    )
    .bind(doctor_account_id as i32)
    .bind(doctor_profile_id as i32)
    .fetch_one(pool)
    .await;
    assert!(inactive_doctor.is_err());
}

#[tokio::test]
async fn expiry_atomically_expires_hold_releases_occupancy_and_enqueues_v1_wire_event() {
    let database = common::setup_test_db().await;
    let pool = &database.pool;
    let booking_id = format!(
        "E{}",
        uuid::Uuid::new_v4().simple().to_string()[..18].to_uppercase()
    );
    let hold_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO v2.appointment_hold (booking_id,idempotency_key,patient_account_id,patient_profile_id,doctor_id,doctor_account_id,doctor_profile_id,tenant_id,booking_type,consultation_channel,starts_at,ends_at,expires_at,purpose_code,hold_status) VALUES ($1,'expiry:' || $1,1,2,3,4,5,1,'Instant','video',NOW() + INTERVAL '1 hour',NOW() + INTERVAL '90 minutes',NOW() - INTERVAL '1 minute','PATIENT_BOOKING','ACTIVE') RETURNING hold_id"
    ).bind(&booking_id).fetch_one(pool).await.expect("test Hold must insert");
    sqlx::query("INSERT INTO v2.doctor_occupancy (doctor_profile_id, starts_at, ends_at, hold_id) VALUES (5, NOW() + INTERVAL '1 hour', NOW() + INTERVAL '90 minutes', $1)")
        .bind(hold_id).execute(pool).await.expect("test occupancy must insert");

    let expired: Vec<(String,)> = sqlx::query_as(
        "SELECT booking_id FROM v2.expire_appointment_holds(10, 'hold-expiry-test')",
    )
    .fetch_all(pool)
    .await
    .expect("expiry transaction must complete");
    assert_eq!(expired, vec![(booking_id.clone(),)]);
    let hold_status: String =
        sqlx::query_scalar("SELECT hold_status::text FROM v2.appointment_hold WHERE hold_id=$1")
            .bind(hold_id)
            .fetch_one(pool)
            .await
            .unwrap();
    let occupancy_status: String = sqlx::query_scalar(
        "SELECT occupancy_status::text FROM v2.doctor_occupancy WHERE hold_id=$1",
    )
    .bind(hold_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let event: serde_json::Value = sqlx::query_scalar("SELECT payload FROM v2.event_outbox WHERE aggregate_id=$1 AND event_type='ReservationExpired'")
        .bind(&booking_id).fetch_one(pool).await.unwrap();
    assert_eq!(hold_status, "EXPIRED");
    assert_eq!(occupancy_status, "RELEASED");
    assert_eq!(event["__type"], "ReservationExpired");
    assert_eq!(event["bookingId"], booking_id);
}

#[tokio::test]
async fn booking_transfers_hold_prescreen_and_uses_a_distinct_appointment_identity() {
    let database = common::setup_test_db().await;
    let pool = &database.pool;
    let doctor_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO v2.doctor_identity (doctor_id, doctor_account_id, doctor_profile_id, is_active) VALUES ($1, 71001, 72001, true)")
        .bind(doctor_id).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO v2.doctor_consultation_config (doctor_id, instant_available, schedule_available) VALUES ($1, true, false)")
        .bind(doctor_id).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO v2.doctor_service_config_projection (doctor_id, channels, languages, duration_minutes, doctor_fee_amount, doctor_fee_currency, profile_version, source_event_id) VALUES ($1, ARRAY['video'], ARRAY['en'], 15, 525.00, 'THB', 9, 'prescreen-transfer')")
        .bind(doctor_id).execute(pool).await.unwrap();
    let booking_id: String = sqlx::query_scalar("SELECT booking_id FROM v2.create_appointment_hold(71,72,73,71001,72001,1,1,1,'Instant','video',NOW() + INTERVAL '2 hours',900,900,true)")
        .fetch_one(pool).await.unwrap();
    let prescreen_id: i32 =
        sqlx::query_scalar("SELECT v2.attach_hold_prescreen($1, $2, 'RAW_JSON')")
            .bind(&booking_id)
            .bind(r#"{"symptom":"fever"}"#)
            .fetch_one(pool)
            .await
            .unwrap();
    sqlx::query("SELECT v2.confirm_payment_and_enqueue_consultation_booked($1, 99, 'pay-99', '[]'::jsonb, $2::numeric, $3, 0, 1, 'hold-test-events')")
        .bind(&booking_id)
        .bind("525.00")
        .bind("THB")
        .execute(pool)
        .await
        .unwrap();
    let (appointment_id, appointment_prescreen_id, source_prescreen_id): (String, i32, Option<i32>) = sqlx::query_as(
        "SELECT appointment_id, prescreen_data_id, source_hold_prescreen_id FROM v2.appointment WHERE booking_id=$1",
    ).bind(&booking_id).fetch_one(pool).await.unwrap();
    assert_ne!(
        appointment_id, booking_id,
        "Appointment identity must not reuse bookingId"
    );
    assert_eq!(appointment_prescreen_id, prescreen_id);
    assert_eq!(source_prescreen_id, Some(prescreen_id));
}

#[tokio::test]
async fn instant_hold_quote_drives_one_payment_confirmation_and_appointment() {
    let database = common::setup_test_db().await;
    let pool = &database.pool;
    let doctor_id = uuid::Uuid::new_v4();

    sqlx::query("INSERT INTO v2.doctor_identity (doctor_id, doctor_account_id, doctor_profile_id, is_active) VALUES ($1, 81001, 82001, true)")
        .bind(doctor_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO v2.doctor_consultation_config (doctor_id, instant_available, schedule_available) VALUES ($1, true, false)")
        .bind(doctor_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO v2.doctor_service_config_projection (doctor_id, channels, languages, duration_minutes, doctor_fee_amount, doctor_fee_currency, profile_version, source_event_id) VALUES ($1, ARRAY['video'], ARRAY['en'], 15, 350.00, 'THB', 1, 'local-contract-e2e')")
        .bind(doctor_id)
        .execute(pool)
        .await
        .unwrap();

    let booking_id: String = sqlx::query_scalar(
        "SELECT booking_id FROM v2.create_appointment_hold(81,82,83,81001,82001,1,1,1,'Instant','video',NOW() + INTERVAL '2 hours',900,900,true)",
    )
    .fetch_one(pool)
    .await
    .expect("instant Hold must persist");
    sqlx::query_scalar::<_, i32>("SELECT v2.attach_hold_prescreen($1, $2, 'RAW_JSON')")
        .bind(&booking_id)
        .bind(r#"{"symptom":"Headache for two days"}"#)
        .fetch_one(pool)
        .await
        .expect("Hold must persist its prescreen before payment confirmation");
    let (quoted_amount, quoted_currency): (String, String) = sqlx::query_as(
        "SELECT quoted_amount::text, quoted_currency FROM v2.appointment_hold WHERE booking_id = $1",
    )
    .bind(&booking_id)
    .fetch_one(pool)
    .await
    .expect("persisted Hold must expose its immutable quote");

    sqlx::query("SELECT v2.confirm_payment_and_enqueue_consultation_booked($1, 81042, 'PT-LOCAL-CONTRACT-001', '[]'::jsonb, $2::numeric, $3, 1, EXTRACT(EPOCH FROM NOW())::bigint, 'local-contract-events')")
        .bind(&booking_id)
        .bind(&quoted_amount)
        .bind(&quoted_currency)
        .execute(pool)
        .await
        .expect("payment confirmation must accept the immutable Hold quote");

    let appointment_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM v2.appointment WHERE booking_id = $1")
            .bind(&booking_id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(appointment_count, 1);
    let outbox_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2.event_outbox WHERE aggregate_id = $1 AND event_type = 'ConsultationBooked'",
    )
    .bind(&booking_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(outbox_count, 1);
}
