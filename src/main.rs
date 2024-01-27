use clap::Parser as _;
use git2;
use std::fmt;

#[derive(Debug)]
enum ErrorKind {
    OpeningRepo,
    GettingHead, // https://www.youtube.com/watch?v=aS8O-F0ICxw
    ParsingBase(String),
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::OpeningRepo => write!(f, "opening repo"),
            ErrorKind::GettingHead => write!(f, "getting head"),
            ErrorKind::ParsingBase(revspec) => write!(f, "parsing base revision {:?}", revspec),
        }
    }
}

#[derive(Debug)]
struct GitError {
    kind: ErrorKind,
    repo_path: String,
    source: git2::Error,
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} for repo {}: {}", self.kind, self.repo_path, self.source)
    }
}

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = {".".to_string()})]
    repo_path: String,
    base: String,
}

fn do_main() -> Result<(), GitError> {
    let args = Args::parse();

    // TODO: Is there a nice way to make these error constructions more concise?
    // Possibly by redesigning the error types? I tried writing a local lambda
    // that captures the repo_path and takes the desc as an argument, and
    // produces another lambda that takes the source as an argument. But I ran
    // into troubles with lifetimes, I think because the outer lambda took
    // ownership of its args.
    let repo = git2::Repository::open(&args.repo_path).map_err(|e| GitError{
        kind: ErrorKind::OpeningRepo, repo_path: args.repo_path.to_string(), source: e,
    })?;
    let _head = repo.head().map_err(|e| GitError{
        kind: ErrorKind::GettingHead, repo_path: args.repo_path.to_string(), source: e,
    })?;
    let (obj, reference) = repo.revparse_ext(&args.base).map_err(|e| GitError{
        kind: ErrorKind::ParsingBase(args.base), repo_path: args.repo_path.to_string(), source: e,
    })?;
    println!("base: {:?}, {:?}", obj, reference.map_or("no ref".to_string(), |r| {
            r.kind().map_or("no kind".to_string(), |kind| kind.to_string())
    }));
    return Ok(());
}

fn main() {
    // TODO: I found if I just return a Result from main, it doesn't use Display
    // it just debug-prints the struct. So here I"m just manually printing the
    // Display representation. Is there a smarter way to do this?
    match do_main() {
        Ok(()) => println!("OK!"),
        Err(e) => println!("{}", e),
    };
}