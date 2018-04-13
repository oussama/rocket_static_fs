//! rocket_static_fs implements a simplistic but functional static file server for the
//! rocket framework.
//!
//! # Example
//!
//! This example works for sharing the src folder of your app.
//!
//! ```rust.ignore
//! #![feature(plugin)]
//! #![plugin(rocket_codegen)]
//!
//! extern crate rocket;
//! extern crate rocket_static_fs;
//!
//! #[get("/")]
//! fn index() -> &'static str {
//!     "Hellos, world!"
//! }
//!
//! fn main() {
//!     rocket::ignite()
//!         .attach(rocket_static_fs::StaticFileServer::new("src", "/src/").unwrap())
//!         .mount("/", routes![index])
//!         .launch();
//! }
//! ```

extern crate chrono;
extern crate flate2;
extern crate mime_guess;
extern crate rocket;

pub mod fs;

use chrono::prelude::*;
use flate2::read::GzEncoder;
use flate2::Compression;
use mime_guess::get_mime_type;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Method;
use rocket::http::Status;
use rocket::{Request, Response};
use std::error::Error;
use fs::FileSystem;
use std::path::Path;

/// StaticFileServer is your fairing for the static file server.
pub struct StaticFileServer<T> where T: FileSystem + Sized + Send + Sync {
    fs: T,
    prefix: String,
}

impl<T> StaticFileServer<T> where T: FileSystem + Sized + Send + Sync {
    /// Constructs a new StaticFileServer fairing.
    ///
    /// `path` is local directory to serve from.
    /// `prefix` is the prefix the serve from.
    ///
    /// You can set a prefix of /assets and only requests to /assets/* will be served.
    pub fn new(fs: T, prefix: &str) -> Result<Self, Box<Error>> {
        let mut prefix = prefix.to_string();
        if !prefix.ends_with('/') {
            prefix.push_str("/");
        }

        Ok(StaticFileServer { fs: fs, prefix })
    }
}

impl<T: 'static> Fairing for StaticFileServer<T> where T: FileSystem + Sized + Send + Sync {
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
        if !(request.method() == Method::Get && uri.starts_with(&self.prefix)) {
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

        // If the If-Modified-Since header and the modified time of the file are the same, we
        // respond with a 304 here
        if let Some(time) = if_modified_since {
            if let Ok(time) = Utc.datetime_from_str(&time, "%a, %d %b %Y %H:%M:%S GMT") {
                let duration: chrono::Duration = time.signed_duration_since(modified);
                if duration.num_seconds() == 0 {
                    response.set_status(Status::NotModified);
                    return;
                };
            };
        };

        // Otherwise we try to send the file, which should work since that stat above should have
        // worked as well.
        match self.fs.open(req_path, None) {
            Ok(f) => {
                response.set_status(Status::Ok);
                response.set_raw_header(
                    "Last-Modified",
                    modified.format("%a, %d %b %Y %H:%M:%S GMT").to_string(),
                );

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
    use rocket;
    use rocket::http::{Status, Header};
    use rocket::local::Client;
    use super::StaticFileServer;
    use super::fs::LocalFileSystem;

    #[test]
    fn test_with_local_filesystem() {
        let fs = LocalFileSystem::new("src");
        let rocket = rocket::ignite().attach(StaticFileServer::new(fs, "/test").unwrap());
        let client = Client::new(rocket).expect("valid rocket");

        let resp = client.get("/test/lib.rs").dispatch();
        assert_eq!(resp.status(), Status::Ok);
        assert_eq!(resp.headers().get_one("Content-Type").expect("no content type"), "text/x-rust");
        let last_modified = resp.headers().get_one("Last-Modified").expect("no last modified header").to_owned();

        let resp = client.get("/test/lib.rs").header(Header::new("If-Modified-Since", last_modified)).dispatch();
        assert_eq!(resp.status(), Status::NotModified);
    }
}