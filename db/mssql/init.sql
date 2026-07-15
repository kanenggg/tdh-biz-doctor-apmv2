create table appointment
(
    id                      int identity (12, 1)
        constraint PK_appointment_id
            primary key
        constraint FK_appointment_doctor_usid
            references appointment,
    schedule_id             int,
    order_item_id           int,
    account_usid            nvarchar(90)                            not null,
    doctor_usid             nvarchar(90)                            not null,
    appointment_status      nvarchar(45)                            not null,
    appointment_number      nvarchar(45)                            not null,
    detail                  nvarchar(max),
    appointment_type        nvarchar(45)                            not null,
    doctor_note             nvarchar(max),
    appointment_remark      nvarchar(max),
    opd_card                nvarchar(max),
    created_at              datetime
        constraint DF__appointme__creat__7F80E8EA default getdate() not null,
    updated_at              datetime
        constraint DF__appointme__updat__00750D23 default NULL,
    created_by              nvarchar(90),
    updated_by              nvarchar(90),
    doctor_account_id       int,
    appointment_time        datetime,
    patient_name            nvarchar(512),
    rating                  tinyint,
    has_prescription        bit      default 0,
    comment                 nvarchar(max),
    insurance_name          nvarchar(90),
    payment_type            nvarchar(90),
    doctor_name             nvarchar(90),
    doctor_telephone        nvarchar(90),
    patient_telephone       nvarchar(90),
    patient_email           nvarchar(90),
    appointment_end         datetime,
    meet_type               varchar(15),
    canceled_by             nvarchar(90),
    canceled_reason         nvarchar(1024),
    refund                  bit,
    ignore_payment          bit,
    ignore_payment_reason   nvarchar(90),
    remark                  nvarchar(1024),
    duration_extra          smallint default 300                    not null,
    duration_of_symptom     int,
    duration_unit           int,
    biz_unit_id             int      default 1,
    patient_device_platform varchar(100)
)
go

create index appointment_appointment_number_index
    on appointment (appointment_number)
go

create index appointment_doctor_available_idx
    on appointment (doctor_usid asc, appointment_status asc, appointment_time desc, appointment_end desc)
go

create table appointment_archives
(
    id                 int identity
        constraint PK_appointment_archives_id
            primary key,
    appointment_id     int                            not null,
    appointment_number nvarchar(100),
    tokbox_session_id  nvarchar(max),
    archive_file_url   nvarchar(max),
    status             nvarchar(45) default 'available',
    created_at         datetime     default getdate() not null,
    updated_at         datetime     default getdate()
)
go

create table appointment_attachments
(
    id             int identity
        constraint PK__appointm__3213E83F74BC5C22
            primary key,
    appointment_id int,
    us_id          int,
    doctor_id      int,
    attachment_url nvarchar(512),
    created_at     datetimeoffset,
    created_by     nvarchar(255)
)
go

create table appointment_icd10s
(
    id             bigint identity
        primary key,
    created_at     datetimeoffset,
    updated_at     datetimeoffset,
    deleted_at     datetimeoffset,
    appointment_id bigint,
    code           nvarchar(20)
)
go

create index idx_th_appointment_icd10s_deleted_at
    on appointment_icd10s (deleted_at)
go

create table appointment_medical_report_history
(
    id                         bigint identity
        constraint PK_appointment_medical_report_history_id
            primary key,
    created_at                 datetimeoffset,
    updated_at                 datetimeoffset,
    deleted_at                 datetimeoffset,
    revision                   int           not null,
    appointment_number         nvarchar(100) not null,
    patient_username           nvarchar(200),
    appointment_type           nvarchar(100),
    appointment_time           datetime,
    detail                     nvarchar(3000),
    diagnosis                  nvarchar(max),
    doctor_note                nvarchar(max),
    doctor_name                nvarchar(200),
    license_no                 nvarchar(100),
    medicine_prescription_item nvarchar(max),
    old_value                  nvarchar(max),
    new_value                  nvarchar(max),
    created_by                 nvarchar(90),
    updated_by                 nvarchar(90)
)
go

create table appointment_notification
(
    id              bigint identity
        primary key,
    created_at      datetimeoffset,
    updated_at      datetimeoffset,
    deleted_at      datetimeoffset,
    appointment_id  int
        constraint fk_th_appointment_notifications
            references appointment,
    notification_id nvarchar(max)
)
go

create table appointment_order_attachment
(
    id                   bigint identity
        primary key,
    created_at           datetimeoffset,
    updated_at           datetimeoffset,
    deleted_at           datetimeoffset,
    is_shown             bit,
    file_path            nvarchar(max),
    appointment_id       bigint,
    order_item_id        bigint,
    attachment_string_id nvarchar(max)
)
go

create index idx_appointment_order_attachment_deleted_at
    on appointment_order_attachment (deleted_at)
go

create index idx_th_appointment_order_attachment_deleted_at
    on appointment_order_attachment (deleted_at)
go

create table appointment_session_chat
(
    id             bigint identity
        primary key,
    created_at     datetimeoffset,
    updated_at     datetimeoffset,
    deleted_at     datetimeoffset,
    session_id     bigint,
    message        nvarchar(max),
    type           nvarchar(max),
    chat_string_id nvarchar(max),
    user_string_id nvarchar(max)
)
go

create index idx_th_appointment_session_chat_deleted_at
    on appointment_session_chat (deleted_at)
go

create table appointment_session_chat_image
(
    id                   bigint identity
        primary key,
    created_at           datetimeoffset,
    updated_at           datetimeoffset,
    deleted_at           datetimeoffset,
    chat_image_string_id nvarchar(max),
    chat_id              bigint
        constraint fk_th_appointment_session_chat_images
            references appointment_session_chat,
    image_path           nvarchar(max)
)
go

create index idx_th_appointment_session_chat_image_deleted_at
    on appointment_session_chat_image (deleted_at)
go

create table appointment_session_screenshot
(
    id             bigint identity
        constraint appointment_session_screenshot_pk
            primary key nonclustered,
    created_at     datetimeoffset,
    updated_at     datetimeoffset,
    deleted_at     datetimeoffset,
    appointment_id bigint        not null,
    image_path     nvarchar(max) not null
)
go

create unique index appointment_session_screenshot_id_uindex
    on appointment_session_screenshot (id)
go

create table appointment_tokbox_session
(
    id                     int identity
        constraint PK_appointment_tokbox_session_id
            primary key,
    appointment_id         int                              not null
        constraint appointment_tokbox_session$appointment_tokbox
            references appointment,
    session_string_id      nvarchar(90)                     not null,
    token                  nvarchar(1000) default NULL,
    created_at             datetime       default getdate() not null,
    created_by             nvarchar(90),
    media_server_url       nvarchar(90),
    project_id             nvarchar(90),
    partner_id             nvarchar(90),
    session_status         nvarchar(90),
    updated_at             datetime       default getdate(),
    force_archive          bit            default 0,
    conference_provider_id int
)
go

create table appointment_transaction_log
(
    id                 int identity
        constraint PK_appointment_transaction_log_id
            primary key,
    appointment_id     int                            not null
        constraint appointment_transaction_log$appointment_transaction_log
            references appointment,
    appointment_status nvarchar(45)                   not null,
    transaction_detail nvarchar(max),
    created_at         datetime     default getdate() not null,
    created_by         nvarchar(90),
    created_source     nvarchar(45) default NULL,
    role               nvarchar(45)
)
go



