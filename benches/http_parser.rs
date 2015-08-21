#![cfg_attr(test, feature(test))]

#[cfg(test)]
extern crate test;
extern crate combine;

use test::Bencher;

use combine::*;
use combine::combinator::{take_while1, range};
use combine::primitives::Error;

use std::fs::File;
use std::env;


#[derive(Debug)]
struct Request<'a> {
    method:  &'a [u8],
    uri:     &'a [u8],
    version: &'a [u8],
}

#[derive(Debug)]
struct Header<'a> {
    name:  &'a [u8],
    value: Vec<&'a [u8]>,
}

fn is_token(c: u8) -> bool {
    c < 128 && c > 31 && b"()<>@,;:\\\"/[]?={} \t".iter().position(|&i| i == c).is_none()
}

fn is_horizontal_space(c: u8) -> bool { c == b' ' || c == b'\t' }
fn is_space(c: u8)            -> bool { c == b' ' }
fn is_not_space(c: u8)        -> bool { c != b' ' }
fn is_http_version(c: u8)     -> bool { c >= b'0' && c <= b'9' || c == b'.' }

#[bench]
fn http_parser(b: &mut Bencher) {
    let mut contents: Vec<u8> = Vec::new();

    {
        use std::io::Read;

        let mut file = File::open("benches/http_requests.txt").ok().expect("Failed to open file");

        let _ = file.read_to_end(&mut contents).unwrap();
    }

    // Making a closure, because parser instances cannot be reused
    let end_of_line = || (satisfy(|&c| c == b'\r').skip(satisfy(|&c| c == b'\n'))).or(satisfy(|&c| c == b'\n'));

    // Cannot use char() as it requires a char, instead use satisfy and dereference the pointer to
    // the item
    let mut http_version = range(&b"HTTP/"[..])
        // Need a map() here to be able to use FromIterator<u8>
        .with(take_while1(|& &c| is_http_version(c)));

    let request_line = parser(|input| (
        // Yet again, dereferencing pointers before checking if it is a token
        take_while1(|& &c| is_token(c)),
        take_while1(|& &c| is_space(c)),
        take_while1(|& &c| is_not_space(c)),
        take_while1(|& &c| is_space(c)),
        &mut http_version,
        ).map(|(method, _, uri, _, version)| Request {
              method:  method,
              uri:     uri,
              version: version,
        })
        .parse_lazy(input)
    );


    let message_header = parser(|input| {
        let message_header_line = (
            take_while1(|& &c| is_horizontal_space(c)),
            take_while1(|& &c| c != b'\r' && c != b'\n'),
            end_of_line())
            .map(|(_, line, _)| line);

            (take_while1(|& &c| is_token(c)),
            satisfy(|&c| c == b':'),
            many1(message_header_line)
            )
            .map(|(name, _, value)| Header {
                name: name,
                value: value,
            })
            .parse_lazy(input)
        });

    let mut request = (
        request_line,
        end_of_line(),
        many(message_header),
        end_of_line()
        ).map(|(request, _, headers, _)| (request, headers));

    let mut i   = 0;
    let mut buf = &contents[..];
    b.iter(|| {
        loop {
            // Needed for inferrence for many(message_header)
            let r: Result<((Request, Vec<Header>), _), _> = request.parse(buf);

            match r {
                Ok(((_, _), b)) => {
                    i = i + 1;

                    buf = b
                },
                Err(e) => {
                    if e.errors[0] == Error::end_of_input() {
                        return
                    }
                    panic!("{:?}", e);
                }
            }

            if buf.is_empty() {
                break;
            }
        }
    });
}
