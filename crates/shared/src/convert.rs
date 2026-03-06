use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    Integer(u8),
    Size,
}

impl fmt::Display for Base {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(base) => base.fmt(f),
            Self::Size => f.write_str("size"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntConversionError((char, Base), (char, Base));

impl fmt::Display for IntConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conversion from {}{} to {}{} failed", self.0.0, self.0.1, self.1.0, self.1.1)
    }
}

impl std::error::Error for IntConversionError {}

pub trait ConvertFrom<T>: Sized {
    type Error: std::error::Error;

    fn convert_from(value: T) -> Result<Self, Self::Error>;
}

impl<I, O> ConvertFrom<I> for O
where
    I: ConvertTo<O>,
{
    type Error = I::Error;

    fn convert_from(value: I) -> Result<Self, Self::Error> {
        value.convert()
    }
}

pub trait ConvertTo<T> {
    type Error: std::error::Error;

    fn convert(self) -> Result<T, Self::Error>;
}

impl ConvertTo<u32> for f32 {
    type Error = IntConversionError;

    fn convert(self) -> Result<u32, Self::Error> {
        if (0.0..=4_294_967_295.0).contains(&self) {
            Ok(unsafe { self.to_int_unchecked() })
        } else {
            Err(IntConversionError(('f', Base::Integer(32)), ('u', Base::Integer(32))))
        }
    }
}

impl ConvertTo<f32> for usize {
    type Error = IntConversionError;

    fn convert(self) -> Result<f32, Self::Error> {
        u16::try_from(self).map_or(Err(IntConversionError(('u', Base::Size), ('f', Base::Integer(32)))), |value| Ok(value.into()))
    }
}

impl ConvertTo<u32> for usize {
    type Error = IntConversionError;

    fn convert(self) -> Result<u32, Self::Error> {
        self.try_into().map_or(Err(IntConversionError(('u', Base::Size), ('u', Base::Integer(32)))), Ok)
    }
}

pub trait TryConvert<V>: Sized {
    fn try_convert(value: V) -> Option<Self>;
}
