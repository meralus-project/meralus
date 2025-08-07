use std::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

static CURRENT_INDENT: AtomicUsize = AtomicUsize::new(0);

pub fn indent_up() {
    CURRENT_INDENT.fetch_sub(1, Ordering::Relaxed);
}

pub fn indent_down() {
    CURRENT_INDENT.fetch_add(1, Ordering::Relaxed);
}

pub trait PrettyFmt {
    #[allow(clippy::missing_errors_doc)]
    fn write_indent(&mut self) -> fmt::Result;
    #[allow(clippy::missing_errors_doc)]
    fn write_array_like<V: fmt::Display, T: IntoIterator<Item = V>>(&mut self, items: T, linebreak: bool) -> fmt::Result;
}

impl PrettyFmt for fmt::Formatter<'_> {
    fn write_indent(&mut self) -> fmt::Result {
        self.write_str(&" ".repeat(2 * CURRENT_INDENT.load(Ordering::Relaxed)))
    }

    fn write_array_like<V: fmt::Display, T: IntoIterator<Item = V>>(&mut self, items: T, linebreak: bool) -> fmt::Result {
        let mut first_taken = false;

        for item in items {
            if first_taken {
                if linebreak {
                    self.write_str(",\n")?;
                } else {
                    self.write_str(", ")?;
                }
            }

            if linebreak {
                self.write_indent()?;
            }

            item.fmt(self)?;

            if !first_taken {
                first_taken = true;
            }
        }

        Ok(())
    }
}
