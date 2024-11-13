use easyerr::*;

#[derive(Debug, Error)]
enum TestError {
    #[error(transparent)]
    Foo { source: std::io::Error, a: u64 },
    #[error("something went terribly wrong!")]
    Bar,
    #[error("stringy {f0}")]
    Baz(String),
}

fn main() {
    let _e = Err::<(), _>(std::io::Error::new(std::io::ErrorKind::NotFound, "oops"))
        .context(TestCtx::Foo { a: 0 });
}
