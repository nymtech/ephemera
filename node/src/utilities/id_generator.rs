use uuid::Uuid;

pub(crate) type EphemeraId = String;

pub(crate) fn generate() -> EphemeraId {
    Uuid::new_v4().to_string()
}
