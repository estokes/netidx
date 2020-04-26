// This file is generated by rust-protobuf 2.14.0. Do not edit
// @generated

// https://github.com/rust-lang/rust-clippy/issues/702
#![allow(unknown_lints)]
#![allow(clippy::all)]

#![cfg_attr(rustfmt, rustfmt_skip)]

#![allow(box_pointers)]
#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(trivial_casts)]
#![allow(unsafe_code)]
#![allow(unused_imports)]
#![allow(unused_results)]
//! Generated file from `shared.proto`

use protobuf::Message as Message_imported_for_functions;
use protobuf::ProtobufEnum as ProtobufEnum_imported_for_functions;

/// Generated files are compatible only with the same version
/// of protobuf runtime.
// const _PROTOBUF_VERSION_CHECK: () = ::protobuf::VERSION_2_14_0;

#[derive(PartialEq,Clone,Default,Debug)]
pub struct SocketAddr {
    // message oneof groups
    pub addr: ::std::option::Option<SocketAddr_oneof_addr>,
    // special fields
    pub unknown_fields: ::protobuf::UnknownFields,
    pub cached_size: ::protobuf::CachedSize,
}

impl<'a> ::std::default::Default for &'a SocketAddr {
    fn default() -> &'a SocketAddr {
        <SocketAddr as ::protobuf::Message>::default_instance()
    }
}

#[derive(Clone,PartialEq,Debug)]
pub enum SocketAddr_oneof_addr {
    V4(SocketAddr_SocketAddrV4),
    V6(SocketAddr_SocketAddrV6),
}

impl SocketAddr {
    pub fn new() -> SocketAddr {
        ::std::default::Default::default()
    }

    // .SocketAddr.SocketAddrV4 V4 = 1;


    pub fn get_V4(&self) -> &SocketAddr_SocketAddrV4 {
        match self.addr {
            ::std::option::Option::Some(SocketAddr_oneof_addr::V4(ref v)) => v,
            _ => SocketAddr_SocketAddrV4::default_instance(),
        }
    }

    // .SocketAddr.SocketAddrV6 V6 = 2;


    pub fn get_V6(&self) -> &SocketAddr_SocketAddrV6 {
        match self.addr {
            ::std::option::Option::Some(SocketAddr_oneof_addr::V6(ref v)) => v,
            _ => SocketAddr_SocketAddrV6::default_instance(),
        }
    }
}

