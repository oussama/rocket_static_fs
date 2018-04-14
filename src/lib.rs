//! rocket_static_fs implements a simplistic but functional static file server for the
//! rocket framework.
//!
//! # Example
//!
//! This example works for sharing the src folder of your app.
//!
//! ```
//! #![feature(plugin)]
//! #![plugin(rocket_codegen)]
//!
//! extern crate rocket;
//! extern crate rocket_static_fs;
//!
//! use rocket_static_fs::{StaticFileServer, fs};
//!
//! #[get("/")]
//! fn index() -> &'static str {
//!     "Hello, world!"
//! }
//!
//! fn main() {
//!     rocket::ignite()
//!         .attach(StaticFileServer::new(fs::LocalFileSystem::new("src"), "/src/").unwrap())
//!         .mount("/", routes![index]);
//!     // And finally launch it
//! }
//! ```

extern crate chrono;
extern crate flate2;
extern crate mime_guess;
extern crate regex;
extern crate rocket;
#[macro_use]
extern crate lazy_static;
extern crate byteorder;

pub mod fs;

use chrono::prelude::*;
use flate2::read::GzEncoder;
use flate2::Compression;
use fs::FileSystem;
use mime_guess::get_mime_type;
use regex::Regex;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::http::Method;
use rocket::http::Status;
use rocket::{Request, Response};
use std::error::Error as StdError;
use std::fmt;
use std::io::Read;
use std::path::Path;
use std::str::FromStr;

