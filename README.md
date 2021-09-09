# stackful

[![](https://img.shields.io/crates/v/stackful.svg)](https://crates.io/crates/stackful)
[![](https://docs.rs/stackful/badge.svg)](https://docs.rs/stackful/)
[![Build Status](https://github.com/nbdd0121/stackful/workflows/CI/badge.svg)](https://github.com/nbdd0121/stackful/actions)

`stackful` attempts to bridge sync and async and blur the difference between them.

It allows you to easily convert between them with two supplied function `wait` and `stackful`.
It can be quitely useful if you are using a library that only provides sync interface on top of
async IO.

More details can be found in the docs or the source code.

## Example

```Rust
use async_std::io::Read as AsyncRead;
use async_std::prelude::*;
use byteorder::{ReadBytesExt, LE};
use stackful::{stackful, wait};
use std::io::Read;
use std::marker::Unpin;

struct Sync<T>(T);

impl<T> Read for Sync<T>
where
    T: AsyncRead + Unpin,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        wait(self.0.read(buf))
    }
}

async fn process(stream: &mut (dyn AsyncRead + Unpin)) -> u32 {
    stackful(|| {
        let mut sync = Sync(stream);
        // Note that this will recursively call into `read` function will
        // calls `wait` to await the future.
        sync.read_u32::<LE>().unwrap()
        // This is just an example, can be complex processing, zipping, etc.
        // If you are calling into a FFI library that uses a callback, you
        // can even `wait()` from that callback and turn the whole FFI library
        // into async!
    })
    .await
}

fn main() {
    async_std::task::block_on(async {
        async_std::task::spawn_local(async {
            // This is just an example, can be any AsyncRead stream
            let mut stream: &[u8] = &[0xef, 0xbe, 0xad, 0xde];
            println!("{:x}", process(&mut stream).await);
        })
        .await;
    });
}
```