impl ::protobuf::Message for SocketAddr {
    fn is_initialized(&self) -> bool {
        if let Some(SocketAddr_oneof_addr::V4(ref v)) = self.addr {
            if !v.is_initialized() {
                return false;
            }
        }
        if let Some(SocketAddr_oneof_addr::V6(ref v)) = self.addr {
            if !v.is_initialized() {
                return false;
            }
        }
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    if wire_type != ::protobuf::wire_format::WireTypeLengthDelimited {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    self.addr = ::std::option::Option::Some(SocketAddr_oneof_addr::V4(is.read_message()?));
                },
                2 => {
                    if wire_type != ::protobuf::wire_format::WireTypeLengthDelimited {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    self.addr = ::std::option::Option::Some(SocketAddr_oneof_addr::V6(is.read_message()?));
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        if let ::std::option::Option::Some(ref v) = self.addr {
            match v {
                &SocketAddr_oneof_addr::V4(ref v) => {
                    let len = v.compute_size();
                    my_size += 1 + ::protobuf::rt::compute_raw_varint32_size(len) + len;
                },
                &SocketAddr_oneof_addr::V6(ref v) => {
                    let len = v.compute_size();
                    my_size += 1 + ::protobuf::rt::compute_raw_varint32_size(len) + len;
                },
            };
        }
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        if let ::std::option::Option::Some(ref v) = self.addr {
            match v {
                &SocketAddr_oneof_addr::V4(ref v) => {
                    os.write_tag(1, ::protobuf::wire_format::WireTypeLengthDelimited)?;
                    os.write_raw_varint32(v.get_cached_size())?;
                    v.write_to_with_cached_sizes(os)?;
                },
                &SocketAddr_oneof_addr::V6(ref v) => {
                    os.write_tag(2, ::protobuf::wire_format::WireTypeLengthDelimited)?;
                    os.write_raw_varint32(v.get_cached_size())?;
                    v.write_to_with_cached_sizes(os)?;
                },
            };
        }
        os.write_unknown_fields(self.get_unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn as_any(&self) -> &dyn (::std::any::Any) {
        self as &dyn (::std::any::Any)
    }
    fn as_any_mut(&mut self) -> &mut dyn (::std::any::Any) {
        self as &mut dyn (::std::any::Any)
    }
    fn into_any(self: Box<Self>) -> ::std::boxed::Box<dyn (::std::any::Any)> {
        self
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        Self::descriptor_static()
    }

    fn new() -> SocketAddr {
        SocketAddr::new()
    }

    fn default_instance() -> &'static SocketAddr {
        static mut instance: ::protobuf::lazy::Lazy<SocketAddr> = ::protobuf::lazy::Lazy::INIT;
        unsafe {
            instance.get(SocketAddr::new)
        }
    }
}

impl ::protobuf::Clear for SocketAddr {
    fn clear(&mut self) {
        self.addr = ::std::option::Option::None;
        self.addr = ::std::option::Option::None;
        self.unknown_fields.clear();
    }
}

impl ::protobuf::reflect::ProtobufValue for SocketAddr {
    fn as_ref(&self) -> ::protobuf::reflect::ReflectValueRef {
        ::protobuf::reflect::ReflectValueRef::Message(self)
    }
}

#[derive(PartialEq,Clone,Default,Debug)]
pub struct SocketAddr_SocketAddrV4 {
    // message fields
    pub port: u32,
    pub octets: u32,
    // special fields
    pub unknown_fields: ::protobuf::UnknownFields,
    pub cached_size: ::protobuf::CachedSize,
}

impl<'a> ::std::default::Default for &'a SocketAddr_SocketAddrV4 {
    fn default() -> &'a SocketAddr_SocketAddrV4 {
        <SocketAddr_SocketAddrV4 as ::protobuf::Message>::default_instance()
    }
}

impl SocketAddr_SocketAddrV4 {
    pub fn new() -> SocketAddr_SocketAddrV4 {
        ::std::default::Default::default()
    }

    // uint32 port = 1;


    pub fn get_port(&self) -> u32 {
        self.port
    }

    // uint32 octets = 2;


    pub fn get_octets(&self) -> u32 {
        self.octets
    }
}

impl ::protobuf::Message for SocketAddr_SocketAddrV4 {
    fn is_initialized(&self) -> bool {
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_uint32()?;
                    self.port = tmp;
                },
                2 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_uint32()?;
                    self.octets = tmp;
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        if self.port != 0 {
            my_size += ::protobuf::rt::value_size(1, self.port, ::protobuf::wire_format::WireTypeVarint);
        }
        if self.octets != 0 {
            my_size += ::protobuf::rt::value_size(2, self.octets, ::protobuf::wire_format::WireTypeVarint);
        }
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        if self.port != 0 {
            os.write_uint32(1, self.port)?;
        }
        if self.octets != 0 {
            os.write_uint32(2, self.octets)?;
        }
        os.write_unknown_fields(self.get_unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn as_any(&self) -> &dyn (::std::any::Any) {
        self as &dyn (::std::any::Any)
    }
    fn as_any_mut(&mut self) -> &mut dyn (::std::any::Any) {
        self as &mut dyn (::std::any::Any)
    }
    fn into_any(self: Box<Self>) -> ::std::boxed::Box<dyn (::std::any::Any)> {
        self
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        Self::descriptor_static()
    }

    fn new() -> SocketAddr_SocketAddrV4 {
        SocketAddr_SocketAddrV4::new()
    }

    fn default_instance() -> &'static SocketAddr_SocketAddrV4 {
        static mut instance: ::protobuf::lazy::Lazy<SocketAddr_SocketAddrV4> = ::protobuf::lazy::Lazy::INIT;
        unsafe {
            instance.get(SocketAddr_SocketAddrV4::new)
        }
    }
}

impl ::protobuf::Clear for SocketAddr_SocketAddrV4 {
    fn clear(&mut self) {
        self.port = 0;
        self.octets = 0;
        self.unknown_fields.clear();
    }
}

impl ::protobuf::reflect::ProtobufValue for SocketAddr_SocketAddrV4 {
    fn as_ref(&self) -> ::protobuf::reflect::ReflectValueRef {
        ::protobuf::reflect::ReflectValueRef::Message(self)
    }
}

#[derive(PartialEq,Clone,Default,Debug)]
pub struct SocketAddr_SocketAddrV6 {
    // message fields
    pub port: u32,
    pub flowinfo: u32,
    pub scope_id: u32,
    pub octets_a: u64,
    pub octets_b: u64,
    // special fields
    pub unknown_fields: ::protobuf::UnknownFields,
    pub cached_size: ::protobuf::CachedSize,
}

impl<'a> ::std::default::Default for &'a SocketAddr_SocketAddrV6 {
    fn default() -> &'a SocketAddr_SocketAddrV6 {
        <SocketAddr_SocketAddrV6 as ::protobuf::Message>::default_instance()
    }
}

impl SocketAddr_SocketAddrV6 {
    pub fn new() -> SocketAddr_SocketAddrV6 {
        ::std::default::Default::default()
    }

