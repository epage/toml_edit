use crate::easy::de::Error;

pub(crate) struct ItemDeserializer {
    input: crate::Item,
    validate_struct_keys: bool,
}

impl ItemDeserializer {
    pub(crate) fn new(input: crate::Item) -> Self {
        Self {
            input,
            validate_struct_keys: false,
        }
    }

    pub(crate) fn with_struct_key_validation(mut self) -> Self {
        self.validate_struct_keys = true;
        self
    }
}

impl<'de, 'a> serde::Deserializer<'de> for ItemDeserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self.input {
            crate::Item::None => visitor.visit_none(),
            crate::Item::Value(v) => {
                crate::easy::de::ValueDeserializer::new(v).deserialize_any(visitor)
            }
            crate::Item::Table(v) => visitor.visit_map(crate::easy::de::TableMapAccess::new(v)),
            crate::Item::ArrayOfTables(v) => {
                visitor.visit_seq(crate::easy::de::ArraySeqAccess::with_array_of_tables(v))
            }
        }
    }

    // `None` is interpreted as a missing field so be sure to implement `Some`
    // as a present field.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if self.validate_struct_keys {
            match &self.input {
                crate::Item::Table(values) => super::validate_struct_keys(&values.items, fields)?,
                crate::Item::Value(crate::Value::InlineTable(values)) => {
                    super::validate_struct_keys(&values.items, fields)?
                }
                _ => {}
            }
        }

        self.deserialize_any(visitor)
    }

    // Called when the type to deserialize is an enum, as opposed to a field in the type.
    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self.input {
            crate::Item::Value(v) => {
                crate::easy::de::ValueDeserializer::new(v).deserialize_enum(name, variants, visitor)
            }
            crate::Item::Table(v) => {
                if v.is_empty() {
                    Err(crate::easy::de::Error::custom(
                        "wanted exactly 1 element, found 0 elements",
                    ))
                } else if v.len() != 1 {
                    Err(crate::easy::de::Error::custom(
                        "wanted exactly 1 element, more than 1 element",
                    ))
                } else {
                    visitor.visit_enum(crate::easy::de::TableMapAccess::new(v))
                }
            }
            _ => Err(crate::easy::de::Error::custom("wanted string or table")),
        }
    }

    serde::forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string seq
        bytes byte_buf map unit newtype_struct
        ignored_any unit_struct tuple_struct tuple identifier
    }
}
