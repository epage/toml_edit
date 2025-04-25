use toml_write::TomlWrite as _;

use super::array::SerializeValueArray;
use super::key::KeySerializer;
use super::value::ValueSerializer;
use super::Error;

#[doc(hidden)]
#[allow(clippy::large_enum_variant)]
pub enum SerializeMap<'d> {
    Datetime(SerializeDatetime<'d>),
    Table(SerializeTable<'d>),
}

impl<'d> SerializeMap<'d> {
    pub(crate) fn table(dst: &'d mut String) -> Result<Self, Error> {
        Ok(Self::Table(SerializeTable::new(dst)?))
    }

    pub(crate) fn datetime(dst: &'d mut String) -> Self {
        Self::Datetime(SerializeDatetime::new(dst))
    }
}

impl serde::ser::SerializeMap for SerializeMap<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, input: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        match self {
            Self::Datetime(s) => s.serialize_key(input),
            Self::Table(s) => s.serialize_key(input),
        }
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        match self {
            Self::Datetime(s) => s.serialize_value(value),
            Self::Table(s) => s.serialize_value(value),
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self {
            Self::Datetime(s) => s.end(),
            Self::Table(s) => s.end(),
        }
    }
}

impl serde::ser::SerializeStruct for SerializeMap<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        match self {
            Self::Datetime(s) => s.serialize_field(key, value),
            Self::Table(s) => s.serialize_field(key, value),
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self {
            Self::Datetime(s) => s.end(),
            Self::Table(s) => s.end(),
        }
    }
}

#[doc(hidden)]
pub struct SerializeDatetime<'d> {
    dst: &'d mut String,
    value: Option<crate::Datetime>,
}

impl<'d> SerializeDatetime<'d> {
    pub(crate) fn new(dst: &'d mut String) -> Self {
        Self { dst, value: None }
    }
}

impl serde::ser::SerializeMap for SerializeDatetime<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, _input: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        unreachable!("datetimes should only be serialized as structs, not maps")
    }

    fn serialize_value<T>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        unreachable!("datetimes should only be serialized as structs, not maps")
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        unreachable!("datetimes should only be serialized as structs, not maps")
    }
}

impl serde::ser::SerializeStruct for SerializeDatetime<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        if key == toml_datetime::__unstable::FIELD {
            self.value = Some(value.serialize(DatetimeFieldSerializer::default())?);
        }

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        use std::fmt::Write as _;

        let value = self.value.ok_or(Error::unsupported_none())?;
        write!(self.dst, "{value}")?;
        Ok(())
    }
}

#[doc(hidden)]
pub struct SerializeTable<'d> {
    dst: &'d mut String,
    seen_value: bool,
    key: Option<String>,
}

impl<'d> SerializeTable<'d> {
    pub(crate) fn new(dst: &'d mut String) -> Result<Self, Error> {
        dst.open_inline_table()?;
        Ok(Self {
            dst,
            seen_value: false,
            key: None,
        })
    }
}

impl serde::ser::SerializeMap for SerializeTable<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, input: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        let mut encoded_key = String::new();
        input.serialize(KeySerializer {
            dst: &mut encoded_key,
        })?;
        self.key = Some(encoded_key);
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        let encoded_key = self
            .key
            .take()
            .expect("always called after `serialize_key`");
        let mut encoded_value = String::new();
        let mut value_serializer = MapValueSerializer::new(&mut encoded_value);
        let res = value.serialize(&mut value_serializer);
        match res {
            Ok(()) => {
                use std::fmt::Write as _;

                if self.seen_value {
                    self.dst.val_sep()?;
                }
                self.seen_value = true;
                self.dst.space()?;
                write!(self.dst, "{encoded_key}")?;
                self.dst.space()?;
                self.dst.keyval_sep()?;
                self.dst.space()?;
                write!(self.dst, "{encoded_value}")?;
            }
            Err(e) => {
                if !(e == Error::unsupported_none() && value_serializer.is_none) {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if self.seen_value {
            self.dst.space()?;
        }
        self.dst.close_inline_table()?;
        Ok(())
    }
}

impl serde::ser::SerializeStruct for SerializeTable<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        let mut encoded_value = String::new();
        let mut value_serializer = MapValueSerializer::new(&mut encoded_value);
        let res = value.serialize(&mut value_serializer);
        match res {
            Ok(()) => {
                use std::fmt::Write as _;

                if self.seen_value {
                    self.dst.val_sep()?;
                }
                self.seen_value = true;
                self.dst.space()?;
                self.dst.key(key)?;
                self.dst.space()?;
                self.dst.keyval_sep()?;
                self.dst.space()?;
                write!(self.dst, "{encoded_value}")?;
            }
            Err(e) => {
                if !(e == Error::unsupported_none() && value_serializer.is_none) {
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if self.seen_value {
            self.dst.space()?;
        }
        self.dst.close_inline_table()?;
        Ok(())
    }
}

#[derive(Default)]
struct DatetimeFieldSerializer {}

impl serde::ser::Serializer for DatetimeFieldSerializer {
    type Ok = toml_datetime::Datetime;
    type Error = Error;
    type SerializeSeq = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeMap = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = serde::ser::Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, _value: bool) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_i8(self, _value: i8) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_i16(self, _value: i16) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_i32(self, _value: i32) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_i64(self, _value: i64) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_u8(self, _value: u8) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_u16(self, _value: u16) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_u32(self, _value: u32) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_u64(self, _value: u64) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_char(self, _value: char) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        v.parse::<toml_datetime::Datetime>().map_err(Error::new)
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_some<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        Err(Error::date_invalid())
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        Err(Error::date_invalid())
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        Err(Error::date_invalid())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(Error::date_invalid())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::date_invalid())
    }
}