    // uint32 port = 1;


    pub fn get_port(&self) -> u32 {
        self.port
    }

    // uint32 flowinfo = 2;


    pub fn get_flowinfo(&self) -> u32 {
        self.flowinfo
    }

    // uint32 scope_id = 3;


    pub fn get_scope_id(&self) -> u32 {
        self.scope_id
    }

    // uint64 octets_a = 4;


    pub fn get_octets_a(&self) -> u64 {
        self.octets_a
    }

    // uint64 octets_b = 5;


    pub fn get_octets_b(&self) -> u64 {
        self.octets_b
    }
}

impl ::protobuf::Message for SocketAddr_SocketAddrV6 {
    fn is_initialized(&self) -> bool {
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_uint32()?;
                    self.port = tmp;
                },
                2 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_uint32()?;
                    self.flowinfo = tmp;
                },
                3 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_uint32()?;
                    self.scope_id = tmp;
                },
                4 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_uint64()?;
                    self.octets_a = tmp;
                },
                5 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_uint64()?;
                    self.octets_b = tmp;
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        if self.port != 0 {
            my_size += ::protobuf::rt::value_size(1, self.port, ::protobuf::wire_format::WireTypeVarint);
        }
        if self.flowinfo != 0 {
            my_size += ::protobuf::rt::value_size(2, self.flowinfo, ::protobuf::wire_format::WireTypeVarint);
        }
        if self.scope_id != 0 {
            my_size += ::protobuf::rt::value_size(3, self.scope_id, ::protobuf::wire_format::WireTypeVarint);
        }
        if self.octets_a != 0 {
            my_size += ::protobuf::rt::value_size(4, self.octets_a, ::protobuf::wire_format::WireTypeVarint);
        }
        if self.octets_b != 0 {
            my_size += ::protobuf::rt::value_size(5, self.octets_b, ::protobuf::wire_format::WireTypeVarint);
        }
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        if self.port != 0 {
            os.write_uint32(1, self.port)?;
        }
        if self.flowinfo != 0 {
            os.write_uint32(2, self.flowinfo)?;
        }
        if self.scope_id != 0 {
            os.write_uint32(3, self.scope_id)?;
        }
        if self.octets_a != 0 {
            os.write_uint64(4, self.octets_a)?;
        }
        if self.octets_b != 0 {
            os.write_uint64(5, self.octets_b)?;
        }
        os.write_unknown_fields(self.get_unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn as_any(&self) -> &dyn (::std::any::Any) {
        self as &dyn (::std::any::Any)
    }
    fn as_any_mut(&mut self) -> &mut dyn (::std::any::Any) {
        self as &mut dyn (::std::any::Any)
    }
    fn into_any(self: Box<Self>) -> ::std::boxed::Box<dyn (::std::any::Any)> {
        self
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        Self::descriptor_static()
    }

    fn new() -> SocketAddr_SocketAddrV6 {
        SocketAddr_SocketAddrV6::new()
    }

    fn default_instance() -> &'static SocketAddr_SocketAddrV6 {
        static mut instance: ::protobuf::lazy::Lazy<SocketAddr_SocketAddrV6> = ::protobuf::lazy::Lazy::INIT;
        unsafe {
            instance.get(SocketAddr_SocketAddrV6::new)
        }
    }
}

