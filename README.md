# rocket_static_fs

A simple static file server for Rust's rocket framework.

[Documentation](https://docs.rs/rocket_static_fs)

## Features

- Basic HTTP caching via Last-Modified header
- GZip encoding
- `Range` support (no multipart ranges yet)
- Support for multiple file backends:
  - LocalFileSystem => serve file from a local directory
  - EmbeddedFileSystem => serve files which are bundled into the binary
    - An example for that is documented on the EmbeddedFileSystem struct
  - You can add your own FileSystem implementations by implementing the fs::FileSystem trait

## Todos

- Support for more encodings
- Cache-Control header rules
- Support directory listing

## Suggestions / Contributions?

Submit an issue/PR. But in almost all cases it's better to first open
an issue before submitting a PR, so you don't waste your time implementing
a PR which may get rejected.
 
# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.
