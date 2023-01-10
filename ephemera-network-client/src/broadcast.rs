#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RbMsg {
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub node_id: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub timestamp: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(oneof = "rb_msg::ReliableBroadcast", tags = "4, 5, 6, 7")]
    pub reliable_broadcast: ::core::option::Option<rb_msg::ReliableBroadcast>,
}
/// Nested message and enum types in `RbMsg`.
pub mod rb_msg {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum ReliableBroadcast {
        #[prost(message, tag = "4")]
        PrePrepare(super::PrePrepareMsg),
        #[prost(message, tag = "5")]
        Prepare(super::PrepareMsg),
        #[prost(message, tag = "6")]
        Commit(super::CommitMsg),
        #[prost(message, tag = "7")]
        Ack(super::AckMsg),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PrePrepareMsg {
    #[prost(bytes = "vec", tag = "1")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PrepareMsg {
    #[prost(bytes = "vec", tag = "1")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CommitMsg {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AckMsg {}