impl ::protobuf::Clear for SocketAddr_SocketAddrV6 {
    fn clear(&mut self) {
        self.port = 0;
        self.flowinfo = 0;
        self.scope_id = 0;
        self.octets_a = 0;
        self.octets_b = 0;
        self.unknown_fields.clear();
    }
}

impl ::protobuf::reflect::ProtobufValue for SocketAddr_SocketAddrV6 {
    fn as_ref(&self) -> ::protobuf::reflect::ReflectValueRef {
        ::protobuf::reflect::ReflectValueRef::Message(self)
    }
}

#[derive(PartialEq,Clone,Default,Debug)]
pub struct PermissionToken {
    // message fields
    pub path: ::protobuf::Chars,
    pub timestamp: u64,
    // special fields
    pub unknown_fields: ::protobuf::UnknownFields,
    pub cached_size: ::protobuf::CachedSize,
}

impl<'a> ::std::default::Default for &'a PermissionToken {
    fn default() -> &'a PermissionToken {
        <PermissionToken as ::protobuf::Message>::default_instance()
    }
}

impl PermissionToken {
    pub fn new() -> PermissionToken {
        ::std::default::Default::default()
    }

    // string path = 1;


    pub fn get_path(&self) -> &str {
        &self.path
    }

    // uint64 timestamp = 2;


    pub fn get_timestamp(&self) -> u64 {
        self.timestamp
    }
}

impl ::protobuf::Message for PermissionToken {
    fn is_initialized(&self) -> bool {
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    ::protobuf::rt::read_singular_proto3_carllerche_string_into(wire_type, is, &mut self.path)?;
                },
                2 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_uint64()?;
                    self.timestamp = tmp;
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        if !self.path.is_empty() {
            my_size += ::protobuf::rt::string_size(1, &self.path);
        }
        if self.timestamp != 0 {
            my_size += ::protobuf::rt::value_size(2, self.timestamp, ::protobuf::wire_format::WireTypeVarint);
        }
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        if !self.path.is_empty() {
            os.write_string(1, &self.path)?;
        }
        if self.timestamp != 0 {
            os.write_uint64(2, self.timestamp)?;
        }
        os.write_unknown_fields(self.get_unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn as_any(&self) -> &dyn (::std::any::Any) {
        self as &dyn (::std::any::Any)
    }
    fn as_any_mut(&mut self) -> &mut dyn (::std::any::Any) {
        self as &mut dyn (::std::any::Any)
    }
    fn into_any(self: Box<Self>) -> ::std::boxed::Box<dyn (::std::any::Any)> {
        self
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        Self::descriptor_static()
    }

    fn new() -> PermissionToken {
        PermissionToken::new()
    }

    fn default_instance() -> &'static PermissionToken {
        static mut instance: ::protobuf::lazy::Lazy<PermissionToken> = ::protobuf::lazy::Lazy::INIT;
        unsafe {
            instance.get(PermissionToken::new)
        }
    }
}

impl ::protobuf::Clear for PermissionToken {
    fn clear(&mut self) {
        ::protobuf::Clear::clear(&mut self.path);
        self.timestamp = 0;
        self.unknown_fields.clear();
    }
}

impl ::protobuf::reflect::ProtobufValue for PermissionToken {
    fn as_ref(&self) -> ::protobuf::reflect::ReflectValueRef {
        ::protobuf::reflect::ReflectValueRef::Message(self)
    }
}