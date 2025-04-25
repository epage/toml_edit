use toml_write::TomlWrite as _;

use super::Error;

#[doc(hidden)]
pub struct SerializeValueArray<'d> {
    dst: &'d mut String,
    seen_value: bool,
}

impl<'d> SerializeValueArray<'d> {
    pub(crate) fn new(dst: &'d mut String) -> Result<Self, Error> {
        dst.open_array()?;
        Ok(Self {
            dst,
            seen_value: false,
        })
    }
}

impl serde::ser::SerializeSeq for SerializeValueArray<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        if self.seen_value {
            self.dst.val_sep()?;
            self.dst.space()?;
        }
        self.seen_value = true;
        value.serialize(super::ValueSerializer::new(self.dst))?;
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.dst.close_array()?;
        Ok(())
    }
}

impl serde::ser::SerializeTuple for SerializeValueArray<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}

impl serde::ser::SerializeTupleVariant for SerializeValueArray<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}

impl serde::ser::SerializeTupleStruct for SerializeValueArray<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}
