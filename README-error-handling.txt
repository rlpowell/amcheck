Because I keep confusing myself: the error handling we're using is tracing + thiserror + error_stack.  That last is the special sauce for getting nested reports of errors through a call stack.

Note in particular that because we're using error_stack, we don't need to chain errors; so for example this:

#[derive(Debug, Error)]
enum MyError {
    #[error("IMAP error")]
    Imap,

doesn't look like this:

#[derive(Debug, Error)]
enum MyError {
    #[error("IMAP error")]
    Imap(#[from] ImapError),

, and that's on purpose.

https://www.youtube.com/watch?v=jpVzSse7oJ4 covers why you want
thiserror well; good overview of Rust error handling
