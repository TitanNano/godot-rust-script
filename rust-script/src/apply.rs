pub trait Apply: Sized {
    fn apply<F: FnOnce(&mut Self) -> ()>(mut self, cb: F) -> Self {
        cb(&mut self);
        self
    }
}

impl<T: Sized> Apply for T {}
