/*
 * hurl (https://hurl.dev)
 * Copyright (C) 2020 Orange
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *          http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 */

///
/// Uncompress body response
/// using the Content-Encoding response header
///
use std::io::prelude::*;

use crate::http;

use super::core::RunnerError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Encoding {
    Brotli,
    Gzip,
    Deflate,
    Identity,
}

impl http::Response {
    fn content_encoding(&self) -> Result<Option<Encoding>, RunnerError> {
        for header in self.headers.clone() {
            if header.name.as_str().to_ascii_lowercase() == "content-encoding" {
                return match header.value.as_str() {
                    "br" => Ok(Some(Encoding::Brotli)),
                    "gzip" => Ok(Some(Encoding::Gzip)),
                    "deflate" => Ok(Some(Encoding::Deflate)),
                    "identity" => Ok(Some(Encoding::Identity)),
                    v => Err(RunnerError::UnsupportedContentEncoding(v.to_string())),
                };
            }
        }
        Ok(None)
    }

    pub fn uncompress_body(&self) -> Result<Vec<u8>, RunnerError> {
        let encoding = self.content_encoding()?;
        match encoding {
            Some(Encoding::Identity) => Ok(self.body.clone()),
            Some(Encoding::Gzip) => uncompress_gzip(&self.body[..]),
            Some(Encoding::Deflate) => uncompress_zlib(&self.body[..]),
            Some(Encoding::Brotli) => uncompress_brotli(&self.body[..]),
            None => Ok(self.body.clone()),
        }
    }
}

fn uncompress_brotli(data: &[u8]) -> Result<Vec<u8>, RunnerError> {
    let mut reader = brotli::Decompressor::new(data, 4096);
    let mut buf = [0u8; 4096];
    let n = match reader.read(&mut buf[..]) {
        Err(_) => {
            return Err(RunnerError::CouldNotUncompressResponse(
                "brotli".to_string(),
            ));
        }
        Ok(size) => size,
    };
    Ok(buf[..n].to_vec())
}

fn uncompress_gzip(data: &[u8]) -> Result<Vec<u8>, RunnerError> {
    let mut decoder = match libflate::gzip::Decoder::new(data) {
        Ok(v) => v,
        Err(_) => return Err(RunnerError::CouldNotUncompressResponse("gzip".to_string())),
    };
    let mut buf = Vec::new();
    match decoder.read_to_end(&mut buf) {
        Ok(_) => Ok(buf),
        Err(_) => Err(RunnerError::CouldNotUncompressResponse("gzip".to_string())),
    }
}

fn uncompress_zlib(data: &[u8]) -> Result<Vec<u8>, RunnerError> {
    let mut decoder = match libflate::zlib::Decoder::new(data) {
        Ok(v) => v,
        Err(_) => return Err(RunnerError::CouldNotUncompressResponse("zlib".to_string())),
    };
    let mut buf = Vec::new();
    match decoder.read_to_end(&mut buf) {
        Ok(_) => Ok(buf),
        Err(_) => Err(RunnerError::CouldNotUncompressResponse("zlib".to_string())),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_content_encoding() {
        let response = http::Response {
            version: http::Version::Http10,
            status: 200,
            headers: vec![],
            body: vec![],
        };
        assert_eq!(response.content_encoding().unwrap(), None);

        let response = http::Response {
            version: http::Version::Http10,
            status: 200,
            headers: vec![http::Header {
                name: "Content-Encoding".to_string(),
                value: "xx".to_string(),
            }],
            body: vec![],
        };
        assert_eq!(
            response.content_encoding().err().unwrap(),
            RunnerError::UnsupportedContentEncoding("xx".to_string())
        );

        let response = http::Response {
            version: http::Version::Http10,
            status: 200,
            headers: vec![http::Header {
                name: "Content-Encoding".to_string(),
                value: "br".to_string(),
            }],
            body: vec![],
        };
        assert_eq!(
            response.content_encoding().unwrap().unwrap(),
            Encoding::Brotli
        );
    }

    #[test]
    fn test_uncompress_body() {
        let response = http::Response {
            version: http::Version::Http10,
            status: 200,
            headers: vec![http::Header {
                name: "Content-Encoding".to_string(),
                value: "br".to_string(),
            }],
            body: vec![
                0x21, 0x2c, 0x00, 0x04, 0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x20, 0x57, 0x6f, 0x72, 0x6c,
                0x64, 0x21,
            ],
        };
        assert_eq!(response.uncompress_body().unwrap(), b"Hello World!");

        let response = http::Response {
            version: http::Version::Http10,
            status: 200,
            headers: vec![],
            body: b"Hello World!".to_vec(),
        };
        assert_eq!(response.uncompress_body().unwrap(), b"Hello World!");
    }

    #[test]
    fn test_uncompress_brotli() {
        let data = vec![
            0x21, 0x2c, 0x00, 0x04, 0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x20, 0x57, 0x6f, 0x72, 0x6c,
            0x64, 0x21,
        ];
        assert_eq!(uncompress_brotli(&data[..]).unwrap(), b"Hello World!");
    }

    #[test]
    fn test_uncompress_gzip() {
        let data = vec![
            0x1f, 0x8b, 0x08, 0x08, 0xa7, 0x52, 0x85, 0x5f, 0x00, 0x03, 0x64, 0x61, 0x74, 0x61,
            0x2e, 0x74, 0x78, 0x74, 0x00, 0xf3, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0x08, 0xcf, 0x2f,
            0xca, 0x49, 0x51, 0x04, 0x00, 0xa3, 0x1c, 0x29, 0x1c, 0x0c, 0x00, 0x00, 0x00,
        ];
        assert_eq!(uncompress_gzip(&data[..]).unwrap(), b"Hello World!");
    }

    #[test]
    fn test_uncompress_zlib() {
        let data = vec![
            0x78, 0x9c, 0xf3, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0x08, 0xcf, 0x2f, 0xca, 0x49, 0x51,
            0x04, 0x00, 0x1c, 0x49, 0x04, 0x3e,
        ];
        assert_eq!(uncompress_zlib(&data[..]).unwrap(), b"Hello World!");
    }

    #[test]
    fn test_uncompress_error() {
        let data = vec![0x21];
        assert_eq!(
            uncompress_brotli(&data[..]).err().unwrap(),
            RunnerError::CouldNotUncompressResponse("brotli".to_string())
        );
        assert_eq!(
            uncompress_gzip(&data[..]).err().unwrap(),
            RunnerError::CouldNotUncompressResponse("gzip".to_string())
        );
    }
}
