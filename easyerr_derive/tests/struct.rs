use easyerr::*;

#[derive(Debug, Error)]
#[error(transparent)]
pub struct FooError<const LEN: usize> {
    source: std::io::Error,
    x: [u64; LEN],
}

#[derive(Debug, Error)]
#[error("something went wrong with the number {f0}")]
pub struct BarError(u64);

#[derive(Debug, Error)]
#[error("something went wrong")]
pub struct BazError;

pub fn main() {
    let _e = Err::<(), _>(std::io::Error::new(std::io::ErrorKind::NotFound, "oops"))
        .context(FooCtx { x: [0] });
}
