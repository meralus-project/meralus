use core::fmt;

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Glutin(glutin::error::Error),
    ShaderCreation(String),
    TextureCreation(String),
    BufferCreation(String),
}

impl From<glutin::error::Error> for Error {
    fn from(value: glutin::error::Error) -> Self {
        Self::Glutin(value)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Glutin(error) => error.fmt(f),
            Self::ShaderCreation(error) => write!(f, "Shader creation error: {error}"),
            Self::TextureCreation(error) => write!(f, "Texture creation error: {error}"),
            Self::BufferCreation(error) => write!(f, "Buffer creation error: {error}"),
        }
    }
}

impl core::error::Error for Error {}