lazy_static! {
    static ref RANGE_HEADER_REGEX: Regex = Regex::new(r#"(.*?)=(\d+)-(\d+)"#).unwrap();
}

const LAST_MODIFIED_DATE_FORMAT: &str = "%a, %d %b %Y %H:%M:%S GMT";

#[derive(Debug)]
struct Error {
    description: String,
}

impl Error {
    fn new(description: &str) -> Self {
        Error {
            description: description.to_string(),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        &self.description
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_str(&self.description)
    }
}

/// Represents a `Range` header.
///
/// Implements FromStr for convenience.
struct Range {
    typ: String,
    start: u64,
    end: u64,
}

impl Range {
    fn len(&self) -> u64 {
        self.end - self.start + 1
    }
}

impl FromStr for Range {
    type Err = Box<StdError>;

    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        match RANGE_HEADER_REGEX.captures(s) {
            Some(matches) => {
                let typ = &matches[1];
                let start: u64 = matches[2].parse()?;
                let end: u64 = matches[3].parse()?;

                Ok(Range {
                    typ: typ.to_string(),
                    start,
                    end,
                })
            }
            None => Err(Box::new(Error::new("invalid range header"))),
        }
    }
}

/// StaticFileServer is your fairing for the static file server.
pub struct StaticFileServer<T>
where
    T: FileSystem + Sized + Send + Sync,
{
    fs: T,
    prefix: String,
}

impl<T> StaticFileServer<T>
where
    T: FileSystem + Sized + Send + Sync,
{
    /// Constructs a new StaticFileServer fairing.
    ///
    /// `path` is local directory to serve from.
    /// `prefix` is the prefix the serve from.
    ///
    /// You can set a prefix of /assets and only requests to /assets/* will be served.
    pub fn new(fs: T, prefix: &str) -> Result<Self, Box<StdError>> {
        let mut prefix = prefix.to_string();
        if !prefix.ends_with('/') {
            prefix.push_str("/");
        }

        Ok(StaticFileServer { fs, prefix })
    }
}

impl<T: 'static> Fairing for StaticFileServer<T>
where
    T: FileSystem + Sized + Send + Sync,
{
    fn info(&self) -> Info {
        Info {
            name: "static_file_server",
            kind: Kind::Response,
        }
    }

    fn on_response(&self, request: &Request, response: &mut Response) {
        // Only handle requests which aren't otherwise handled.
        if response.status() != Status::NotFound {
            return;
        }

        // Only handle requests which include our prefix
        let uri = request.uri().as_str();
        if !((request.method() == Method::Get || request.method() == Method::Head)
            && uri.starts_with(&self.prefix))
        {
            return;
        }

        // Strip out the prefix to get the normal file path
        let req_path = uri.replacen(&self.prefix, "", 1);

        // Fail on paths outside of the given path
        if !self.fs.path_valid(&req_path) {
            response.set_status(Status::Forbidden);
            return;
        };

        // Fail if it is no file
        // TODO: Support directory listing
        if !self.fs.is_file(&req_path) {
            response.set_status(Status::NotFound);
            return;
        };

        // Let's set the mime type here, this can't possibly go wrong anymore *cough*.
        {
            let file_extension = Path::new(&req_path).extension().unwrap().to_str().unwrap();
            let mime = get_mime_type(file_extension).to_string();
            response.set_raw_header("Content-Type", mime);
        };

        // Get the file modification date and the If-Modified-Since header value
        let modified = self.fs.last_modified(&req_path).expect("no modified since");
        let modified: DateTime<Utc> = DateTime::from(modified);
        let if_modified_since = request.headers().get("If-Modified-Since").next();

        // Only on a GET request: If the If-Modified-Since header and the modified time of the file are the same, we
        // respond with a 304 here
        if request.method() == Method::Get {
            if let Some(time) = if_modified_since {
                if let Ok(time) = Utc.datetime_from_str(&time, LAST_MODIFIED_DATE_FORMAT) {
                    let duration: chrono::Duration = time.signed_duration_since(modified);
                    if duration.num_seconds() == 0 {
                        response.set_status(Status::NotModified);
                        return;
                    };
                };
            };
        }

        let size = match self.fs.size(&req_path) {
            Ok(s) => s,
            Err(_) => {
                response.set_status(Status::Forbidden);
                return;
            }
        };

        // In case someone heads the file, we inform him about the content length and
        // that we support byte ranges.
        if request.method() == Method::Head {
            response.set_header(Header::new("Accept-Ranges", "bytes"));
            response.set_header(Header::new("Content-Length", format!("{}", size)));
            response.set_status(Status::Ok);
            return;
        }

        // Let's parse the range header if it exists
        let range_header = request.headers().get_one("Range").unwrap_or("");

        // If we get a multipart range request, we more or less fail gracefully here for the moment.
        // We simply set the range here to an error and send the complete file cause of that.
        // TODO: Support multipart ranges
        let range: Result<Range, Box<StdError>> = if range_header.contains(',') {
            Err(Box::new(Error::new("multipart ranges not supported")))
        } else {
            range_header.parse::<Range>()
        };

        // Set the start byte for the request
        let start = match range {
            Ok(ref range) => range.start,
            Err(_) => 0,
        };

        // Otherwise we try to send the file, which should work since that size above should have
        // worked as well.
        match self.fs.open(&req_path, Some(start)) {
            Ok(mut f) => {
                response.set_header(Header::new("Accept-Ranges", "bytes"));
                response.set_status(Status::Ok);
                response.set_raw_header(
                    "Last-Modified",
                    modified.format(LAST_MODIFIED_DATE_FORMAT).to_string(),
                );

                // If we got a range header, we set the corresponding headers here and
                // set f to a limit reader so it will stop when it reached the range len.
                if let Ok(ref range) = range {
                    f = Box::new(f.take(range.len()));
                    response.set_header(Header::new("Content-Length", format!("{}", range.len())));
                    response.set_header(Header::new(
                        "Content-Range",
                        format!("{}={}-{}/{}", range.typ, range.start, range.end, size),
                    ));
                    response.set_status(Status::PartialContent);
                }

                // In case the client accepts encodings, we handle these
                // TODO: Support more encodings
                if let Some(encodings) = request.headers().get_one("Accept-Encoding") {
                    if encodings.contains("gzip") {
                        let mut encoder = GzEncoder::new(f, Compression::default());
                        response.set_raw_header("Content-Encoding", "gzip");
                        response.set_streamed_body(encoder);
                        return;
                    };
                };

                response.set_streamed_body(f);
            }
            Err(_) => {
                // TODO: What else could go wrong here? IMO it can be just no permissions
                response.set_status(Status::Forbidden);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fs::LocalFileSystem;
    use super::Range;
    use super::StaticFileServer;
    use rocket;
    use rocket::http::{Header, Status};
    use rocket::local::Client;

    #[test]
    fn test_with_local_filesystem() {
        let fs = LocalFileSystem::new("src");
        let rocket = rocket::ignite().attach(StaticFileServer::new(fs, "/test").unwrap());
        let client = Client::new(rocket).expect("valid rocket");

        // Test simply getting a file
        let resp = client.get("/test/lib.rs").dispatch();
        assert_eq!(resp.status(), Status::Ok);
        assert_eq!(
            resp.headers()
                .get_one("Content-Type")
                .expect("no content type"),
            "text/x-rust"
        );

        let last_modified = resp.headers()
            .get_one("Last-Modified")
            .expect("no last modified header")
            .to_owned();

        // Check for NotModified on second response with If-Modified-Since header
        let resp = client
            .get("/test/lib.rs")
            .header(Header::new("If-Modified-Since", last_modified))
            .dispatch();
        assert_eq!(resp.status(), Status::NotModified);

        // Test for Range support
        let mut resp = client
            .get("/test/lib.rs")
            .header(Header::new("Range", "bytes=5-10"))
            .dispatch();
        assert_eq!(resp.status(), Status::PartialContent);
        assert_eq!(resp.headers().get_one("Content-Length"), Some("6"));
        let body = resp.body_bytes().unwrap();
        assert_eq!(body.len(), 6);
    }

    #[test]
    fn test_parse_range_header() {
        let range: Range = "bytes=0-1023"
            .parse()
            .expect("unable to parse Range header");
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 1023);
        assert_eq!(range.typ, "bytes");
    }
}
