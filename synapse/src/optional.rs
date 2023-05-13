#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub enum Optional<T> {
    Some(T),
    None,
}

impl<T> Optional<T> {
    pub fn into_option(self) -> Option<T> {
        self.into()
    }

    pub const fn as_ref(&self) -> Option<&T> {
        match self {
            Self::Some(x) => Some(x),
            Self::None => None,
        }
    }

    pub fn as_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Some(x) => Some(x),
            Self::None => None,
        }
    }
}

impl<T> From<Option<T>> for Optional<T> {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(v) => Optional::Some(v),
            None => Optional::None,
        }
    }
}

impl<T> From<Optional<T>> for Option<T> {
    fn from(optional: Optional<T>) -> Option<T> {
        match optional {
            Optional::Some(v) => Some(v),
            Optional::None => None,
        }
    }
}