struct MapValueSerializer<'d> {
    dst: &'d mut String,
    is_none: bool,
}

impl<'d> MapValueSerializer<'d> {
    fn new(dst: &'d mut String) -> Self {
        Self {
            dst,
            is_none: false,
        }
    }
}

impl<'s> serde::ser::Serializer for &'s mut MapValueSerializer<'_> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = SerializeValueArray<'s>;
    type SerializeTuple = SerializeValueArray<'s>;
    type SerializeTupleStruct = SerializeValueArray<'s>;
    type SerializeTupleVariant = SerializeTupleVariant<'s>;
    type SerializeMap = SerializeMap<'s>;
    type SerializeStruct = SerializeMap<'s>;
    type SerializeStructVariant = SerializeStructVariant<'s>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_bool(v)
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_i8(v)
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_i16(v)
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_i32(v)
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_i64(v)
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_u8(v)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_u16(v)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_u32(v)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_u64(v)
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_f32(v)
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_f64(v)
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_char(v)
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_str(v)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_bytes(value)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.is_none = true;
        Err(Error::unsupported_none())
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        ValueSerializer::new(self.dst).serialize_some(value)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_unit()
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_unit_struct(name)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new(self.dst).serialize_unit_variant(name, variant_index, variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        ValueSerializer::new(self.dst).serialize_newtype_struct(name, value)
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        ValueSerializer::new(self.dst).serialize_newtype_variant(
            name,
            variant_index,
            variant,
            value,
        )
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        ValueSerializer::new(self.dst).serialize_seq(len)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        ValueSerializer::new(self.dst).serialize_tuple(len)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        ValueSerializer::new(self.dst).serialize_tuple_struct(name, len)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        ValueSerializer::new(self.dst).serialize_tuple_variant(name, variant_index, variant, len)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        ValueSerializer::new(self.dst).serialize_map(len)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        ValueSerializer::new(self.dst).serialize_struct(name, len)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        ValueSerializer::new(self.dst).serialize_struct_variant(name, variant_index, variant, len)
    }
}

pub(crate) type SerializeTupleVariant<'d> = SerializeVariant<SerializeValueArray<'d>>;
pub(crate) type SerializeStructVariant<'d> = SerializeVariant<SerializeMap<'d>>;

pub struct SerializeVariant<T> {
    #[allow(dead_code)]
    variant: &'static str,
    inner: T,
}

impl<'d> SerializeTupleVariant<'d> {
    pub(crate) fn tuple(
        dst: &'d mut String,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self, Error> {
        Ok(Self {
            variant,
            inner: SerializeValueArray::new(dst)?,
        })
    }
}

impl<'d> SerializeStructVariant<'d> {
    pub(crate) fn struct_(
        dst: &'d mut String,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self, Error> {
        Ok(Self {
            variant,
            inner: SerializeMap::table(dst)?,
        })
    }
}

impl serde::ser::SerializeTupleVariant for SerializeVariant<SerializeValueArray<'_>> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        serde::ser::SerializeSeq::serialize_element(&mut self.inner, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeSeq::end(self.inner)?;
        Ok(())
    }
}

impl serde::ser::SerializeStructVariant for SerializeVariant<SerializeMap<'_>> {
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        serde::ser::SerializeStruct::serialize_field(&mut self.inner, key, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeStruct::end(self.inner)?;
        Ok(())
    }
}
