// This file is @generated by prost-build.
#[derive(serde::Serialize)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Config {
    #[prost(enumeration = "Mode", tag = "1")]
    pub mode: i32,
    #[prost(string, repeated, tag = "2")]
    pub outputs: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(message, repeated, tag = "3")]
    pub sources: ::prost::alloc::vec::Vec<super::super::shared::receiver::Source>,
}
#[derive(serde::Serialize)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Request {
    #[prost(oneof = "request::Request", tags = "1, 2")]
    pub request: ::core::option::Option<request::Request>,
}
/// Nested message and enum types in `Request`.
pub mod request {
    #[derive(serde::Serialize)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Attach {
        #[prost(message, repeated, tag = "1")]
        pub sources: ::prost::alloc::vec::Vec<
            super::super::super::shared::receiver::Source,
        >,
    }
    #[derive(serde::Serialize)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Detach {
        #[prost(message, repeated, tag = "1")]
        pub sources: ::prost::alloc::vec::Vec<
            super::super::super::shared::receiver::Source,
        >,
    }
    #[derive(serde::Serialize)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Request {
        #[prost(message, tag = "1")]
        Attach(Attach),
        #[prost(message, tag = "2")]
        Detach(Detach),
    }
}
#[derive(serde::Serialize)]
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct Response {
    #[prost(oneof = "response::Response", tags = "1, 2")]
    pub response: ::core::option::Option<response::Response>,
}
/// Nested message and enum types in `Response`.
pub mod response {
    #[derive(serde::Serialize)]
    #[derive(Clone, Copy, PartialEq, ::prost::Message)]
    pub struct Attach {}
    #[derive(serde::Serialize)]
    #[derive(Clone, Copy, PartialEq, ::prost::Message)]
    pub struct Detach {}
    #[derive(serde::Serialize)]
    #[derive(Clone, Copy, PartialEq, ::prost::Oneof)]
    pub enum Response {
        #[prost(message, tag = "1")]
        Attach(Attach),
        #[prost(message, tag = "2")]
        Detach(Detach),
    }
}
#[derive(serde::Serialize)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerEvent {
    #[prost(oneof = "server_event::Event", tags = "1, 2")]
    pub event: ::core::option::Option<server_event::Event>,
}
/// Nested message and enum types in `ServerEvent`.
pub mod server_event {
    #[derive(serde::Serialize)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct SlotSet {
        #[prost(uint32, tag = "1")]
        pub slot: u32,
        #[prost(message, optional, tag = "2")]
        pub source: ::core::option::Option<
            super::super::super::shared::receiver::Source,
        >,
    }
    #[derive(serde::Serialize)]
    #[derive(Clone, Copy, PartialEq, ::prost::Message)]
    pub struct SlotUnset {
        #[prost(uint32, tag = "1")]
        pub slot: u32,
    }
    #[derive(serde::Serialize)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Event {
        #[prost(message, tag = "1")]
        SlotSet(SlotSet),
        #[prost(message, tag = "2")]
        SlotUnset(SlotUnset),
    }
}
#[derive(serde::Serialize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Mode {
    Auto = 0,
    Manual = 1,
}
impl Mode {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Auto => "AUTO",
            Self::Manual => "MANUAL",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "AUTO" => Some(Self::Auto),
            "MANUAL" => Some(Self::Manual),
            _ => None,
        }
    }
}
