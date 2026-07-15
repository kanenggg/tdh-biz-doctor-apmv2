use utoipa::openapi::{schema, KnownFormat, ObjectBuilder, SchemaFormat};

pub fn bigdecimal_schema() -> utoipa::openapi::schema::Schema {
    ObjectBuilder::new()
        .schema_type(schema::Type::Number)
        .format(Some(SchemaFormat::KnownFormat(KnownFormat::Float)))
        .build()
        .into()
}
