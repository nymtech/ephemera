use uuid::Uuid;

//Should be decided if ids should be unique for blocks and messages internally
pub(crate) type EphemeraId = String;

pub(crate) fn generate_ephemera_id() -> EphemeraId {
    Uuid::new_v4().to_string()
}
